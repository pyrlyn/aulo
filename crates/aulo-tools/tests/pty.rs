//! T9.2: interactive PTY sessions. A session drives `python3 -i` and a
//! shell through a pseudo-terminal, with capped send input and a capped
//! output buffer.

// why: helpers outside `#[test]` fns still assert with expect/panic.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::time::Duration;

use aulo_tools::pty::{
    BUFFER_MAX_BYTES, INPUT_MAX_BYTES, PtyError, PtyOptions, PtySession, read_until, send_line,
};
use proc_sandbox::policy::{LinuxBackend, SandboxMode, SandboxPolicy};

/// No confinement: this host applies its own macOS sandbox to the test
/// process, and a nested restrictive Seatbelt profile is refused (see the
/// shell tests). The argv wrapping itself is covered there.
fn policy() -> SandboxPolicy {
    SandboxPolicy {
        mode: SandboxMode::DangerFullAccess,
        network: false,
        writable: vec![],
        readonly_in_workspace: vec![],
        linux_backend: LinuxBackend::Auto,
    }
}

fn options() -> PtyOptions {
    PtyOptions::default()
}

fn python() -> Vec<String> {
    vec![
        std::env::var("PYTHON3").unwrap_or_else(|_| "python3".to_owned()),
        "-i".to_owned(),
    ]
}

fn open(program: Vec<String>) -> PtySession {
    PtySession::open(&program, &policy(), &[], &[], &options()).expect("session opens")
}

#[test]
fn python_i_answers_arithmetic() {
    let mut session = open(python());
    // The prompt proves the interpreter is up and reading the pty.
    let _ = read_until(&mut session, ">>>", Duration::from_secs(10));
    send_line(&mut session, "print(6*7)").expect("send");
    let out = read_until(&mut session, "42", Duration::from_secs(10));
    assert!(
        String::from_utf8_lossy(&out).contains("42"),
        "python answered: {:?}",
        String::from_utf8_lossy(&out)
    );
    let exit = session.close().expect("close");
    assert!(exit.is_some(), "the killed program reported an exit");
}

#[test]
fn a_shell_session_runs_a_command() {
    let mut session = open(vec!["/bin/sh".to_owned()]);
    send_line(&mut session, "echo pty-ok").expect("send");
    let out = read_until(&mut session, "pty-ok", Duration::from_secs(10));
    assert!(String::from_utf8_lossy(&out).contains("pty-ok"));
    send_line(&mut session, "exit 3").expect("send");
    let exit = session.wait_exit(Duration::from_secs(10)).expect("exit");
    assert_eq!(exit.code, Some(3));
}

#[test]
fn the_buffer_is_capped_and_drops_are_counted() {
    let mut session = open(vec!["/bin/sh".to_owned()]);
    // ~600 KB of output, far over the 256 KiB buffer, with nobody reading.
    // The shell is interactive (a pty makes it so), so the line ends the
    // session once the output has flowed.
    send_line(&mut session, "seq 1 100000; exit 0").expect("send");
    let exit = session.wait_exit(Duration::from_secs(30)).expect("exit");
    assert_eq!(exit.code, Some(0));
    let drained = session.read(usize::MAX);
    assert!(
        drained.len() <= BUFFER_MAX_BYTES,
        "the buffer stayed at {} bytes",
        drained.len()
    );
    assert!(
        session.dropped() > 0,
        "the overflow was counted, not buffered"
    );
}

#[test]
fn read_never_returns_more_than_asked() {
    let mut session = open(vec!["/bin/sh".to_owned()]);
    send_line(&mut session, "echo abcdef").expect("send");
    let out = read_until(&mut session, "abcdef", Duration::from_secs(10));
    assert!(out.len() >= 6);
    let capped = session.read(3);
    assert!(capped.len() <= 3);
}

#[test]
fn oversized_input_is_refused() {
    let mut session = open(vec!["/bin/sh".to_owned()]);
    let error = session
        .send(&vec![b'x'; INPUT_MAX_BYTES + 1])
        .expect_err("oversized input is refused");
    assert!(matches!(error, PtyError::InputTooLarge { .. }), "{error}");
}

#[test]
fn close_kills_the_program() {
    let session = open(vec!["sleep".to_owned(), "300".to_owned()]);
    let pid = session.pid().expect("a pid");
    let exit = session.close().expect("close");
    assert!(exit.is_some(), "the killed program reported an exit");
    // The pid is gone: a fresh wait cannot see it.
    let reaped = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            &format!("kill -0 {pid} 2>/dev/null && echo alive || echo gone"),
        ])
        .output()
        .expect("probe runs");
    assert_eq!(String::from_utf8_lossy(&reaped.stdout).trim(), "gone");
}

#[test]
fn dropping_a_session_kills_the_program() {
    let mut session = open(vec!["sleep".to_owned(), "300".to_owned()]);
    let pid = session.pid().expect("a pid");
    // Drain nothing; just drop it.
    session.send(b"\x03").expect("ctrl-c sends");
    drop(session);
    std::thread::sleep(Duration::from_millis(200));
    let reaped = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            &format!("kill -0 {pid} 2>/dev/null && echo alive || echo gone"),
        ])
        .output()
        .expect("probe runs");
    assert_eq!(String::from_utf8_lossy(&reaped.stdout).trim(), "gone");
}

#[test]
fn resize_reaches_the_program() {
    let mut session = open(python());
    let _ = read_until(&mut session, ">>>", Duration::from_secs(10));
    session.resize(40, 132).expect("resize");
    send_line(
        &mut session,
        "import shutil; print(shutil.get_terminal_size().columns)",
    )
    .expect("send");
    let out = read_until(&mut session, "132", Duration::from_secs(10));
    assert!(
        String::from_utf8_lossy(&out).contains("132"),
        "the program saw the new size: {:?}",
        String::from_utf8_lossy(&out)
    );
    let _ = session.close();
}

#[test]
fn the_cwd_and_extra_env_reach_the_program() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut options = options();
    options.cwd = Some(PathBuf::from(dir.path()));
    options.env = vec![("AULO_PTY".to_owned(), "set".to_owned())];
    let mut session = PtySession::open(&["/bin/sh".to_owned()], &policy(), &[], &[], &options)
        .expect("session opens");
    send_line(&mut session, "pwd; printf %s \"$AULO_PTY\"").expect("send");
    let out = read_until(&mut session, "set", Duration::from_secs(10));
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("set"), "the extra env arrived: {text:?}");
    let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
    assert!(
        text.contains(&canonical.display().to_string()),
        "the cwd arrived: {text:?}"
    );
    let _ = session.close();
}

#[test]
fn an_empty_program_is_refused() {
    let error =
        PtySession::open(&[], &policy(), &[], &[], &options()).expect_err("no program to run");
    assert!(matches!(error, PtyError::NoProgram), "{error}");
}
