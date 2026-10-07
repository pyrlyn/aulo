//! The backends against a fake that keeps files and a model of what the OS
//! tool has registered, so a wrong call order or a non-idempotent step fails
//! the way the real tool would.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use super::*;

#[derive(Debug, Default)]
struct Fake {
    files: BTreeMap<PathBuf, String>,
    dirs: BTreeSet<PathBuf>,
    /// Every command run, as one line.
    log: Vec<String>,
    loaded: bool,
    running: bool,
    enabled: bool,
    task: Option<String>,
}

fn fail(stderr: &str) -> io::Result<Output> {
    Ok(Output {
        success: false,
        stdout: String::new(),
        stderr: stderr.to_owned(),
    })
}

fn ok(stdout: &str) -> io::Result<Output> {
    Ok(Output {
        success: true,
        stdout: stdout.to_owned(),
        stderr: String::new(),
    })
}

impl Fake {
    fn launchctl(&mut self, args: &[String]) -> io::Result<Output> {
        match (args[0].as_str(), self.loaded) {
            ("print", true) => ok(if self.running {
                "state = running\n"
            } else {
                "state = waiting\n"
            }),
            ("print", false) => fail("not found"),
            ("bootstrap", false) => {
                (self.loaded, self.running) = (true, true);
                ok("")
            }
            ("bootout", true) => {
                (self.loaded, self.running) = (false, false);
                ok("")
            }
            // The real tool answers "Input/output error" for both.
            ("bootstrap" | "bootout", _) => fail("Input/output error"),
            _ => fail("unknown subcommand"),
        }
    }

    fn systemctl(&mut self, args: &[String]) -> io::Result<Output> {
        assert_eq!(args[0], "--user");
        let rest: Vec<&str> = args[1..].iter().map(String::as_str).collect();
        match rest.as_slice() {
            ["is-enabled", _] => Ok(Output {
                success: self.enabled,
                ..Output::default()
            }),
            ["is-active", _] => Ok(Output {
                success: self.running,
                ..Output::default()
            }),
            ["enable", "--now", _] => {
                (self.enabled, self.running) = (true, true);
                ok("")
            }
            ["disable", "--now", _] => {
                (self.enabled, self.running) = (false, false);
                ok("")
            }
            ["daemon-reload"] | ["restart", _] => ok(""),
            other => fail(&format!("unexpected {other:?}")),
        }
    }

    fn schtasks(&mut self, args: &[String]) -> io::Result<Output> {
        let tr = args
            .iter()
            .position(|a| a == "/TR")
            .map(|i| args[i + 1].clone());
        match (args[0].as_str(), self.task.is_some()) {
            ("/Query", true) => ok("Status:                               Running\n"),
            ("/Query" | "/Run" | "/End" | "/Delete", false) => {
                fail("ERROR: The system cannot find the file specified.")
            }
            ("/Create", _) => {
                self.task = tr;
                ok("")
            }
            ("/Delete", true) => {
                self.task = None;
                ok("")
            }
            ("/Run" | "/End", true) => ok(""),
            _ => fail("unknown"),
        }
    }
}

impl ServiceManager for Fake {
    fn read(&self, path: &Path) -> io::Result<Option<String>> {
        Ok(self.files.get(path).cloned())
    }

    fn write(&mut self, path: &Path, contents: &str) -> io::Result<()> {
        self.files.insert(path.to_owned(), contents.to_owned());
        Ok(())
    }

    fn remove(&mut self, path: &Path) -> io::Result<()> {
        self.files.remove(path);
        Ok(())
    }

    fn create_dir_all(&mut self, path: &Path) -> io::Result<()> {
        self.dirs.insert(path.to_owned());
        Ok(())
    }

    fn run(&mut self, program: &str, args: &[String]) -> io::Result<Output> {
        self.log.push(format!("{program} {}", args.join(" ")));
        match program {
            "id" => ok("501\n"),
            "launchctl" => self.launchctl(args),
            "systemctl" => self.systemctl(args),
            "schtasks" => self.schtasks(args),
            other => Err(io::Error::new(io::ErrorKind::NotFound, other.to_owned())),
        }
    }
}

fn spec_at(exe: &str, home_pinned: bool) -> Spec {
    Spec::new(
        Path::new(exe),
        Path::new("/Users/me/aulo home"),
        home_pinned,
        PathBuf::from("/Users/me"),
    )
    .unwrap()
}

fn spec() -> Spec {
    spec_at("/opt/aulo/aulod", false)
}

