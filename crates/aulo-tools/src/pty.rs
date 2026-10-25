//! Interactive PTY sessions (spec §6.1, T9.2).
//!
//! A session runs an interactive program — a shell, `python -i`, an editor —
//! on a pseudo-terminal: open, send, read, close, resize. The session's
//! buffer is capped: output the host has not read yet is dropped at the
//! front of a fixed ring and counted, so a program that prints forever
//! cannot grow the daemon's memory.
//!
//! Policy integration point: a session is `Shell(cmd)` under spec §6.1,
//! exactly as the one-shot shell tool: the host decides with
//! [`aulo_policy::shell_call`] / `decide_shell` before opening one. The
//! program argv is wrapped by `proc-sandbox` like the shell tool's command
//! (its documented use: an interactive login shell in a PTY).

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use proc_sandbox::policy::SandboxPolicy;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapRb};

/// Bytes one `send` may carry. Larger input is refused, not chunked: a
/// paste the model meant as one write is one write.
pub const INPUT_MAX_BYTES: usize = 64 * 1024;

/// Bytes of unread output the session keeps. Older output is dropped and
/// counted; the host reads with [`PtySession::read`].
pub const BUFFER_MAX_BYTES: usize = 256 * 1024;

/// How often the reader thread looks at the ring while draining.
const READ_CHUNK: usize = 16 * 1024;

/// How long `close` waits for the reader thread after the kill.
const CLOSE_GRACE: Duration = Duration::from_secs(2);

/// What a session is opened with.
#[derive(Debug, Clone)]
pub struct PtyOptions {
    /// Terminal rows.
    pub rows: u16,
    /// Terminal columns.
    pub cols: u16,
    /// Working directory; the host confines it like the shell tool's cwd.
    pub cwd: Option<PathBuf>,
    /// Environment on top of the allowlist; the host decides what may go
    /// here.
    pub env: Vec<(String, String)>,
    /// Environment names the child inherits; the rest is cleared.
    pub env_allowlist: Vec<String>,
}

impl Default for PtyOptions {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            cwd: None,
            env: Vec::new(),
            env_allowlist: vec![
                "PATH".to_owned(),
                "HOME".to_owned(),
                "LANG".to_owned(),
                "LC_ALL".to_owned(),
                "LC_CTYPE".to_owned(),
                "TERM".to_owned(),
                "TMPDIR".to_owned(),
                "USER".to_owned(),
                "SHELL".to_owned(),
            ],
        }
    }
}

/// Why a session could not be opened or driven.
#[derive(Debug, thiserror::Error)]
pub enum PtyError {
    /// The program argv is empty.
    #[error("no program to run")]
    NoProgram,
    /// The sandbox could not wrap the argv (Landlock cannot wrap an argv).
    #[error("sandbox: {0}")]
    Sandbox(std::io::Error),
    /// The pty could not be opened.
    #[error("pty: {0}")]
    Open(String),
    /// The program could not be spawned.
    #[error("spawn: {0}")]
    Spawn(String),
    /// Input over [`INPUT_MAX_BYTES`].
    #[error("input is {got} bytes, over the {max} byte cap")]
    InputTooLarge {
        /// Bytes received.
        got: usize,
        /// The cap.
        max: usize,
    },
    /// The pty refused a write, read or resize.
    #[error("pty io: {0}")]
    Io(String),
}

/// How a session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtyExit {
    /// The exit code, or `None` when a signal killed the program.
    pub code: Option<i32>,
    /// The signal name when one killed it.
    pub signal: Option<String>,
}

/// An open session. Dropping it kills the program.
pub struct PtySession {
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Option<Box<dyn std::io::Write + Send>>,
    consumer: HeapCons<u8>,
    dropped: Arc<AtomicU64>,
    exited: Arc<AtomicBool>,
    exit: Arc<std::sync::Mutex<Option<PtyExit>>>,
    done: Receiver<()>,
}

impl std::fmt::Debug for PtySession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtySession")
            .field("pid", &self.pid())
            .field("buffered", &self.consumer.occupied_len())
            .finish_non_exhaustive()
    }
}

