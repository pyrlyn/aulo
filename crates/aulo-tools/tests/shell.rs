//! T9.1: the shell tool runs with a timeout, an output cap and a lossless
//! archive; the cwd is confined and the environment is an allowlist.
//!
//! The functional tests run with `danger-full-access`: this host applies
//! its own macOS sandbox to the test process, and a nested restrictive
//! Seatbelt profile is refused with `sandbox_apply: Operation not
//! permitted`, which would fail every run for the wrong reason. The
//! confinement test uses the real workspace-write profile and skips itself
//! when the host cannot nest sandboxes (or has no backend at all).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aulo_tools::shell::{
    DEFAULT_TIMEOUT, OutputArchive, OutputEvent, RunRequest, ShellError, ShellTool, StreamView,
    policy_call,
};
use proc_sandbox::policy::{LinuxBackend, SandboxMode, SandboxPolicy};
use proc_sandbox::sandbox;

/// An environment that carries only PATH, HOME and a secret no allowlist
/// names.
type Env = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

fn env_source() -> Env {
    let mut env = HashMap::new();
    env.insert("PATH".to_owned(), std::env::var("PATH").unwrap_or_default());
    env.insert("HOME".to_owned(), std::env::var("HOME").unwrap_or_default());
    env.insert("SECRET_TOKEN".to_owned(), "s3cret".to_owned());
    Arc::new(move |key: &str| env.get(key).cloned())
}

fn tool_with(dir: &Path, mode: SandboxMode) -> ShellTool {
    ShellTool::new(
        SandboxPolicy {
            mode,
            network: false,
            writable: vec![],
            readonly_in_workspace: vec![],
            linux_backend: LinuxBackend::Auto,
        },
        vec![dir.to_path_buf()],
        vec![dir.to_path_buf()],
        PathBuf::from("/bin/sh"),
        dir.to_path_buf(),
        env_source(),
        dir.join("archives"),
    )
}

/// The tool without confinement, for tests of everything but the sandbox.
fn tool(dir: &Path) -> ShellTool {
    tool_with(dir, SandboxMode::DangerFullAccess)
}

fn request(command: &str) -> RunRequest<'_> {
    RunRequest {
        command,
        cwd: None,
        timeout: None,
        env: &[],
    }
}

#[test]
fn a_command_runs_and_reports_its_exit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tool = tool(dir.path());
    let outcome = tool.run(&request("echo hello")).expect("run");
    assert_eq!(outcome.code, Some(0));
    assert!(!outcome.timed_out);
    assert_eq!(outcome.stdout.text.trim_end(), "hello");
    assert_eq!(outcome.stderr.text, "");
    let failed = tool.run(&request("exit 3")).expect("run");
    assert_eq!(failed.code, Some(3));
}

#[test]
fn the_timeout_kills_the_process_tree() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tool = tool(dir.path());
    // Two sleeps: the shell and a grandchild that both hold the stdout pipe
    // open. Killing only the shell would leave the pipe open for 300 s and
    // this call would block; the group kill closes it at once.
    let started = Instant::now();
    let outcome = tool
        .run(&RunRequest {
            command: "sleep 300 & sleep 300",
            cwd: None,
            timeout: Some(Duration::from_millis(300)),
            env: &[],
        })
        .expect("run");
    assert!(outcome.timed_out);
    assert_eq!(outcome.code, None, "a killed tree has no exit code");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the tree died with the kill; the run took {:?}",
        started.elapsed()
    );
}

#[test]
fn truncated_output_is_recoverable() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tool = tool(dir.path());
    let outcome = tool.run(&request("seq 1 20000")).expect("run");
    assert_eq!(outcome.code, Some(0));
    assert!(outcome.stdout.truncated);
    assert!(outcome.stdout.text.len() <= 32 * 1024);
    assert!(outcome.stdout.total_bytes > 64 * 1024);
    let id = outcome.stdout.archive.expect("the full output is archived");
    let total = tool.archive().len(&id).expect("archive size");
    assert_eq!(total, outcome.stdout.total_bytes);
    // The tail is recoverable from the archive even though the view cut it.
    let tail = tool
        .archive()
        .read(&id, total - 6, 6)
        .expect("read the archive tail");
    assert_eq!(tail, b"20000\n");
}

#[test]
fn the_cwd_is_confined_to_the_workspace() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tool = tool(dir.path());
    let error = tool
        .run(&RunRequest {
            command: "pwd",
            cwd: Some(Path::new("/etc")),
            timeout: None,
            env: &[],
        })
        .expect_err("/etc is outside the workspace");
    assert!(matches!(error, ShellError::Cwd(_)), "{error}");
    let outcome = tool
        .run(&RunRequest {
            command: "pwd",
            cwd: Some(dir.path()),
            timeout: None,
            env: &[],
        })
        .expect("the workspace is a fine cwd");
    assert_eq!(outcome.code, Some(0));
    // The tool canonicalized the root, so the shell's physical pwd matches.
    let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
    assert_eq!(
        outcome.stdout.text.trim_end(),
        canonical.display().to_string()
    );
}

#[test]
fn the_env_allowlist_hides_everything_else() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tool = tool(dir.path());
    let outcome = tool
        .run(&request("printf %s \"$SECRET_TOKEN\""))
        .expect("run");
    assert_eq!(
        outcome.stdout.text, "",
        "a variable outside the allowlist never reaches the command"
    );
    let outcome = tool
        .run(&RunRequest {
            command: "printf %s \"$GREETING\"",
            cwd: None,
            timeout: None,
            env: &[("GREETING".to_owned(), "hi".to_owned())],
        })
        .expect("run");
    assert_eq!(
        outcome.stdout.text, "hi",
        "the host's own extras pass through"
    );
}

