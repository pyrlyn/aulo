//! `aulod service`: run the daemon as a per-user service that starts at login.
//! Each OS backend only talks to a `ServiceManager`, so tests drive all three
//! against a fake on any host.

mod launchd;
mod schtasks;
mod systemd;
#[cfg(test)]
mod tests;

use std::fs::{self, DirBuilder};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result, bail};
use clap::Subcommand;

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum Action {
    /// Install and start aulod as a user service that starts at login
    Install,
    /// Stop and remove the user service; succeeds when none is installed
    Uninstall,
    /// Show whether the user service is installed and running
    Status,
}

/// What the real machine would be asked to do, and nothing else, so a fake can
/// stand in for the file system and the OS tools.
pub trait ServiceManager {
    /// `None` when the file does not exist.
    fn read(&self, path: &Path) -> io::Result<Option<String>>;
    /// Creates missing parent directories.
    fn write(&mut self, path: &Path, contents: &str) -> io::Result<()>;
    /// A file that is already gone is not an error.
    fn remove(&mut self, path: &Path) -> io::Result<()>;
    fn create_dir_all(&mut self, path: &Path) -> io::Result<()>;
    /// An `Err` means the program could not be started; a program that ran and
    /// failed is `Ok` with `success == false`.
    fn run(&mut self, program: &str, args: &[String]) -> io::Result<Output>;
}

#[derive(Debug, Clone, Default)]
pub struct Output {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `program`; a non-zero exit is an answer, a spawn failure is an error.
fn query(mgr: &mut dyn ServiceManager, program: &str, args: &[&str]) -> Result<Output> {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    mgr.run(program, &args)
        .with_context(|| format!("cannot run {program}"))
}

/// Like `query`, but a failure stops the operation with the tool's message.
fn must(mgr: &mut dyn ServiceManager, program: &str, args: &[&str]) -> Result<Output> {
    let out = query(mgr, program, args)?;
    if !out.success {
        bail!("{program} {} failed: {}", args.join(" "), out.stderr.trim());
    }
    Ok(out)
}

/// What the unit has to say about the daemon it starts.
#[derive(Debug, Clone)]
pub struct Spec {
    /// Absolute path of the aulod binary.
    pub exe: String,
    /// The resolved aulo home; the service log lives under it.
    pub home: String,
    /// Whether the home must be passed on, because the installer's own
    /// `AULO_HOME` is set and a service does not inherit the login shell.
    pub home_pinned: bool,
    pub user_home: PathBuf,
}

impl Spec {
    pub fn new(exe: &Path, home: &Path, home_pinned: bool, user_home: PathBuf) -> Result<Self> {
        Ok(Self {
            exe: text(exe)?,
            home: text(home)?,
            home_pinned,
            user_home,
        })
    }

    pub fn detect() -> Result<Self> {
        let exe = std::env::current_exe().context("cannot find the aulod binary")?;
        let user_home = std::env::home_dir().context("cannot find the user home directory")?;
        let pinned = std::env::var_os("AULO_HOME").is_some_and(|v| !v.is_empty());
        let home =
            aulo_config::aulo_home().context("cannot find the aulo home: set AULO_HOME or HOME")?;
        // A relative AULO_HOME would resolve against the service manager's
        // working directory, not the shell's.
        let home = std::path::absolute(home).context("cannot resolve the aulo home")?;
        Self::new(&exe, &home, pinned, user_home)
    }

    pub fn log_dir(&self) -> PathBuf {
        Path::new(&self.home).join("logs")
    }

    /// Where the service manager sends stdout and stderr: panics and startup
    /// errors that happen before the daemon's own file logging exists.
    pub fn log_file(&self) -> PathBuf {
        self.log_dir().join("aulod-service.log")
    }
}

/// Paths end up inside XML, unit files and command lines. A control character
/// (a newline above all) would start a new directive, so refuse it outright;
/// non-UTF-8 paths cannot be written into any of the three formats.
fn text(path: &Path) -> Result<String> {
    let s = path
        .to_str()
        .with_context(|| format!("{} is not valid UTF-8", path.display()))?;
    if s.chars().any(char::is_control) {
        bail!("{s:?} contains a control character");
    }
    Ok(s.to_owned())
}

#[derive(Debug, PartialEq, Eq)]
pub enum Installed {
    Fresh,
    Updated,
    Unchanged,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    NotInstalled,
    Installed { running: bool },
}

pub trait Backend {
    fn install(&self, mgr: &mut dyn ServiceManager, spec: &Spec) -> Result<Installed>;
    /// Whether anything was installed; removing nothing is still success.
    fn uninstall(&self, mgr: &mut dyn ServiceManager, spec: &Spec) -> Result<bool>;
    fn status(&self, mgr: &mut dyn ServiceManager, spec: &Spec) -> Result<Status>;
}

pub fn backend_for(os: &str) -> Result<Box<dyn Backend>> {
    match os {
        "macos" => Ok(Box::new(launchd::Launchd)),
        "linux" => Ok(Box::new(systemd::Systemd)),
        "windows" => Ok(Box::new(schtasks::Schtasks)),
        other => bail!("no user service backend for {other}"),
    }
}

/// Writes `contents` only when it differs; reports whether the file changed.
fn sync_file(mgr: &mut dyn ServiceManager, path: &Path, contents: &str) -> Result<bool> {
    let existing = mgr
        .read(path)
        .with_context(|| format!("cannot read {}", path.display()))?;
    if existing.as_deref() == Some(contents) {
        return Ok(false);
    }
    mgr.write(path, contents)
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(true)
}

pub fn execute(action: Action) -> Result<ExitCode> {
    let backend = backend_for(std::env::consts::OS)?;
    let (spec, mut mgr) = (Spec::detect()?, System);
    let (message, code) = match action {
        Action::Install => match backend.install(&mut mgr, &spec)? {
            Installed::Fresh => ("service installed and started", ExitCode::SUCCESS),
            Installed::Updated => ("service updated", ExitCode::SUCCESS),
            Installed::Unchanged => ("service already installed", ExitCode::SUCCESS),
        },
        Action::Uninstall => match backend.uninstall(&mut mgr, &spec)? {
            true => ("service stopped and removed", ExitCode::SUCCESS),
            false => ("service is not installed", ExitCode::SUCCESS),
        },
        Action::Status => match backend.status(&mut mgr, &spec)? {
            Status::NotInstalled => ("not installed", ExitCode::FAILURE),
            Status::Installed { running: true } => ("installed, running", ExitCode::SUCCESS),
            Status::Installed { running: false } => ("installed, not running", ExitCode::SUCCESS),
        },
    };
    println!("{message}");
    Ok(code)
}

/// The real file system and real OS tools.
#[derive(Debug)]
struct System;

impl ServiceManager for System {
    fn read(&self, path: &Path) -> io::Result<Option<String>> {
        match fs::read_to_string(path) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn write(&mut self, path: &Path, contents: &str) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, contents)
    }

    fn remove(&mut self, path: &Path) -> io::Result<()> {
        match fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    fn create_dir_all(&mut self, path: &Path) -> io::Result<()> {
        let mut dir = DirBuilder::new();
        dir.recursive(true);
        // The daemon's own directories are private to the user; match that.
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut dir, 0o700);
        dir.create(path)
    }

    fn run(&mut self, program: &str, args: &[String]) -> io::Result<Output> {
        // No shell: each argument reaches the program as one argv entry.
        let out = Command::new(program).args(args).output()?;
        Ok(Output {
            success: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}
