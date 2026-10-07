//! Windows: a Task Scheduler task that runs at the user's logon. A task cannot
//! redirect output without a shell, so only the daemon's own log files under the
//! aulo home exist; that is where its logs already go.

use anyhow::Result;

use super::{Backend, Installed, ServiceManager, Spec, Status, must, query};

const TASK: &str = "aulod";

#[derive(Debug)]
pub struct Schtasks;

/// One argument as the C runtime's command-line parser reads it back:
/// backslashes only matter in front of a quote, and a trailing run of them must
/// be doubled before the closing quote.
pub fn quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_owned();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            c => {
                out.push_str(&"\\".repeat(backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

/// A task has no environment of its own, so a non-default home travels as the
/// `--home` flag instead of `AULO_HOME`.
pub fn command_line(spec: &Spec) -> String {
    let home = if spec.home_pinned {
        format!(" --home {}", quote(&spec.home))
    } else {
        String::new()
    };
    format!("{}{home} run", quote(&spec.exe))
}

fn exists(mgr: &mut dyn ServiceManager) -> Result<bool> {
    Ok(query(mgr, "schtasks", &["/Query", "/TN", TASK])?.success)
}

impl Backend for Schtasks {
    fn install(&self, mgr: &mut dyn ServiceManager, spec: &Spec) -> Result<Installed> {
        let existed = exists(mgr)?;
        // Task Scheduler cannot tell whether a definition changed, and `/F`
        // replaces it in place, so a repeat install is a harmless rewrite.
        must(
            mgr,
            "schtasks",
            &[
                "/Create",
                "/SC",
                "ONLOGON",
                "/TN",
                TASK,
                "/TR",
                &command_line(spec),
                "/F",
            ],
        )?;
        if existed {
            return Ok(Installed::Updated);
        }
        // A logon trigger only fires at the next logon; start it now like the
        // other platforms do.
        must(mgr, "schtasks", &["/Run", "/TN", TASK])?;
        Ok(Installed::Fresh)
    }

    fn uninstall(&self, mgr: &mut dyn ServiceManager, _spec: &Spec) -> Result<bool> {
        if !exists(mgr)? {
            return Ok(false);
        }
        // Fails when the task is not running, which is fine.
        query(mgr, "schtasks", &["/End", "/TN", TASK])?;
        must(mgr, "schtasks", &["/Delete", "/TN", TASK, "/F"])?;
        Ok(true)
    }

    fn status(&self, mgr: &mut dyn ServiceManager, _spec: &Spec) -> Result<Status> {
        let out = query(mgr, "schtasks", &["/Query", "/TN", TASK, "/FO", "LIST"])?;
        if !out.success {
            return Ok(Status::NotInstalled);
        }
        // The state label is localised, but the value `Running` is the English
        // name only; other locales report "not running", which errs safe.
        let running = out
            .stdout
            .lines()
            .any(|l| l.trim_end().ends_with("Running"));
        Ok(Status::Installed { running })
    }
}
