//! The subject table (spec §6.1): the tool name each subject kind answers
//! to, and the call a tool builds for the engine.
//!
//! One tool name per subject keeps the rule grammar small: a rule names
//! the subject kind (`App(com.apple.Safari)`, `Site(domain:example.com)`,
//! `Edit(src/**)`, `mcp__github__*`) and matches every tool of that kind,
//! however the tool itself is spelled.

use perm_rules::{Call, Risk};

/// The shell tool. Canonical; `shell` is an alias onto it (T5.2).
pub const BASH: &str = "bash";
/// App control: launch, quit, focus, list (spec §6.1's `App(id)`).
pub const APP: &str = "app";
/// Browser control, whatever the backend (spec §6.1's `Site(host)`).
pub const SITE: &str = "site";
/// Synthetic input: click, type, key, scroll (spec §6.1's `Input(kind)`).
pub const INPUT: &str = "input";
/// Clipboard read and write (spec §6.1's `Clipboard(read/write)`).
pub const CLIPBOARD: &str = "clipboard";

/// The file tools, which take path globs (perm-rules' own path tools).
pub const FILE_TOOLS: &[&str] = &[
    "read",
    "edit",
    "write",
    "grep",
    "glob",
    "outline",
    "apply_patch",
];

/// The grammar aulo's rules are written in: Claude Code's set, `shell` as
/// an alias of `bash`, and the app, site, input and clipboard tool
/// families folded onto their canonical names, so a rule names the
/// subject kind rather than each tool spelling.
///
/// `app` takes path globs, because a bundle family is written as one:
/// `App(com.apple.*)` covers every Apple app, `App(com.apple.Safari)` one.
pub fn grammar() -> perm_rules::Grammar {
    perm_rules::Grammar::default()
        .alias("shell", BASH)
        .alias("read_file", "read")
        .alias("write_file", "write")
        .alias("edit_file", "edit")
        .alias("app_launch", APP)
        .alias("app_quit", APP)
        .alias("app_focus", APP)
        .alias("app_list", APP)
        .alias("browser_open", SITE)
        .alias("browser_read", SITE)
        .alias("browser_snapshot", SITE)
        .alias("browser_click", SITE)
        .alias("browser_type", SITE)
        .alias("browser_scroll", SITE)
        .alias("input_click", INPUT)
        .alias("input_type", INPUT)
        .alias("input_key", INPUT)
        .alias("input_scroll", INPUT)
        .alias("clipboard_read", CLIPBOARD)
        .alias("clipboard_write", CLIPBOARD)
        .path_tool(APP)
}

/// A call on one app, by bundle id. Driving an app runs a process, so the
/// risk fallback asks.
#[must_use]
pub fn app_call(bundle: &str) -> Call {
    Call::new(APP, bundle, Risk::Exec)
}

/// A call on one site, by host. Reading a page changes nothing, so the
/// risk fallback allows; per-site asks come from rules (T5.6).
#[must_use]
pub fn site_call(host: &str) -> Call {
    Call::new(SITE, host, Risk::ReadOnly)
}

/// A call on one file. Reads are read-only; writes and edits are writes.
#[must_use]
pub fn file_call(tool: &str, path: &str) -> Call {
    let risk = if tool == "read" || tool == "grep" || tool == "glob" || tool == "outline" {
        Risk::ReadOnly
    } else {
        Risk::Write
    };
    Call::new(tool, path, risk)
}

/// A call to one MCP tool, `mcp__server__tool`. A third-party tool can do
/// anything, so the risk fallback asks.
#[must_use]
pub fn mcp_call(server: &str, tool: &str) -> Call {
    Call::new(
        format!("mcp__{server}__{tool}"),
        format!("mcp__{server}__{tool}"),
        Risk::Exec,
    )
}

/// A call to one plugin tool, `plugin__id__tool`, rated like an MCP tool.
#[must_use]
pub fn plugin_call(id: &str, tool: &str) -> Call {
    Call::new(
        format!("plugin__{id}__{tool}"),
        format!("plugin__{id}__{tool}"),
        Risk::Exec,
    )
}

/// A synthetic input action. Typing and clicking act as the user, so the
/// risk fallback asks.
#[must_use]
pub fn input_call(kind: &str) -> Call {
    Call::new(INPUT, kind, Risk::Exec)
}

/// A clipboard action. Reading is read-only; writing is a write.
#[must_use]
pub fn clipboard_call(action: &str) -> Call {
    let risk = if action == "read" {
        Risk::ReadOnly
    } else {
        Risk::Write
    };
    Call::new(CLIPBOARD, action, risk)
}
