//! Linux: a systemd user unit, enabled for the user's default target.

use std::path::PathBuf;

use anyhow::{Context, Result};

use super::{Backend, Installed, ServiceManager, Spec, Status, must, query, sync_file};

const UNIT: &str = "aulod.service";

#[derive(Debug)]
pub struct Systemd;

fn unit_path(spec: &Spec) -> PathBuf {
    spec.user_home.join(".config/systemd/user").join(UNIT)
}

/// A systemd word in double quotes. `%` starts a specifier everywhere in a
/// unit, and `$` starts an environment substitution in `ExecStart=`; both are
/// doubled so a path is never reinterpreted.
pub fn quote(s: &str, expand_dollar: bool) -> String {
    // The backslash goes first so the escapes added below are not doubled.
    let mut s = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    if expand_dollar {
        s = s.replace('$', "$$");
    }
    format!("\"{s}\"")
}

pub fn render(spec: &Spec) -> String {
    let exec = quote(&spec.exe, true);
    let env = spec.home_pinned.then(|| {
        let home = quote(&format!("AULO_HOME={}", spec.home), false);
        format!("Environment={home}\n")
    });
    let env = env.unwrap_or_default();
    // `append:` takes the rest of the line verbatim, so only `%` needs care.
    let log = spec.log_file().to_string_lossy().replace('%', "%%");
    format!(
        "[Unit]\n\
         Description=aulo daemon\n\
         \n\
         [Service]\n\
         ExecStart={exec} run\n\
         {env}\
         Restart=on-failure\n\
         RestartSec=5\n\
         StandardOutput=append:{log}\n\
         StandardError=append:{log}\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    )
}

fn ctl(mgr: &mut dyn ServiceManager, args: &[&str]) -> Result<bool> {
    Ok(query(mgr, "systemctl", &[&["--user"], args].concat())?.success)
}

fn ctl_must(mgr: &mut dyn ServiceManager, args: &[&str]) -> Result<()> {
    must(mgr, "systemctl", &[&["--user"], args].concat()).map(drop)
}

impl Backend for Systemd {
    fn install(&self, mgr: &mut dyn ServiceManager, spec: &Spec) -> Result<Installed> {
        let path = unit_path(spec);
        let existed = mgr.read(&path)?.is_some();
        mgr.create_dir_all(&spec.log_dir())
            .context("cannot create the log directory")?;
        let changed = sync_file(mgr, &path, &render(spec))?;
        let enabled = ctl(mgr, &["is-enabled", UNIT])?;
        let active = ctl(mgr, &["is-active", UNIT])?;
        if !changed && enabled && active {
            return Ok(Installed::Unchanged);
        }
        if changed {
            ctl_must(mgr, &["daemon-reload"])?;
        }
        ctl_must(mgr, &["enable", "--now", UNIT])?;
        // `enable --now` leaves a running service on its old definition.
        if changed && active {
            ctl_must(mgr, &["restart", UNIT])?;
        }
        Ok(if existed {
            Installed::Updated
        } else {
            Installed::Fresh
        })
    }

    fn uninstall(&self, mgr: &mut dyn ServiceManager, spec: &Spec) -> Result<bool> {
        let path = unit_path(spec);
        // Without the unit there is nothing to ask systemctl about, which also
        // keeps uninstall working on a host with no systemd.
        if mgr.read(&path)?.is_none() {
            return Ok(false);
        }
        ctl_must(mgr, &["disable", "--now", UNIT])?;
        mgr.remove(&path)?;
        ctl_must(mgr, &["daemon-reload"])?;
        Ok(true)
    }

    fn status(&self, mgr: &mut dyn ServiceManager, spec: &Spec) -> Result<Status> {
        if mgr.read(&unit_path(spec))?.is_none() {
            return Ok(Status::NotInstalled);
        }
        let running = ctl(mgr, &["is-active", UNIT])?;
        Ok(Status::Installed { running })
    }
}
