//! The shell tool: run a command inside the sandbox with a timeout, an
//! output cap, streamed output events, a confined cwd and an env allowlist
//! (spec §6.1, T9.1).
//!
//! The full output of every run is archived losslessly under the archive
//! root; the model sees a truncated view plus the archive id, the cox
//! `expand` pattern. A run that outlives its timeout has its whole process
//! tree killed: the child is a process-group leader on Unix, so a shell
//! that spawned grandchildren cannot outlive the decision to stop it.
//!
//! Policy integration point: [`policy_call`] builds the `aulo-policy`
//! `perm-rules` call for a command line. The host runs it through policy
//! and only calls [`ShellTool::run`] on an `Allow`; the sandbox mode the
//! host passes in reflects the outcome (T5.4 owns the profiles).

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use proc_sandbox::path::ConfineError;
use proc_sandbox::policy::SandboxPolicy;
use proc_sandbox::sandbox;

/// Environment variables a command inherits unless the host says otherwise.
/// Everything else — API keys above all — stays behind.
pub const DEFAULT_ENV_ALLOWLIST: &[&str] = &[
    "PATH", "HOME", "LANG", "LC_ALL", "LC_CTYPE", "TERM", "TMPDIR", "USER", "SHELL",
];

/// Bytes of one stream the model is shown. Longer output is truncated in
/// the view and recoverable from the archive.
pub const OUTPUT_VIEW_MAX_BYTES: usize = 32 * 1024;

/// Bytes of one stream kept in the archive. A command that outproduces this
/// has its archive cut and the outcome says so; the view was already
/// truncated long before.
pub const ARCHIVE_MAX_BYTES: u64 = 256 * 1024 * 1024;

/// Default command timeout when the host names none.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// How long a killed tree gets to close its pipes after the timeout kill.
const KILL_GRACE: Duration = Duration::from_secs(5);

/// Read chunk size from the child's pipes.
const READ_CHUNK: usize = 16 * 1024;

/// Where the tool reads environment variables from. The daemon passes its
/// own view; tests inject a map.
pub type EnvSource = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// The process environment.
pub fn process_env() -> EnvSource {
    Arc::new(|key| std::env::var(key).ok())
}

/// An archive id: the handle the model uses to expand output. Validated on
/// read so a model-supplied id cannot name another file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArchiveId(String);

impl ArchiveId {
    /// Parses an id the model or a client supplied. `None` for anything
    /// that is not a plain id, so a hostile id never reaches the
    /// filesystem.
    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        Self::valid(id).then(|| Self(id.to_owned()))
    }

    /// The id as the model sees it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn valid(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 64
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    }
}

impl std::fmt::Display for ArchiveId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which stream a chunk or an archive belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

impl Stream {
    /// The archive file suffix.
    fn suffix(self) -> &'static str {
        match self {
            Stream::Stdout => "stdout",
            Stream::Stderr => "stderr",
        }
    }
}

/// A streamed output event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputEvent {
    /// A chunk of one stream arrived.
    Chunk {
        /// Which stream.
        stream: Stream,
        /// The bytes; not necessarily valid UTF-8.
        bytes: Vec<u8>,
    },
    /// The command exited or was killed.
    Exit {
        /// The exit status, or `None` when it was killed by a signal.
        code: Option<i32>,
        /// Whether the timeout killed it.
        timed_out: bool,
    },
}

/// The full output of one stream, archived losslessly.
#[derive(Debug, Clone, Default)]
pub struct OutputArchive {
    root: PathBuf,
}

impl OutputArchive {
    /// Archives under `root`, created on first write.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The root archives live under.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Creates the archive file for `id` and `stream`, ready to append to.
    ///
    /// # Errors
    ///
    /// The archive root cannot be created or the file cannot be opened.
    pub fn create(&self, id: &ArchiveId, stream: Stream) -> io::Result<PathBuf> {
        std::fs::create_dir_all(&self.root)?;
        let path = self
            .root
            .join(format!("{}.{}", id.as_str(), stream.suffix()));
        std::fs::File::create(&path)?;
        Ok(path)
    }

