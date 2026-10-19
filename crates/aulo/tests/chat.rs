//! `aulo chat`, `aulo chats` and `aulo status` against `aulod --scripted`.
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::panic)]

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const AULO: &str = env!("CARGO_BIN_EXE_aulo");
const AULOD: &str = env!("CARGO_BIN_EXE_aulod");

struct Daemon {
    child: Child,
    home: tempfile::TempDir,
}

impl Daemon {
    fn start() -> Self {
        let home = tempfile::tempdir().unwrap();
        let mut child = Command::new(AULOD)
            .args(["--home", home.path().to_str().unwrap(), "--scripted"])
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        wait_until_serving(home.path(), &mut child);
        Self { child, home }
    }

    fn home(&self) -> &Path {
        self.home.path()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_until_serving(home: &Path, child: &mut Child) {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("aulod exited early: {status}");
        }
        let out = Command::new(AULO)
            .arg("status")
            .env("AULO_HOME", home)
            .output()
            .unwrap();
        if out.status.success() && out.stdout.starts_with(b"serving") {
            return;
        }
        if start.elapsed() > Duration::from_secs(15) {
            panic!(
                "aulod did not start: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn aulo(home: &Path, args: &[&str]) -> std::process::Output {
    Command::new(AULO)
        .args(args)
        .env("AULO_HOME", home)
        .output()
        .unwrap()
}

fn trycmd_case(home: &Path, case: &str) {
    let cases = trycmd::TestCases::new();
    cases.env("AULO_HOME", home.to_str().unwrap());
    cases.case(case);
}

#[test]
fn status_and_an_empty_chat_list() {
    let daemon = Daemon::start();
    trycmd_case(daemon.home(), "tests/chat/status.toml");
    trycmd_case(daemon.home(), "tests/chat/chats-ls.toml");
}

#[test]
fn a_text_turn_and_a_tool_call_render() {
    let daemon = Daemon::start();
    trycmd_case(daemon.home(), "tests/chat/hello.toml");
    trycmd_case(daemon.home(), "tests/chat/tool.toml");
}

#[test]
fn chats_can_be_removed() {
    let daemon = Daemon::start();
    let created = aulo(daemon.home(), &["chat"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let listed = aulo(daemon.home(), &["chats", "ls"]);
    assert!(listed.status.success());
    let line = String::from_utf8(listed.stdout).unwrap();
    let id = line.split('\t').next().unwrap();
    assert_eq!(id.len(), 26);
    assert!(line.contains("(untitled)"));
    let removed = aulo(daemon.home(), &["chats", "rm", id]);
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert_eq!(
        String::from_utf8(removed.stdout).unwrap(),
        format!("removed {id}\n")
    );
    let after = aulo(daemon.home(), &["chats", "ls"]);
    assert_eq!(String::from_utf8(after.stdout).unwrap(), "no chats\n");
}

#[test]
fn a_bad_chat_id_is_refused() {
    let daemon = Daemon::start();
    trycmd_case(daemon.home(), "tests/chat/chats-rm.toml");
}

#[test]
fn ctrl_c_interrupts_the_turn() {
    let daemon = Daemon::start();
    let mut child = Command::new(AULO)
        .args(["chat"])
        .env("AULO_HOME", daemon.home())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    writeln!(stdin, "wait").unwrap();
    stdin.flush().unwrap();
    let output = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let collected = std::sync::Arc::clone(&output);
    let reader = std::thread::spawn(move || {
        // The held reply is `...` with no newline yet, so this reads bytes
        // rather than lines.
        let mut reader = stdout;
        let mut buf = [0_u8; 64];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    collected
                        .lock()
                        .unwrap()
                        .push_str(&String::from_utf8_lossy(&buf[..n]));
                }
                Err(_) => break,
            }
        }
    });
    let start = Instant::now();
    loop {
        if output.lock().unwrap().contains("...") {
            break;
        }
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            panic!("no held turn: {}", output.lock().unwrap());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    drop(stdin);
    let finished = child.wait().unwrap();
    reader.join().unwrap();
    assert!(finished.success(), "{finished}");
    let text = output.lock().unwrap().clone();
    assert!(text.contains("interrupted"), "{text}");
    assert!(!text.contains('\u{1b}'));
}