impl PtySession {
    /// Opens `program` (an absolute path and its arguments) on a pty under
    /// `policy`.
    ///
    /// # Errors
    ///
    /// [`PtyError`]; an empty argv, a sandbox that cannot wrap the argv, a
    /// pty that will not open, or a program that will not spawn.
    pub fn open(
        program: &[String],
        policy: &SandboxPolicy,
        roots: &[PathBuf],
        writable_roots: &[PathBuf],
        options: &PtyOptions,
    ) -> Result<Self, PtyError> {
        if program.is_empty() {
            return Err(PtyError::NoProgram);
        }
        // The sandbox wraps the argv exactly as it wraps the shell tool's
        // `<shell> -c` (proc-sandbox's documented PTY use).
        let argv = proc_sandbox::sandbox::argv(policy, roots, writable_roots, program)
            .map_err(PtyError::Sandbox)?;
        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system
            .openpty(portable_pty::PtySize {
                rows: options.rows,
                cols: options.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Open(e.to_string()))?;

        let mut builder = portable_pty::CommandBuilder::new(&argv[0]);
        builder.args(&argv[1..]);
        builder.env_clear();
        for name in &options.env_allowlist {
            if let Ok(value) = std::env::var(name) {
                builder.env(name, value);
            }
        }
        for (name, value) in &options.env {
            builder.env(name, value);
        }
        if let Some(cwd) = &options.cwd {
            builder.cwd(cwd);
        }
        let child = pair
            .slave
            .spawn_command(builder)
            .map_err(|e| PtyError::Spawn(e.to_string()))?;
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Open(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::Open(e.to_string()))?;

        let ring: HeapRb<u8> = HeapRb::new(BUFFER_MAX_BYTES);
        let (mut prod, cons) = ring.split();
        let dropped = Arc::new(AtomicU64::new(0));
        let exited = Arc::new(AtomicBool::new(false));
        let (done_tx, done_rx) = mpsc::channel();
        let reader_dropped = Arc::clone(&dropped);
        let reader_exited = Arc::clone(&exited);
        std::thread::spawn(move || {
            let mut chunk = [0u8; READ_CHUNK];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        for byte in &chunk[..n] {
                            if prod.try_push(*byte).is_err() {
                                reader_dropped.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            reader_exited.store(true, Ordering::Release);
            let _ = done_tx.send(());
        });

        Ok(Self {
            child: Some(child),
            master: pair.master,
            writer: Some(writer),
            consumer: cons,
            dropped,
            exited,
            exit: Arc::new(std::sync::Mutex::new(None)),
            done: done_rx,
        })
    }

    /// The child's process id, when the platform names one.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(|c| c.process_id())
    }

    /// Bytes buffered and unread.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.consumer.occupied_len()
    }

    /// Bytes dropped because the buffer was full and nobody read.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Whether the pty reached EOF: the program closed its side, so no
    /// more output will arrive.
    #[must_use]
    pub fn at_eof(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    /// Sends input to the program.
    ///
    /// # Errors
    ///
    /// [`PtyError::InputTooLarge`] over [`INPUT_MAX_BYTES`], or a pty that
    /// refused the write.
    pub fn send(&mut self, input: &[u8]) -> Result<(), PtyError> {
        if input.len() > INPUT_MAX_BYTES {
            return Err(PtyError::InputTooLarge {
                got: input.len(),
                max: INPUT_MAX_BYTES,
            });
        }
        let Some(writer) = self.writer.as_mut() else {
            return Err(PtyError::Io("the session is closed".to_owned()));
        };
        writer
            .write_all(input)
            .and_then(|()| writer.flush())
            .map_err(|e| PtyError::Io(e.to_string()))
    }

    /// Drains up to `max` buffered bytes. Never blocks.
    #[must_use]
    pub fn read(&mut self, max: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(max.min(self.consumer.occupied_len()));
        while out.len() < max {
            match self.consumer.try_pop() {
                Some(byte) => out.push(byte),
                None => break,
            }
        }
        out
    }

    /// Tells the program the window changed size.
    ///
    /// # Errors
    ///
    /// The pty refused the resize.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), PtyError> {
        self.master
            .resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Io(e.to_string()))
    }

    /// The exit status once the program has ended, polling without
    /// blocking.
    #[must_use]
    pub fn poll_exit(&mut self) -> Option<PtyExit> {
        if let Ok(slot) = self.exit.lock()
            && let Some(exit) = slot.as_ref()
        {
            return Some(exit.clone());
        }
        let child = self.child.as_mut()?;
        match child.try_wait() {
            Ok(Some(status)) => {
                let exit = PtyExit {
                    code: status
                        .signal()
                        .map(|_| None)
                        .unwrap_or(Some(status.exit_code() as i32)),
                    signal: status.signal().map(str::to_owned),
                };
                if let Ok(mut slot) = self.exit.lock() {
                    *slot = Some(exit.clone());
                }
                Some(exit)
            }
            _ => None,
        }
    }

    /// Blocks until the program ends or `timeout` passes.
    #[must_use]
    pub fn wait_exit(&mut self, timeout: Duration) -> Option<PtyExit> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if let Some(exit) = self.poll_exit() {
                return Some(exit);
            }
            if std::time::Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Kills the program — the whole tree on Unix, through the pty's
    /// process-group leader — sends EOF, and waits briefly for the reader.
    ///
    /// # Errors
    ///
    /// The pty refused the kill.
    pub fn close(mut self) -> Result<Option<PtyExit>, PtyError> {
        self.kill();
        let _ = self.done.recv_timeout(CLOSE_GRACE);
        Ok(self.poll_exit())
    }

    /// Kills the program without consuming the session.
    fn kill(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
        }
        // On Unix the program is its own process-group leader, so a shell
        // session's children die with it.
        #[cfg(unix)]
        if let Some(leader) = self.master.process_group_leader()
            && let Some(pid) = rustix::process::Pid::from_raw(leader)
        {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
        // Dropping the writer sends EOF to anything still holding the pty.
        self.writer.take();
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if self.poll_exit().is_none() {
            self.kill();
        }
    }
}

/// Reads until `needle` appears in the session's output or the deadline
/// passes. Returns everything drained. A test helper for driving an
/// interactive program.
#[must_use]
pub fn read_until(session: &mut PtySession, needle: &str, timeout: Duration) -> Vec<u8> {
    let deadline = std::time::Instant::now() + timeout;
    let mut seen = Vec::new();
    loop {
        let chunk = session.read(READ_CHUNK);
        if !chunk.is_empty() {
            seen.extend_from_slice(&chunk);
            if String::from_utf8_lossy(&seen).contains(needle) {
                return seen;
            }
        }
        if std::time::Instant::now() >= deadline {
            return seen;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Sends a line and waits for the program to answer.
///
/// # Errors
///
/// The pty refused the write.
pub fn send_line(session: &mut PtySession, line: &str) -> Result<(), PtyError> {
    let mut input = line.as_bytes().to_vec();
    input.push(b'\r');
    session.send(&input)
}