    /// Reads `len` bytes at `start` from an archive.
    ///
    /// # Errors
    ///
    /// The id is not a valid [`ArchiveId`], or the file cannot be read.
    pub fn read(&self, id: &ArchiveId, start: u64, len: usize) -> io::Result<Vec<u8>> {
        if !ArchiveId::valid(id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "archive id is not valid",
            ));
        }
        // Try the stream the caller most likely means first; both files
        // share the id.
        for stream in [Stream::Stdout, Stream::Stderr] {
            let path = self
                .root
                .join(format!("{}.{}", id.as_str(), stream.suffix()));
            let meta = match std::fs::metadata(&path) {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            if meta.len() <= start {
                continue;
            }
            use std::io::Seek;
            let mut file = std::fs::File::open(&path)?;
            file.seek(io::SeekFrom::Start(start))?;
            let mut buf = vec![0u8; len];
            let read = file.read(&mut buf)?;
            buf.truncate(read);
            return Ok(buf);
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no archive under that id",
        ))
    }

    /// The archived size of `id`'s stream, whichever one exists.
    ///
    /// # Errors
    ///
    /// The id is not a valid [`ArchiveId`], or no archive has that id.
    pub fn len(&self, id: &ArchiveId) -> io::Result<u64> {
        if !ArchiveId::valid(id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "archive id is not valid",
            ));
        }
        for stream in [Stream::Stdout, Stream::Stderr] {
            let path = self
                .root
                .join(format!("{}.{}", id.as_str(), stream.suffix()));
            if let Ok(meta) = std::fs::metadata(&path) {
                return Ok(meta.len());
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no archive under that id",
        ))
    }
}

/// Why a run could not start or finish.
#[derive(Debug, thiserror::Error)]
pub enum ShellError {
    /// The command is empty or over [`aulo_policy::SHELL_COMMAND_MAX_BYTES`].
    #[error("command is empty or over the {max} byte cap")]
    Command {
        /// The cap that was exceeded.
        max: usize,
    },
    /// The cwd escapes the workspace roots.
    #[error("cwd: {0}")]
    Cwd(#[from] ConfineError),
    /// The sandbox could not wrap the command.
    #[error("sandbox: {0}")]
    Sandbox(io::Error),
    /// The command could not be spawned.
    #[error("spawn: {0}")]
    Spawn(io::Error),
    /// The archive could not be written.
    #[error("archive: {0}")]
    Archive(io::Error),
    /// A reader thread joined badly; the run is unusable.
    #[error("output reader failed")]
    Reader,
}

/// What one run asked for.
#[derive(Debug, Clone, Default)]
pub struct RunRequest<'a> {
    /// The command line, run as `<shell> -c <command>`.
    pub command: &'a str,
    /// Working directory; confined to the workspace roots. `None` is the
    /// tool's default cwd.
    pub cwd: Option<&'a Path>,
    /// Timeout; `None` is [`DEFAULT_TIMEOUT`].
    pub timeout: Option<Duration>,
    /// Extra environment variables, on top of the allowlist. The host
    /// decides what may go here.
    pub env: &'a [(String, String)],
}

/// One stream as the model sees it: the capped head, plus the archive id
/// when the full output is on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamView {
    /// The head of the stream, at most [`OUTPUT_VIEW_MAX_BYTES`] bytes,
    /// decoded lossily.
    pub text: String,
    /// Whether the full stream was longer than the view.
    pub truncated: bool,
    /// The archive id, when anything was archived.
    pub archive: Option<ArchiveId>,
    /// Total bytes the stream produced.
    pub total_bytes: u64,
}

