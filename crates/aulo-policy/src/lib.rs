//! Shell risk for the policy fallback (spec §7, T5.2).
//!
//! [`shell_call`] rates a command line with `shell-classify` and attaches that
//! risk, plus the simple commands the line splits into, to a `perm-rules`
//! call. [`decide_shell`] runs that call through the rule engine. When no
//! rule matches, the risk is the decision: a read-only command runs, and
//! anything else asks. Rules still win: a deny rule refuses `ls`, and an
//! allow rule can permit a destructive line.
//!
//! The broader subject table (app, site, file, MCP, input) is T5.1. This
//! crate only classifies shell lines.

use std::path::Path;

use perm_rules::{ApprovalPolicy, Engine, PermissionMode, SandboxMode, Segments};

pub use perm_rules::{Call, DecidedBy, Outcome, Risk, RuleError, RuleSet, Why};

/// Tool name on the call. `perm-rules` compares it case-insensitively.
/// `Shell(...)` in a rule is the same tool (spec §6.1).
const BASH: &str = "bash";

/// A command line longer than this is not parsed. Tree-sitter walks the
/// whole buffer, and the text is untrusted model input, so the cap fails
/// closed: the call is opaque `Exec` and asks unless a rule covers the tool
/// with no subject.
pub const SHELL_COMMAND_MAX_BYTES: usize = 64 * 1024;

/// The grammar shell rules use: Claude Code's defaults, plus `Shell` as an
/// alias of `bash` so aulo's subject name and `Bash(...)` rules hit the same
/// call.
fn grammar() -> perm_rules::Grammar {
    perm_rules::Grammar::default().alias("shell", BASH)
}

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
            tool: BASH.to_string(),
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
        tool: BASH.to_string(),
        subject: command.to_string(),
        risk: map_risk(shell_classify::classify(command)),
        segments: Some(Segments {
            commands: split.commands,
            opaque: split.opaque,
        }),
    }
}

/// Decides a shell line. `rules` are the perm-rules lists (deny, then allow,
/// then ask). With no matching rule the mode is [`PermissionMode::Default`],
/// the approval policy is [`ApprovalPolicy::OnRequest`], and the sandbox is
/// workspace-write: read-only runs, and write, exec and destructive ask.
///
/// # Errors
///
/// Returns [`RuleError`] when a rule string is not valid perm-rules grammar.
/// The command itself is never the error text.
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
