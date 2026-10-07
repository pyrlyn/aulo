//! `aulod` command-line output, as `trycmd` fixtures in `tests/aulod/`.
// Helpers outside #[test] fns are not covered by allow-unwrap-in-tests.
#![allow(clippy::unwrap_used)]

use std::fs::{self, File};

#[test]
fn cli() {
    // The second-instance fixture needs a home whose lock is already taken, so
    // the test plays the first instance. The pid in the pidfile is made up:
    // the fixture only checks that the message reports what the pidfile says.
    let home = tempfile::tempdir().unwrap();
    let run = home.path().join("run");
    fs::create_dir_all(&run).unwrap();
    fs::write(run.join("aulod.pid"), "4242\n").unwrap();
    let lock = File::create(run.join("aulod.lock")).unwrap();
    lock.try_lock().unwrap();

    trycmd::TestCases::new()
        .env("AULO_HOME", home.path().to_str().unwrap())
        .case("tests/aulod/*.toml");

    drop(lock);
}