/// How a run ended.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// The exit status; `None` when a signal killed the command.
    pub code: Option<i32>,
    /// Whether the timeout killed the tree.
    pub timed_out: bool,
    /// How long the run took.
    pub duration: Duration,
    /// The sandbox backend that confined it, or `None` when unconfined.
    pub backend: Option<&'static str>,
    /// Standard output.
    pub stdout: StreamView,
    /// Standard error.
    pub stderr: StreamView,
}

/// The `perm-rules` call a command line is judged by (T5.2): tool `bash`,
/// subject the line, risk from `shell-classify`.
#[must_use]
pub fn policy_call(command: &str) -> aulo_policy::Call {
    aulo_policy::shell_call(command)
}

/// Stats one reader thread reports when its pipe reached EOF.
struct StreamStats {
    total_bytes: u64,
    view: Vec<u8>,
    archive_truncated: bool,
}

/// The shell tool. Cheap to clone behind an `Arc`; the daemon holds one.
pub struct ShellTool {
    policy: SandboxPolicy,
    roots: Vec<PathBuf>,
    writable_roots: Vec<PathBuf>,
    shell: PathBuf,
    default_cwd: PathBuf,
    env_allowlist: Vec<String>,
    env_source: EnvSource,
    archive: OutputArchive,
    timeout: Duration,
}

