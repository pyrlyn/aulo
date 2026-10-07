//! Manual check of the real permission probe; ignored by default because CI has
//! no TCC database or consent store worth asserting on. The probe only reads
//! the state, so it never shows a system dialog.
//!
//! `cargo test -p aulo-audio --test mic_permission -- --ignored --nocapture`

use aulo_audio::{MicPermissionProbe, Platform, SystemProbe};

#[test]
#[ignore = "reports this machine's microphone permission"]
fn the_system_probe_reports_this_machines_microphone_permission() {
    let status = SystemProbe.status();
    println!("microphone permission: {status}");
    match status.notice(Platform::current()) {
        Some(notice) => println!("notice: {notice:?}"),
        None => println!("no notice"),
    }
}
