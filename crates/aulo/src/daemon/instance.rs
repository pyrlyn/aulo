//! One daemon per `AULO_HOME`: an exclusive lock on `run/aulod.lock`, plus a
//! pidfile that tells the next instance (and the user) who holds it.

use std::fs::{self, DirBuilder, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Held for the life of the process. The lock is released by the OS when the
/// process dies, however it dies; dropping this removes the pidfile on a
/// clean exit.
#[derive(Debug)]
pub struct Instance {
    _lock: File,
    pidfile: PathBuf,
}

impl Instance {
    pub fn acquire(home: &Path) -> Result<Self> {
        let socket = aulo_server::local_socket_path(home);
        let dir = socket
            .parent()
            .context("the socket path has no directory")?;
        create_run_dir(dir).with_context(|| format!("cannot create {}", dir.display()))?;
        let lock_path = socket.with_file_name("aulod.lock");
        let pidfile = socket.with_file_name("aulod.pid");

        let lock = open_lock(&lock_path)
            .with_context(|| format!("cannot open {}", lock_path.display()))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => bail!(
                "already running (pid {}, home {}); stop it first or use another AULO_HOME",
                running_pid(&pidfile),
                home.display()
            ),
            Err(TryLockError::Error(e)) => {
                return Err(e).with_context(|| format!("cannot lock {}", lock_path.display()));
            }
        }
        fs::write(&pidfile, format!("{}\n", std::process::id()))
            .with_context(|| format!("cannot write {}", pidfile.display()))?;
        Ok(Self {
            _lock: lock,
            pidfile,
        })
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_file(&self.pidfile) {
            tracing::warn!(path = %self.pidfile.display(), error = %e, "could not remove the pidfile");
        }
        // The lock file stays: deleting it would let a new instance lock a
        // fresh file while a slow one still holds the old inode.
    }
}

/// Owner-only like the socket's directory, which `aulo-server` insists on.
fn create_run_dir(dir: &Path) -> std::io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}

fn open_lock(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    // Never truncate: another process may hold and rely on the file.
    options.create(true).write(true).truncate(false);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// The holder may not have written its pidfile yet, or may have died and left
/// a stale one that the lock already proves wrong, so this is a hint only.
fn running_pid(pidfile: &Path) -> String {
    fs::read_to_string(pidfile)
        .ok()
        .and_then(|text| text.trim().parse::<u32>().ok())
        .map_or_else(|| "unknown".to_owned(), |pid| pid.to_string())
}