/// Commands that change state, without the read-only queries.
fn changes(fake: &Fake) -> Vec<&str> {
    fake.log
        .iter()
        .map(String::as_str)
        .filter(|c| {
            !(c.starts_with("id ")
                || c.contains(" print ")
                || c.contains("is-enabled")
                || c.contains("is-active")
                || c.contains("/Query"))
        })
        .collect()
}

// launchd and systemd only run on Unix; on Windows `Path::join` writes `\`.
#[cfg(unix)]
#[test]
fn launchd_install_and_uninstall_are_idempotent() {
    let (backend, spec, mut fake) = (launchd::Launchd, spec(), Fake::default());
    let plist = PathBuf::from("/Users/me/Library/LaunchAgents/dev.aulo.aulod.plist");

    assert_eq!(backend.install(&mut fake, &spec).unwrap(), Installed::Fresh);
    assert!(fake.files.contains_key(&plist));
    assert!(fake.dirs.contains(Path::new("/Users/me/aulo home/logs")));
    assert_eq!(
        changes(&fake),
        ["launchctl bootstrap gui/501 /Users/me/Library/LaunchAgents/dev.aulo.aulod.plist"]
    );

    fake.log.clear();
    assert_eq!(
        backend.install(&mut fake, &spec).unwrap(),
        Installed::Unchanged
    );
    assert!(changes(&fake).is_empty());

    let moved = spec_at("/opt/other/aulod", false);
    assert_eq!(
        backend.install(&mut fake, &moved).unwrap(),
        Installed::Updated
    );
    assert!(fake.files[&plist].contains("/opt/other/aulod"));
    assert_eq!(
        changes(&fake),
        [
            "launchctl bootout gui/501/dev.aulo.aulod",
            "launchctl bootstrap gui/501 /Users/me/Library/LaunchAgents/dev.aulo.aulod.plist"
        ]
    );
    assert_eq!(
        backend.status(&mut fake, &spec).unwrap(),
        Status::Installed { running: true }
    );

    fake.log.clear();
    assert!(backend.uninstall(&mut fake, &spec).unwrap());
    assert!(fake.files.is_empty() && !fake.loaded);
    fake.log.clear();
    assert!(!backend.uninstall(&mut fake, &spec).unwrap());
    assert!(fake.log.is_empty());
    assert_eq!(
        backend.status(&mut fake, &spec).unwrap(),
        Status::NotInstalled
    );
}

#[test]
fn launchd_reloads_a_file_that_is_present_but_not_loaded() {
    let (backend, spec, mut fake) = (launchd::Launchd, spec(), Fake::default());
    backend.install(&mut fake, &spec).unwrap();
    (fake.loaded, fake.running) = (false, false);
    fake.log.clear();
    assert_eq!(
        backend.install(&mut fake, &spec).unwrap(),
        Installed::Updated
    );
    assert_eq!(changes(&fake).len(), 1);
    assert!(fake.loaded);
}

#[test]
fn systemd_install_and_uninstall_are_idempotent() {
    let (backend, spec, mut fake) = (systemd::Systemd, spec(), Fake::default());
    let unit = PathBuf::from("/Users/me/.config/systemd/user/aulod.service");

    assert_eq!(backend.install(&mut fake, &spec).unwrap(), Installed::Fresh);
    assert!(fake.files.contains_key(&unit));
    assert_eq!(
        changes(&fake),
        [
            "systemctl --user daemon-reload",
            "systemctl --user enable --now aulod.service"
        ]
    );

    fake.log.clear();
    assert_eq!(
        backend.install(&mut fake, &spec).unwrap(),
        Installed::Unchanged
    );
    assert!(changes(&fake).is_empty());

    let moved = spec_at("/opt/other/aulod", false);
    fake.log.clear();
    assert_eq!(
        backend.install(&mut fake, &moved).unwrap(),
        Installed::Updated
    );
    assert_eq!(
        changes(&fake),
        [
            "systemctl --user daemon-reload",
            "systemctl --user enable --now aulod.service",
            "systemctl --user restart aulod.service"
        ]
    );
    assert_eq!(
        backend.status(&mut fake, &spec).unwrap(),
        Status::Installed { running: true }
    );

    fake.log.clear();
    assert!(backend.uninstall(&mut fake, &spec).unwrap());
    assert_eq!(
        changes(&fake),
        [
            "systemctl --user disable --now aulod.service",
            "systemctl --user daemon-reload"
        ]
    );
    assert!(fake.files.is_empty());
    fake.log.clear();
    assert!(!backend.uninstall(&mut fake, &spec).unwrap());
    assert!(fake.log.is_empty());
}