#[test]
fn the_policy_call_carries_the_shell_subject() {
    // The integration point: the host decides with aulo-policy and only
    // runs an Allow. `rm -rf` is Destructive and asks by default (T5.2).
    let call = policy_call("rm -rf");
    assert_eq!(call.tool, "bash");
    assert_eq!(call.subject, "rm -rf");
    let outcome = aulo_policy::decide_shell("rm -rf", &aulo_policy::RuleSet::default())
        .expect("empty rules compile");
    assert!(
        matches!(outcome, aulo_policy::Outcome::Ask(_)),
        "{outcome:?}"
    );
}

/// Whether this host can actually confine a command: a backend exists and
/// applying its profile works. A nested sandbox is refused, so an agent
/// session (or a host with no bwrap and no Landlock) cannot.
fn confinement_available(tool: &ShellTool) -> bool {
    if sandbox::backend(LinuxBackend::Auto).is_none() {
        return false;
    }
    tool.run(&request("true"))
        .map(|outcome| outcome.code == Some(0))
        .unwrap_or(false)
}

#[test]
fn writes_inside_the_workspace_run_and_outside_are_denied() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tool = tool_with(dir.path(), SandboxMode::WorkspaceWrite);
    if !confinement_available(&tool) {
        // No backend, or this process is already sandboxed and cannot
        // apply a nested profile. The argv composition test below still
        // proves the wrapper is built.
        eprintln!("skipped: this host cannot confine a command");
        return;
    }
    let inside = tool.run(&request("touch marker")).expect("run");
    assert_eq!(inside.code, Some(0), "stderr: {}", inside.stderr.text);
    assert!(dir.path().join("marker").exists());

    let forbidden = dir.path().join("forbidden");
    let outside = tool
        .run(&request(&format!(
            "touch {}/marker && echo WROTE",
            forbidden.display()
        )))
        .expect("run");
    assert_ne!(outside.code, Some(0), "a write outside the roots must fail");
    assert!(!forbidden.join("marker").exists());
}

#[test]
fn the_sandbox_wraps_the_command_argv() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tool = tool_with(dir.path(), SandboxMode::WorkspaceWrite);
    let argv = tool.command_argv("echo hi").expect("argv builds");
    match sandbox::backend(LinuxBackend::Auto) {
        Some(sandbox::Backend::Seatbelt) => {
            assert_eq!(argv[0], "/usr/bin/sandbox-exec", "{argv:?}");
            assert!(argv.contains(&"-p".to_owned()), "{argv:?}");
            assert!(argv.contains(&"--".to_owned()), "{argv:?}");
            assert_eq!(argv[argv.len() - 3..], ["/bin/sh", "-c", "echo hi"]);
        }
        Some(sandbox::Backend::Bwrap) => {
            assert_eq!(argv[0], "bwrap", "{argv:?}");
            assert_eq!(argv[argv.len() - 3..], ["/bin/sh", "-c", "echo hi"]);
        }
        // Landlock wraps in the child's pre_exec, and no backend leaves the
        // command bare.
        Some(sandbox::Backend::Landlock) | None => {
            assert_eq!(argv, ["/bin/sh", "-c", "echo hi"]);
        }
    }
}

#[test]
fn streaming_events_arrive_in_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tool = tool(dir.path());
    let (tx, rx) = std::sync::mpsc::channel();
    let outcome = tool
        .run_streaming(&request("echo one; echo two"), tx)
        .expect("run");
    let mut chunks = Vec::new();
    let mut exit = None;
    for event in rx {
        match event {
            OutputEvent::Chunk { stream, bytes } => {
                assert_eq!(stream, aulo_tools::shell::Stream::Stdout);
                chunks.push(String::from_utf8_lossy(&bytes).into_owned());
            }
            OutputEvent::Exit { code, timed_out } => {
                exit = Some((code, timed_out));
            }
        }
    }
    assert_eq!(exit, Some((Some(0), false)));
    assert_eq!(chunks.join("").trim_end(), "one\ntwo");
    assert_eq!(outcome.stdout.text.trim_end(), "one\ntwo");
}

#[test]
fn an_empty_or_oversized_command_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tool = tool(dir.path());
    assert!(matches!(
        tool.run(&request("")),
        Err(ShellError::Command { .. })
    ));
    let long = "echo ".to_owned() + &"x".repeat(aulo_policy::SHELL_COMMAND_MAX_BYTES);
    assert!(matches!(
        tool.run(&request(&long)),
        Err(ShellError::Command { .. })
    ));
}

#[test]
fn the_archive_rejects_an_untrusted_id() {
    let dir = tempfile::tempdir().expect("temp dir");
    let archive = OutputArchive::new(dir.path().join("archives"));
    assert!(aulo_tools::shell::ArchiveId::parse("../../etc/passwd").is_none());
    assert!(aulo_tools::shell::ArchiveId::parse("abc-123").is_some());
    assert!(
        archive
            .len(&aulo_tools::shell::ArchiveId::parse("abc-123").expect("valid"))
            .is_err()
    );
}

#[test]
fn a_run_without_output_archives_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tool = tool(dir.path());
    let outcome = tool.run(&request("true")).expect("run");
    assert_eq!(outcome.code, Some(0));
    assert_eq!(outcome.stdout, StreamView::default());
}

#[test]
fn the_default_timeout_is_two_minutes() {
    assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(120));
}
