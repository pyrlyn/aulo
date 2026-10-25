//! T5.7: a routine run cannot write a file, and a read still works.

use std::fs;

use aulo_unattended::{
    AccessError, Gate, Presence, RunKind, ToolKind, gate, read_file, write_file,
};

#[test]
fn a_routine_cannot_write_a_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("out.txt");
    let err = write_file(Presence::Attached, RunKind::Routine, &path, b"hello")
        .expect_err("routine write");
    assert_eq!(err, AccessError::Denied);
    assert!(!path.exists());
}

#[test]
fn a_detached_run_cannot_write_either() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("out.txt");
    let err = write_file(Presence::Detached, RunKind::Interactive, &path, b"hello")
        .expect_err("detached write");
    assert_eq!(err, AccessError::Denied);
    assert!(!path.exists());
}

#[test]
fn an_attached_interactive_run_can_write_and_a_routine_can_read_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("out.txt");
    write_file(Presence::Attached, RunKind::Interactive, &path, b"hello").expect("write");
    assert_eq!(fs::read(&path).expect("read back"), b"hello");
    let got = read_file(Presence::Detached, RunKind::Routine, &path).expect("routine read");
    assert_eq!(got, b"hello");
}

#[test]
fn exec_and_input_are_denied_while_reads_are_not() {
    assert_eq!(
        gate(Presence::Attached, RunKind::Routine, ToolKind::Exec),
        Gate::Deny
    );
    assert_eq!(
        gate(Presence::Detached, RunKind::Interactive, ToolKind::Input),
        Gate::Deny
    );
    assert_eq!(
        gate(Presence::Detached, RunKind::Routine, ToolKind::Read),
        Gate::Allow
    );
    assert_eq!(
        gate(Presence::Attached, RunKind::Interactive, ToolKind::Write),
        Gate::Allow
    );
}