#[test]
fn schtasks_install_and_uninstall_are_idempotent() {
    let (backend, spec, mut fake) = (schtasks::Schtasks, spec(), Fake::default());

    assert_eq!(backend.install(&mut fake, &spec).unwrap(), Installed::Fresh);
    assert_eq!(fake.task.as_deref(), Some("/opt/aulo/aulod run"));
    assert_eq!(
        changes(&fake),
        [
            "schtasks /Create /SC ONLOGON /TN aulod /TR /opt/aulo/aulod run /F",
            "schtasks /Run /TN aulod"
        ]
    );

    fake.log.clear();
    assert_eq!(
        backend.install(&mut fake, &spec).unwrap(),
        Installed::Updated
    );
    assert_eq!(
        changes(&fake).len(),
        1,
        "a repeat install rewrites in place and starts nothing"
    );
    assert_eq!(
        backend.status(&mut fake, &spec).unwrap(),
        Status::Installed { running: true }
    );

    assert!(backend.uninstall(&mut fake, &spec).unwrap());
    assert!(fake.task.is_none());
    fake.log.clear();
    assert!(!backend.uninstall(&mut fake, &spec).unwrap());
    assert_eq!(fake.log, ["schtasks /Query /TN aulod"]);
    assert_eq!(
        backend.status(&mut fake, &spec).unwrap(),
        Status::NotInstalled
    );
}

#[test]
fn a_missing_tool_is_an_error_not_a_panic() {
    let err = systemd::Systemd.install(&mut Missing, &spec()).unwrap_err();
    assert!(
        format!("{err:#}").contains("cannot run systemctl"),
        "{err:#}"
    );
}

#[test]
fn a_tool_that_fails_stops_install_with_its_message() {
    let mut fake = Fake::default();
    // A loaded job whose plist changed is booted out and bootstrapped again.
    let spec = spec();
    launchd::Launchd.install(&mut fake, &spec).unwrap();
    fake.files.clear();
    let err = launchd::Launchd
        .install(&mut FailingBootstrap(fake), &spec)
        .unwrap_err();
    assert!(err.to_string().contains("launchctl bootstrap"), "{err:#}");
}

/// Delegates to the fake but refuses every bootstrap.
struct FailingBootstrap(Fake);

impl ServiceManager for FailingBootstrap {
    fn read(&self, path: &Path) -> io::Result<Option<String>> {
        self.0.read(path)
    }
    fn write(&mut self, path: &Path, contents: &str) -> io::Result<()> {
        self.0.write(path, contents)
    }
    fn remove(&mut self, path: &Path) -> io::Result<()> {
        self.0.remove(path)
    }
    fn create_dir_all(&mut self, path: &Path) -> io::Result<()> {
        self.0.create_dir_all(path)
    }
    fn run(&mut self, program: &str, args: &[String]) -> io::Result<Output> {
        if args.first().is_some_and(|a| a == "bootstrap") {
            return fail("Bootstrap failed: 5: Input/output error");
        }
        self.0.run(program, args)
    }
}

/// A host where no tool exists.
struct Missing;

