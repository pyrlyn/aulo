//! macOS: a LaunchAgent plist, loaded into the user's GUI domain.

use anyhow::{Context, Result};

use super::{Backend, Installed, ServiceManager, Spec, Status, must, query, sync_file};

pub const LABEL: &str = "dev.aulo.aulod";

#[derive(Debug)]
pub struct Launchd;

fn plist_path(spec: &Spec) -> std::path::PathBuf {
    spec.user_home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

pub fn escape_xml(s: &str) -> String {
    // `&` first, or the entities below would be escaped again.
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn render(spec: &Spec) -> String {
    let string = |s: &str| format!("<string>{}</string>", escape_xml(s));
    let log = string(&spec.log_file().to_string_lossy());
    // One `<string>` is one argv entry, so spaces in the path need no quoting.
    let args = format!("    {}\n    {}\n", string(&spec.exe), string("run"));
    let env = spec.home_pinned.then(|| {
        let home = string(&spec.home);
        format!("  <key>EnvironmentVariables</key>\n  <dict>\n    <key>AULO_HOME</key>\n    {home}\n  </dict>\n")
    });
    let env = env.unwrap_or_default();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
{args}  </array>
{env}  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>StandardOutPath</key>
  {log}
  <key>StandardErrorPath</key>
  {log}
</dict>
</plist>
"#
    )
}

/// The domain is `gui/<uid>`: a LaunchAgent belongs to the user's login
/// session, not to a system-wide domain.
fn target(mgr: &mut dyn ServiceManager) -> Result<(String, String)> {
    let uid = must(mgr, "id", &["-u"])?.stdout;
    let domain = format!("gui/{}", uid.trim());
    let target = format!("{domain}/{LABEL}");
    Ok((domain, target))
}

/// The `launchctl print` text when the job is loaded.
fn loaded(mgr: &mut dyn ServiceManager, target: &str) -> Result<Option<String>> {
    let out = query(mgr, "launchctl", &["print", target])?;
    Ok(out.success.then_some(out.stdout))
}

impl Backend for Launchd {
    fn install(&self, mgr: &mut dyn ServiceManager, spec: &Spec) -> Result<Installed> {
        let path = plist_path(spec);
        let existed = mgr.read(&path)?.is_some();
        let (domain, target) = target(mgr)?;
        let was_loaded = loaded(mgr, &target)?.is_some();

        // launchd will not start a job whose log directory is missing.
        mgr.create_dir_all(&spec.log_dir())
            .context("cannot create the log directory")?;
        let changed = sync_file(mgr, &path, &render(spec))?;
        if !changed && was_loaded {
            return Ok(Installed::Unchanged);
        }
        if was_loaded {
            // A loaded job keeps its old definition until it is booted out.
            must(mgr, "launchctl", &["bootout", &target])?;
        }
        must(
            mgr,
            "launchctl",
            &["bootstrap", &domain, &path.to_string_lossy()],
        )?;
        Ok(if existed || was_loaded {
            Installed::Updated
        } else {
            Installed::Fresh
        })
    }

    fn uninstall(&self, mgr: &mut dyn ServiceManager, spec: &Spec) -> Result<bool> {
        let path = plist_path(spec);
        // Without the file there is nothing to ask launchctl about.
        if mgr.read(&path)?.is_none() {
            return Ok(false);
        }
        let (_, target) = target(mgr)?;
        if loaded(mgr, &target)?.is_some() {
            must(mgr, "launchctl", &["bootout", &target])?;
        }
        mgr.remove(&path)?;
        Ok(true)
    }

    fn status(&self, mgr: &mut dyn ServiceManager, spec: &Spec) -> Result<Status> {
        if mgr.read(&plist_path(spec))?.is_none() {
            return Ok(Status::NotInstalled);
        }
        let (_, target) = target(mgr)?;
        let running = loaded(mgr, &target)?.is_some_and(|out| out.contains("state = running"));
        Ok(Status::Installed { running })
    }
}
