//! Rules a manifest must satisfy beyond its shape. A plugin manifest is written
//! by a third party and decides what the host launches and what it grants, so
//! each rule fails closed and every string is bounded.

use std::net::IpAddr;

use super::PluginManifest;
use super::limits::{MAX_ARGS, MAX_FS_ROOTS, MAX_HOST_LEN, MAX_HOSTS, MAX_ID_LEN, MAX_STRING_LEN};
use super::model::{Capabilities, Entry, Kind, Process};

/// The first violated rule, as a message that names the offending field.
pub(super) fn check(m: &PluginManifest) -> Result<(), String> {
    if !valid_id(&m.id) {
        return Err(format!(
            "id {}: use 1-{MAX_ID_LEN} lowercase letters, digits and single hyphens, starting with a letter",
            show(&m.id)
        ));
    }
    if m.kinds.is_empty() {
        return Err("kinds: declare at least one kind".into());
    }
    check_entry(m)?;
    check_capabilities(&m.capabilities)
}

/// One entry serves one ABI: gRPC for streaming kinds, MCP or WASM for tools.
/// Kinds that need different ABIs cannot share a plugin, and an entry that
/// serves none of the declared kinds is a mistake worth refusing.
fn check_entry(m: &PluginManifest) -> Result<(), String> {
    let streaming = m.kinds.iter().any(|k| k.is_streaming());
    let tools = m.kinds.contains(&Kind::Tools);
    let entry = m.entry.as_ref();
    match (streaming, tools, entry) {
        (true, true, _) => Err(
            "kinds: tools (MCP or WASM) cannot share a plugin with stt, tts or llm-provider (gRPC); split them"
                .into(),
        ),
        (true, false, Some(Entry::GrpcProcess(p))) | (false, true, Some(Entry::McpProcess(p))) => {
            check_process(p)
        }
        (true, false, _) => Err("entry: stt, tts and llm-provider need type = \"grpc-process\"".into()),
        (false, true, Some(Entry::Wasm { module })) => check_relative("entry.module", module),
        (false, true, _) => Err("entry: tools need type = \"mcp-process\" or \"wasm\"".into()),
        (false, false, None) => Ok(()),
        (false, false, Some(Entry::Wasm { module })) if m.kinds.contains(&Kind::Hooks) => {
            check_relative("entry.module", module)
        }
        (false, false, Some(_)) => {
            Err("entry: skills need no entry; hooks may only use type = \"wasm\"".into())
        }
    }
}

fn check_process(p: &Process) -> Result<(), String> {
    if p.command.contains(['/', '\\']) {
        check_relative("entry.command", &p.command)?;
    } else {
        check_text("entry.command", &p.command)?;
    }
    if p.args.len() > MAX_ARGS {
        return Err(format!("entry.args: at most {MAX_ARGS} arguments"));
    }
    p.args
        .iter()
        .try_for_each(|a| check_text_allow_empty("entry.args", a))
}

fn check_capabilities(c: &Capabilities) -> Result<(), String> {
    if c.net.len() > MAX_HOSTS {
        return Err(format!("capabilities.net: at most {MAX_HOSTS} hosts"));
    }
    if let Some(h) = c.net.iter().find(|h| !valid_host(h)) {
        return Err(format!(
            "capabilities.net {}: expected a lowercase host name, an IP address or *.domain.tld without scheme, port or path",
            show(h)
        ));
    }
    for (field, roots) in [
        ("capabilities.fs_read", &c.fs_read),
        ("capabilities.fs_write", &c.fs_write),
    ] {
        if roots.len() > MAX_FS_ROOTS {
            return Err(format!("{field}: at most {MAX_FS_ROOTS} roots"));
        }
        roots.iter().try_for_each(|r| check_root(field, r))?;
    }
    Ok(())
}

fn valid_id(id: &str) -> bool {
    id.len() <= MAX_ID_LEN
        && id.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && !id.ends_with('-')
        && !id.contains("--")
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn valid_host(host: &str) -> bool {
    if host.len() > MAX_HOST_LEN {
        return false;
    }
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }
    let (wildcard, name) = host
        .strip_prefix("*.")
        .map_or((false, host), |rest| (true, rest));
    let labels: Vec<&str> = name.split('.').collect();
    // `*.com` would grant a whole TLD; require a registrable domain below the wildcard.
    // An all-digit last label is a malformed IPv4 address, not a DNS name.
    labels.iter().all(|l| valid_label(l))
        && (!wildcard || labels.len() >= 2)
        && labels
            .last()
            .is_some_and(|l| !l.bytes().all(|b| b.is_ascii_digit()))
}

fn valid_label(label: &str) -> bool {
    (1..=63).contains(&label.len())
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Absolute on any platform, not just the host's, so one manifest means the
/// same thing everywhere. `..` is refused instead of normalised because a
/// grant must not depend on how a path is later resolved.
fn check_root(field: &str, root: &str) -> Result<(), String> {
    check_text(field, root)?;
    if !is_absolute(root) {
        return Err(format!(
            "{field} {}: roots must be absolute paths",
            show(root)
        ));
    }
    let mut segments = segments(root);
    if root.as_bytes().get(1) == Some(&b':') {
        segments.next(); // drive letter
    }
    if segments.next().is_none() {
        return Err(format!(
            "{field} {}: a filesystem root is too broad",
            show(root)
        ));
    }
    no_dot_segments(field, root)
}

fn check_relative(field: &str, path: &str) -> Result<(), String> {
    check_text(field, path)?;
    if is_absolute(path) {
        return Err(format!(
            "{field} {}: must be relative to the plugin directory",
            show(path)
        ));
    }
    no_dot_segments(field, path)
}

fn is_absolute(path: &str) -> bool {
    let b = path.as_bytes();
    path.starts_with(['/', '\\'])
        || (b.first().is_some_and(u8::is_ascii_alphabetic)
            && b.get(1) == Some(&b':')
            && matches!(b.get(2), Some(b'/' | b'\\')))
}

fn segments(path: &str) -> impl Iterator<Item = &str> {
    path.split(['/', '\\']).filter(|s| !s.is_empty())
}

fn no_dot_segments(field: &str, path: &str) -> Result<(), String> {
    if segments(path).any(|s| s == ".." || s == ".") {
        return Err(format!(
            "{field} {}: `.` and `..` segments are not allowed",
            show(path)
        ));
    }
    Ok(())
}

fn check_text(field: &str, s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err(format!("{field}: must not be empty"));
    }
    check_text_allow_empty(field, s)
}

fn check_text_allow_empty(field: &str, s: &str) -> Result<(), String> {
    if s.len() > MAX_STRING_LEN {
        return Err(format!("{field}: longer than {MAX_STRING_LEN} bytes"));
    }
    if s.chars().any(char::is_control) {
        return Err(format!("{field}: control characters are not allowed"));
    }
    Ok(())
}

/// Quoted and escaped so a hostile value cannot forge log lines, and cut short
/// so it cannot flood them.
fn show(s: &str) -> String {
    let head: String = s.chars().take(48).collect();
    let more = if head.len() < s.len() { "…" } else { "" };
    format!("{head:?}{more}")
}
