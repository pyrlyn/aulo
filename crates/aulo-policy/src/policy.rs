//! The compiled rule set: one engine, one settings triple, decisions as a
//! pure function of the call and the active grants.
//!
//! A [`Policy`] is deterministic code outside the model. It decides
//! nothing about the conversation, opens no socket and reads no key: the
//! daemon builds one per daemon from config and installs it as the
//! session's approval gate ([`crate::PolicyGate`]).

use std::path::Path;

use perm_rules::{
    ApprovalPolicy, Call, Engine, Outcome, PermissionMode, RuleError, RuleSet, SandboxMode, Why,
};

use crate::subjects::grammar;

/// The session-wide settings a decision also depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// The permission mode.
    pub mode: PermissionMode,
    /// When the user is asked.
    pub approval: ApprovalPolicy,
    /// How much the host confines a process.
    pub sandbox: SandboxMode,
}

impl Default for Settings {
    /// aulo's defaults (spec §6.3): the user's desktop, commands confined
    /// to the workspace, risk-based asking.
    fn default() -> Self {
        Self {
            mode: PermissionMode::Default,
            approval: ApprovalPolicy::OnRequest,
            sandbox: SandboxMode::WorkspaceWrite,
        }
    }
}

/// One compiled rule set with its settings.
#[derive(Debug, Clone)]
pub struct Policy {
    engine: Engine,
    settings: Settings,
}

impl Policy {
    /// Compiles `rules` under aulo's grammar. `home` expands a leading
    /// `~/` in a path rule; `cwd` anchors relative path globs.
    ///
    /// # Errors
    ///
    /// [`RuleError`] when a rule is not valid grammar: a config error, so
    /// the caller refuses to serve rather than running without a guard.
    pub fn new(
        rules: &RuleSet,
        settings: Settings,
        home: Option<&Path>,
        cwd: &Path,
    ) -> Result<Self, RuleError> {
        Ok(Self {
            engine: Engine::compile(rules, grammar(), home, cwd)?,
            settings,
        })
    }

    /// The settings in effect.
    #[must_use]
    pub fn settings(&self) -> Settings {
        self.settings
    }

    /// Replaces the settings (a mode switch, an unattended run).
    #[must_use]
    pub fn with_settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// The decision for `call` under `grants`, the active
    /// `(tool, subject)` grants. Order: deny rule, mode, allow rule, ask
    /// rule, grant, approval policy, risk fallback.
    #[must_use]
    pub fn decide(&self, call: &Call, grants: &[(String, String)]) -> Outcome {
        self.engine.decide(
            call,
            self.settings.mode,
            self.settings.approval,
            self.settings.sandbox,
            grants,
        )
    }

    /// The decision for a shell line: `shell-classify`'s risk is the
    /// fallback when no rule matches (T5.2).
    #[must_use]
    pub fn decide_shell(&self, command: &str, grants: &[(String, String)]) -> Outcome {
        self.decide(&crate::shell_call(command), grants)
    }
}

/// The `(tool, subject)` grants an allow-for-session answer to `call`
/// records. Re-exported so a surface does not reach into `perm-rules`.
#[must_use]
pub fn grants_for(call: &Call) -> Vec<(String, String)> {
    perm_rules::grants_for(call)
}

/// One line for a deny reason or a notice.
#[must_use]
pub fn why_text(why: &Why) -> String {
    perm_rules::why_text(why)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subjects::{app_call, file_call, input_call, mcp_call, site_call};
    use perm_rules::DecidedBy;
    use std::path::PathBuf;

    fn policy(rules: RuleSet) -> Policy {
        Policy::new(&rules, Settings::default(), None, &PathBuf::from(".")).expect("compiles")
    }

    #[test]
    fn a_malformed_rule_fails_the_whole_set_with_its_list() {
        let rules = RuleSet {
            deny: vec!["Edit(src/**".into()],
            ..RuleSet::default()
        };
        let error = Policy::new(&rules, Settings::default(), None, &PathBuf::from("."))
            .expect_err("a config error");
        assert_eq!(error.list, perm_rules::RuleList::Deny);
        assert_eq!(error.rule, "Edit(src/**");
    }

    #[test]
    fn every_subject_falls_back_to_its_risk() {
        let policy = policy(RuleSet::default());
        assert!(matches!(
            policy.decide(&app_call("com.apple.Safari"), &[]),
            Outcome::Ask(_)
        ));
        assert!(matches!(
            policy.decide(&site_call("example.com"), &[]),
            Outcome::Allow { .. }
        ));
        assert!(matches!(
            policy.decide(&file_call("read", "src/main.rs"), &[]),
            Outcome::Allow { .. }
        ));
        assert!(matches!(
            policy.decide(&file_call("edit", "src/main.rs"), &[]),
            Outcome::Ask(_)
        ));
        assert!(matches!(
            policy.decide(&mcp_call("github", "create_issue"), &[]),
            Outcome::Ask(_)
        ));
        assert!(matches!(
            policy.decide(&input_call("click"), &[]),
            Outcome::Ask(_)
        ));
    }

    #[test]
    fn a_grant_covers_the_call_it_was_given_for() {
        let policy = policy(RuleSet::default());
        let call = app_call("com.apple.Safari");
        let grants = grants_for(&call);
        assert!(matches!(
            policy.decide(&call, &grants),
            Outcome::Allow {
                by: DecidedBy::Session
            }
        ));
        // A grant for another app covers nothing.
        assert!(matches!(
            policy.decide(&app_call("com.apple.Terminal"), &grants),
            Outcome::Ask(_)
        ));
    }

    #[test]
    fn plan_mode_denies_everything_but_reads() {
        let policy = policy(RuleSet::default()).with_settings(Settings {
            mode: PermissionMode::Plan,
            ..Settings::default()
        });
        assert!(matches!(
            policy.decide(&file_call("read", "src/main.rs"), &[]),
            Outcome::Allow { .. }
        ));
        assert!(matches!(
            policy.decide(&file_call("edit", "src/main.rs"), &[]),
            Outcome::Deny { .. }
        ));
    }
}