impl std::fmt::Debug for ShellTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShellTool")
            .field("shell", &self.shell)
            .field("default_cwd", &self.default_cwd)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl ShellTool {
    /// Builds the tool.
    ///
    /// - `policy` / `roots` / `writable_roots` — the sandbox profile (T5.4
    ///   owns the profiles; the host maps a policy outcome onto these).
    /// - `shell` — an absolute path the host resolved from its own
    ///   allowlist; the sandbox never looks a program up on `PATH`.
    /// - `default_cwd` — where a run without a cwd starts.
    /// - `archive_root` — where full output is kept.
    #[must_use]
    pub fn new(
        policy: SandboxPolicy,
        roots: Vec<PathBuf>,
        writable_roots: Vec<PathBuf>,
        shell: PathBuf,
        default_cwd: PathBuf,
        env_source: EnvSource,
        archive_root: impl Into<PathBuf>,
    ) -> Self {
        // Canonicalize what exists: a sandbox profile and a confined cwd
        // must speak the same physical paths, or `/var` versus
        // `/private/var` (macOS) denies writes the host meant to allow.
        let real = |path: PathBuf| std::fs::canonicalize(&path).unwrap_or(path);
        Self {
            policy,
            roots: roots.into_iter().map(real).collect(),
            writable_roots: writable_roots.into_iter().map(real).collect(),
            shell,
            default_cwd: real(default_cwd),
            env_allowlist: DEFAULT_ENV_ALLOWLIST
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            env_source,
            archive: OutputArchive::new(real(archive_root.into())),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Overrides the default timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Overrides the environment allowlist.
    #[must_use]
    pub fn with_env_allowlist(mut self, names: Vec<String>) -> Self {
        self.env_allowlist = names;
        self
    }

    /// The archive root, so a host can hand the model the expand tool.
    #[must_use]
    pub fn archive(&self) -> &OutputArchive {
        &self.archive
    }

    /// The argv a command runs as, after the sandbox wrapping. A host can
    /// log it; tests assert the wrapper is composed in.
    ///
    /// # Errors
    ///
    /// The sandbox could not wrap the command.
    pub fn command_argv(&self, command: &str) -> io::Result<Vec<String>> {
        sandbox::argv(
            &self.policy,
            &self.roots,
            &self.writable_roots,
            &[
                self.shell.to_string_lossy().into_owned(),
                "-c".to_string(),
                command.to_string(),
            ],
        )
    }

    /// Runs a command and collects the outcome.
    ///
    /// # Errors
    ///
    /// [`ShellError`]; a non-zero exit is a successful run, not an error.
    pub fn run(&self, request: &RunRequest<'_>) -> Result<RunOutcome, ShellError> {
        let (tx, rx) = mpsc::channel();
        let outcome = self.run_streaming(request, tx)?;
        for event in rx {
            if let OutputEvent::Exit { code, .. } = event {
                debug_assert_eq!(code, outcome.code);
            }
        }
        Ok(outcome)
    }

    /// Runs a command, streaming [`OutputEvent`]s to `tx` as they arrive.
    /// The `Exit` event is the last one.
    ///
    /// # Errors
    ///
    /// [`ShellError`]; a non-zero exit is a successful run, not an error.
    pub fn run_streaming(
        &self,
        request: &RunRequest<'_>,
        tx: Sender<OutputEvent>,
    ) -> Result<RunOutcome, ShellError> {
        let command = request.command;
        if command.is_empty() || command.len() > aulo_policy::SHELL_COMMAND_MAX_BYTES {
            return Err(ShellError::Command {
                max: aulo_policy::SHELL_COMMAND_MAX_BYTES,
            });
        }
        let cwd = match request.cwd {
            Some(dir) => {
                // Canonicalize first: a symlinked root (`/var` versus
                // `/private/var`) must not look like an escape. A path that
                // does not exist stays as given; the spawn then fails with
                // a clear error.
                let real = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
                proc_sandbox::path::confine(
                    &self.writable_roots,
                    &self.default_cwd,
                    &real.to_string_lossy(),
                )?
            }
            None => self.default_cwd.clone(),
        };
        let started = Instant::now();
        let mut child = self.spawn(command, &cwd, request)?;
        let stdout = child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>);
        let stderr = child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>);
        let outcome = self.watch(stdout, stderr, &mut child, request, started, &tx)?;
        Ok(outcome)
    }

    /// Spawns the sandboxed command with piped output, its own process
    /// group on Unix, the confined cwd and the allowlisted environment.
    fn spawn(
        &self,
        command: &str,
        cwd: &Path,
        request: &RunRequest<'_>,
    ) -> Result<Child, ShellError> {
        let mut cmd = sandbox::command(
            &self.policy,
            &self.roots,
            &self.writable_roots,
            &self.shell,
            command,
        )
        .map_err(ShellError::Sandbox)?;
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(cwd)
            .env_clear();
        for name in &self.env_allowlist {
            if let Some(value) = (self.env_source)(name) {
                cmd.env(name, value);
            }
        }
        for (name, value) in request.env {
            cmd.env(name, value);
        }
        // A process-group leader, so the timeout kill reaches the whole
        // tree and never the daemon.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        cmd.spawn().map_err(ShellError::Spawn)
    }

    /// Reads both pipes on reader threads, enforces the timeout (killing
    /// the tree), and assembles the outcome.
    fn watch(
        &self,
        stdout: Option<Box<dyn Read + Send + 'static>>,
        stderr: Option<Box<dyn Read + Send + 'static>>,
        child: &mut Child,
        request: &RunRequest<'_>,
        started: Instant,
        tx: &Sender<OutputEvent>,
    ) -> Result<RunOutcome, ShellError> {
        let id = new_archive_id();
        let (done_tx, done_rx) = mpsc::channel::<Stream>();
        let mut handles = Vec::new();
        for (stream, pipe) in [(Stream::Stdout, stdout), (Stream::Stderr, stderr)] {
            let Some(mut pipe) = pipe else { continue };
            let archive_path = self
                .archive
                .create(&id, stream)
                .map_err(ShellError::Archive)?;
            let tx = tx.clone();
            let done_tx = done_tx.clone();
            handles.push(std::thread::spawn(move || {
                let mut file = match std::fs::OpenOptions::new().append(true).open(&archive_path) {
                    Ok(file) => file,
                    Err(_) => {
                        let _ = done_tx.send(stream);
                        return None;
                    }
                };
                let mut total: u64 = 0;
                let mut view: Vec<u8> = Vec::new();
                let mut archive_truncated = false;
                let mut chunk = [0u8; READ_CHUNK];
                loop {
                    match pipe.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            let bytes = &chunk[..n];
                            total += n as u64;
                            if view.len() < OUTPUT_VIEW_MAX_BYTES {
                                let room = OUTPUT_VIEW_MAX_BYTES - view.len();
                                view.extend_from_slice(&bytes[..n.min(room)]);
                            }
                            if total <= ARCHIVE_MAX_BYTES {
                                if file.write_all(bytes).is_err() {
                                    archive_truncated = true;
                                }
                            } else {
                                archive_truncated = true;
                            }
                            let _ = tx.send(OutputEvent::Chunk {
                                stream,
                                bytes: bytes.to_vec(),
                            });
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
                let _ = done_tx.send(stream);
                Some(StreamStats {
                    total_bytes: total,
                    view,
                    archive_truncated,
                })
                .map(|stats| (stream, stats))
            }));
        }
        drop(done_tx);

        let timeout = request.timeout.unwrap_or(self.timeout);
        let deadline = started + timeout;
        let mut finished = 0usize;
        let mut timed_out = false;
        while finished < handles.len() {
            let left = deadline.saturating_duration_since(Instant::now());
            match done_rx.recv_timeout(left) {
                Ok(_) => finished += 1,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    timed_out = true;
                    kill_tree(child);
                    // The killed tree closes its pipes; the grace is only
                    // a backstop for a grandchild that ignored the signal.
                    let grace = Instant::now() + KILL_GRACE;
                    while finished < handles.len() {
                        match done_rx.recv_timeout(grace.saturating_duration_since(Instant::now()))
                        {
                            Ok(_) => finished += 1,
                            Err(_) => break,
                        }
                    }
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        let status = child.wait().ok();
        let mut stats: HashMap<Stream, StreamStats> = HashMap::new();
        for handle in handles {
            if let Ok(Some((stream, stat))) = handle.join() {
                stats.insert(stream, stat);
            }
        }
        let _ = tx.send(OutputEvent::Exit {
            code: status.and_then(|s| s.code()),
            timed_out,
        });
        Ok(RunOutcome {
            code: status.and_then(|s| s.code()),
            timed_out,
            duration: started.elapsed(),
            backend: sandbox::backend(self.policy.linux_backend).map(|b| b.name()),
            stdout: view_of(stats.remove(&Stream::Stdout), &id),
            stderr: view_of(stats.remove(&Stream::Stderr), &id),
        })
    }
}

/// The model-facing view of one stream.
fn view_of(stats: Option<StreamStats>, id: &ArchiveId) -> StreamView {
    match stats {
        None => StreamView {
            text: String::new(),
            truncated: false,
            archive: None,
            total_bytes: 0,
        },
        Some(stats) => StreamView {
            text: String::from_utf8_lossy(&stats.view).into_owned(),
            truncated: stats.total_bytes > stats.view.len() as u64 || stats.archive_truncated,
            archive: (stats.total_bytes > 0).then(|| id.clone()),
            total_bytes: stats.total_bytes,
        },
    }
}

/// Kills the child and everything it spawned.
fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        use rustix::process::{Pid, Signal, kill_process_group};
        // The child is its own group leader (process_group(0) at spawn).
        if let Some(pid) = Pid::from_raw(child.id() as i32) {
            let _ = kill_process_group(pid, Signal::KILL);
        }
    }
    #[cfg(not(unix))]
    {
        // No process groups on Windows: taskkill walks the tree instead.
        let _ = std::process::Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

/// A filesystem-safe archive id: milliseconds since the epoch, the process
/// id and a counter, so concurrent runs never collide.
fn new_archive_id() -> ArchiveId {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ArchiveId(format!("{nanos:x}-{:x}-{n:x}", std::process::id()))
}