impl ServiceManager for Missing {
    fn read(&self, _: &Path) -> io::Result<Option<String>> {
        Ok(None)
    }
    fn write(&mut self, _: &Path, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn remove(&mut self, _: &Path) -> io::Result<()> {
        Ok(())
    }
    fn create_dir_all(&mut self, _: &Path) -> io::Result<()> {
        Ok(())
    }
    fn run(&mut self, program: &str, _: &[String]) -> io::Result<Output> {
        Err(io::Error::new(io::ErrorKind::NotFound, program.to_owned()))
    }
}

// Only the Unix-only plist and unit tests use it.
#[cfg(unix)]
const AWKWARD: &str = r#"/opt/my apps/"q" & <x> 'y' 50% $HOME\bin/aulod"#;

// launchd and systemd only run on Unix; on Windows `Path::join` writes `\`.
#[cfg(unix)]
#[test]
fn plist_escapes_xml_and_keeps_the_path_as_one_argument() {
    let plist = launchd::render(&spec_at(AWKWARD, true));
    assert!(plist.contains(
        "<string>/opt/my apps/&quot;q&quot; &amp; &lt;x&gt; &apos;y&apos; 50% $HOME\\bin/aulod</string>\n    <string>run</string>"
    ));
    assert!(plist.contains("<key>AULO_HOME</key>\n    <string>/Users/me/aulo home</string>"));
    assert!(plist.contains(
        "<key>StandardOutPath</key>\n  <string>/Users/me/aulo home/logs/aulod-service.log</string>"
    ));
    assert!(plist.contains("<key>StandardErrorPath</key>"));
}

#[test]
fn plist_without_a_pinned_home_has_no_environment() {
    let plist = launchd::render(&spec());
    assert!(!plist.contains("AULO_HOME"));
    assert!(plist.starts_with("<?xml"));
}

// launchd and systemd only run on Unix; on Windows `Path::join` writes `\`.
#[cfg(unix)]
#[test]
fn unit_quotes_the_command_and_doubles_specifiers() {
    let unit = systemd::render(&spec_at(AWKWARD, true));
    assert!(
        unit.contains(r#"ExecStart="/opt/my apps/\"q\" & <x> 'y' 50%% $$HOME\\bin/aulod" run"#)
    );
    assert!(unit.contains("Environment=\"AULO_HOME=/Users/me/aulo home\"\n"));
    assert!(unit.contains("StandardOutput=append:/Users/me/aulo home/logs/aulod-service.log\n"));
    assert!(unit.contains("StandardError=append:/Users/me/aulo home/logs/aulod-service.log\n"));
    assert!(unit.contains("WantedBy=default.target"));
}

#[test]
fn unit_environment_keeps_dollars_but_doubles_percent() {
    assert_eq!(systemd::quote("A=$x%y", false), r#""A=$x%%y""#);
    assert!(!systemd::render(&spec()).contains("Environment="));
}

#[test]
fn windows_quoting_follows_the_c_runtime_rules() {
    let q = schtasks::quote;
    assert_eq!(q(r"C:\aulod.exe"), r"C:\aulod.exe");
    assert_eq!(q(""), r#""""#);
    assert_eq!(
        q(r"C:\Program Files\aulo\aulod.exe"),
        r#""C:\Program Files\aulo\aulod.exe""#
    );
    assert_eq!(q(r#"a "b" & c"#), r#""a \"b\" & c""#);
    assert_eq!(q(r"C:\my dir\"), r#""C:\my dir\\""#);
    assert_eq!(q(r#"a\"b c"#), r#""a\\\"b c""#);
}

#[test]
fn task_command_line_carries_the_home_as_a_flag() {
    let line = schtasks::command_line(&spec_at(r"C:\Program Files\aulo\aulod.exe", true));
    assert_eq!(
        line,
        r#""C:\Program Files\aulo\aulod.exe" --home "/Users/me/aulo home" run"#
    );
    assert_eq!(
        schtasks::command_line(&spec_at(r"C:\aulod.exe", false)),
        r"C:\aulod.exe run"
    );
}

#[test]
fn paths_that_cannot_be_written_safely_are_refused() {
    let home = Path::new("/h");
    for bad in ["/a\nExecStop=/bin/evil", "/a\rb", "/a\0b", "/a\u{1b}b"] {
        let err = Spec::new(Path::new(bad), home, false, PathBuf::new()).unwrap_err();
        assert!(err.to_string().contains("control character"), "{bad:?}");
    }
    assert!(Spec::new(Path::new("/a"), Path::new("/h\n"), true, PathBuf::new()).is_err());
}

#[cfg(unix)]
#[test]
fn non_utf8_paths_are_refused() {
    use std::os::unix::ffi::OsStrExt;
    let bad = Path::new(std::ffi::OsStr::from_bytes(b"/a\xffb"));
    assert!(Spec::new(bad, Path::new("/h"), false, PathBuf::new()).is_err());
}

#[test]
fn only_the_three_known_os_have_a_backend() {
    for os in ["macos", "linux", "windows"] {
        assert!(backend_for(os).is_ok());
    }
    assert!(backend_for("freebsd").is_err());
}

#[test]
fn the_real_manager_runs_programs_without_a_shell() {
    // A shell would expand `$HOME` or split on `;`; echo must see them as is.
    #[cfg(unix)]
    {
        let out = System.run("echo", &["$HOME; touch x".to_owned()]).unwrap();
        assert!(out.success);
        assert_eq!(out.stdout, "$HOME; touch x\n");
    }
    assert!(System.run("aulo-no-such-program", &[]).is_err());
}

#[test]
fn the_real_manager_writes_creates_and_removes() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a/b/unit");
    let mut sys = System;
    assert_eq!(sys.read(&file).unwrap(), None);
    sys.write(&file, "x").unwrap();
    assert_eq!(sys.read(&file).unwrap().as_deref(), Some("x"));
    sys.remove(&file).unwrap();
    sys.remove(&file).unwrap();
    sys.create_dir_all(&dir.path().join("logs/deep")).unwrap();
    assert!(dir.path().join("logs/deep").is_dir());
}
