//! The rule engine every tool call passes before running (spec §7, T5.1).
//!
//! Policy is deterministic code outside the model: a [`Policy`] compiles
//! the configured rule lists once, and a decision is a pure function of
//! the call, the settings and the active grants. It fails closed — a
//! malformed rule set is a config error, and an undecidable call asks or
//! denies rather than running.
//!
//! - [`subjects`] — the tool name each subject kind answers to and the
//!   call a tool builds: `Shell(cmd)`, `App(bundle)`, `Site(host)`,
//!   `File(path)`, `Mcp(server, tool)`, `Input(kind)` (spec §6.1).
//! - [`Policy`] — the compiled rules plus the mode, approval policy and
//!   sandbox a decision also depends on.
//! - [`PolicyGate`] — the agent loop's approval seam over a [`Policy`],
//!   which the daemon installs on every session.
//!
//! Shell risk classification ([`shell_call`], T5.2) feeds the risk
//! fallback when no rule matches. Approvals, grants, the blocklist, the
//! sentinel and the audit log build on this engine (T5.3–T5.9).

mod gate;
mod policy;
mod subjects;

use std::path::Path;

use perm_rules::{Engine, Segments};

pub use gate::{Grants, PolicyGate};
pub use perm_rules::{
    ApprovalPolicy, Call, DecidedBy, Outcome, PermissionMode, Risk, RuleError, RuleList, RuleSet,
    SandboxMode, Why,
};
pub use policy::{Policy, Settings, grants_for, why_text};
pub use subjects::{
    APP, BASH, CLIPBOARD, FILE_TOOLS, INPUT, SITE, app_call, clipboard_call, file_call, grammar,
    input_call, mcp_call, plugin_call, site_call,
};

/// Tool name on a shell call. `perm-rules` compares it case-insensitively.
/// `Shell(...)` in a rule is the same tool (spec §6.1).
const SHELL: &str = BASH;

/// A command line longer than this is not parsed. Tree-sitter walks the
/// whole buffer, and the text is untrusted model input, so the cap fails
/// closed: the call is opaque `Exec` and asks unless a rule covers the tool
/// with no subject.
pub const SHELL_COMMAND_MAX_BYTES: usize = 64 * 1024;

/// `shell-classify`'s risk, as the risk `perm-rules` falls back to.
fn map_risk(risk: shell_classify::Risk) -> Risk {
    match risk {
        shell_classify::Risk::ReadOnly => Risk::ReadOnly,
        shell_classify::Risk::Write => Risk::Write,
        shell_classify::Risk::Exec => Risk::Exec,
        shell_classify::Risk::Destructive => Risk::Destructive,
    }
}

/// Rates `command` and builds the call a permission decision judges.
///
/// A line over [`SHELL_COMMAND_MAX_BYTES`] is not parsed and carries no
/// subject text, so the oversized bytes never sit on the call.
#[must_use]
pub fn shell_call(command: &str) -> Call {
    if command.len() > SHELL_COMMAND_MAX_BYTES {
        return Call {
            tool: SHELL.to_string(),
            subject: String::new(),
            risk: Risk::Exec,
            segments: Some(Segments {
                commands: Vec::new(),
                opaque: true,
            }),
        };
    }
    let split = shell_classify::segments(command);
    Call {
        tool: SHELL.to_string(),
        subject: command.to_string(),
        risk: map_risk(shell_classify::classify(command)),
        segments: Some(Segments {
            commands: split.commands,
            opaque: split.opaque,
        }),
    }
}

/// Decides a shell line under `rules` with the default settings. With no
/// matching rule the mode is [`PermissionMode::Default`], the approval
/// policy is [`ApprovalPolicy::OnRequest`], and the sandbox is
/// workspace-write: read-only runs, and write, exec and destructive ask.
///
/// # Errors
///
/// Returns [`RuleError`] when a rule string is not valid perm-rules
/// grammar. The command itself is never the error text.
pub fn decide_shell(command: &str, rules: &RuleSet) -> Result<Outcome, RuleError> {
    let engine = Engine::compile(rules, grammar(), None, Path::new("."))?;
    Ok(engine.decide(
        &shell_call(command),
        PermissionMode::Default,
        ApprovalPolicy::OnRequest,
        SandboxMode::WorkspaceWrite,
        &[],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_calls_carry_the_classified_risk_and_segments() {
        let call = shell_call("ls -la && rm -rf build");
        assert_eq!(call.tool, "bash");
        assert_eq!(call.risk, Risk::Destructive);
        let segments = call.segments.expect("segments");
        assert!(!segments.opaque);
        assert_eq!(segments.commands.len(), 2);
    }

    #[test]
    fn an_oversized_command_is_opaque_and_drops_its_text() {
        let call = shell_call(&"x".repeat(SHELL_COMMAND_MAX_BYTES + 1));
        assert!(call.subject.is_empty());
        let segments = call.segments.expect("segments");
        assert!(segments.opaque);
        assert_eq!(call.risk, Risk::Exec);
    }

    #[test]
    fn the_grammar_reads_every_tool_spelling_as_its_subject_kind() {
        let grammar = grammar();
        assert_eq!(grammar.canonical("shell"), "bash");
        assert_eq!(grammar.canonical("app_launch"), "app");
        assert_eq!(grammar.canonical("browser_open"), "site");
        assert_eq!(grammar.canonical("input_click"), "input");
        assert_eq!(grammar.canonical("clipboard_write"), "clipboard");
        assert_eq!(grammar.canonical("read_file"), "read");
        assert_eq!(
            grammar.canonical("mcp__github__create_issue"),
            "mcp__github__create_issue"
        );
    }
}
