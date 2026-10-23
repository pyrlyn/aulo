//! The tool registry: naming, risk levels and deferred tools (spec §4,
//! T4.7).
//!
//! Every tool aulo offers — built-in, `mcp__server__tool`,
//! `plugin__id__tool` — is registered once with its `llm-wire` shape, its
//! default risk and its concurrency. A deferred tool is hidden from the
//! model until `tool_search` finds it: its spec only reaches a request
//! once the session's [`Discovery`] names it.
//!
//! The registry is deterministic code outside the model. It rates a call
//! (risk and the subject approval rules match on) but decides nothing:
//! the decision is `aulo-policy`'s (T5.1).

use std::collections::{BTreeMap, BTreeSet};

use llm_wire::{Concurrency, Risk, ToolSpec};

/// The built-in search tool that reveals deferred tools.
pub const TOOL_SEARCH: &str = "tool_search";

/// Longest tool name, as the proto caps `ApprovalRequired.tool`.
pub const MAX_NAME_BYTES: usize = 128;
/// Longest one segment of an `mcp__`/`plugin__` name.
const MAX_SEGMENT_BYTES: usize = 64;
/// Longest `tool_search` query.
pub const MAX_QUERY_BYTES: usize = 256;
/// Most deferred tools one search reveals.
pub const MAX_SEARCH_RESULTS: usize = 8;
/// Most deferred tools one session may discover.
pub const MAX_DISCOVERED: usize = 32;
/// Longest tool result `tool_search` returns.
const SEARCH_OUTPUT_MAX_BYTES: usize = 4 * 1024;

/// Why a registration was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// The name is not a built-in, `mcp__server__tool` or
    /// `plugin__id__tool` name.
    #[error("tool name {name:?} is not valid: {reason}")]
    BadName {
        /// The rejected name.
        name: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// Another tool already holds the name.
    #[error("tool {0:?} is already registered")]
    Duplicate(String),
}

/// What a call's subject is, so the registry can pull it out of the
/// model-chosen input for the approval gate (spec §6.1's subject table).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubjectKind {
    /// A command line: `input.command`.
    Command,
    /// A file path: `input.path`.
    Path,
    /// A site host, from `input.url`.
    Host,
    /// An app bundle id: `input.bundle_id`.
    Bundle,
    /// The tool's own name (the default for MCP and plugin tools, whose
    /// rules match by `mcp__server__*` prefix).
    Tool,
    /// No subject: rules match the tool with no subject.
    None,
}

impl SubjectKind {
    /// Pulls the subject out of `input`. A missing or non-string field is
    /// an empty subject, which only a no-subject rule can match.
    #[must_use]
    pub fn extract(self, tool: &str, input: &serde_json::Value) -> String {
        let field = |name: &str| {
            input
                .get(name)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        match self {
            Self::Command => field("command"),
            Self::Path => field("path"),
            Self::Bundle => field("bundle_id"),
            Self::Tool => tool.to_owned(),
            Self::None => String::new(),
            Self::Host => host_of(&field("url")),
        }
    }
}

/// The host of `url`, or the empty string. No URL crate: this only has to
/// be right about the authority, and anything that is not a plain host
/// (a space, a malformed scheme) yields no host, which fails closed for a
/// `domain:` rule. Lower-cased, the way a domain rule matches.
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default();
    if host.is_empty()
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return String::new();
    }
    host.to_ascii_lowercase()
}

/// Whether `name` is a valid tool name: a built-in (`shell`,
/// `read_file`), an MCP tool (`mcp__server__tool`) or a plugin tool
/// (`plugin__id__tool`). Names come from plugins and MCP servers, so the
/// check is strict and total.
pub fn valid_name(name: &str) -> Result<(), RegistryError> {
    let bad = |reason: &'static str| RegistryError::BadName {
        name: name.to_owned(),
        reason,
    };
    if name.is_empty() {
        return Err(bad("empty"));
    }
    if name.len() > MAX_NAME_BYTES {
        return Err(bad("too long"));
    }
    if name.chars().any(char::is_control) {
        return Err(bad("control character"));
    }
    let segments: Vec<&str> = name.split("__").collect();
    match segments.as_slice() {
        [builtin] => {
            if is_segment(builtin, true) {
                Ok(())
            } else {
                Err(bad("a built-in name is [a-z][a-z0-9_]*"))
            }
        }
        [prefix, server, tool] if *prefix == "mcp" || *prefix == "plugin" => {
            if is_segment(server, false) && is_segment(tool, false) {
                Ok(())
            } else {
                Err(bad("each segment is [a-z0-9][a-z0-9_-]*"))
            }
        }
        _ => Err(bad("want a built-in name or prefix__id__tool")),
    }
}

/// One segment of a name: a built-in may start with a letter and use
/// underscores; a server or plugin id starts alphanumeric and may use
/// dashes.
fn is_segment(segment: &str, builtin: bool) -> bool {
    let max = if builtin {
        MAX_NAME_BYTES
    } else {
        MAX_SEGMENT_BYTES
    };
    if segment.is_empty() || segment.len() > max {
        return false;
    }
    let mut chars = segment.chars();
    let first = chars.next().unwrap_or_default();
    if builtin {
        if !first.is_ascii_lowercase() {
            return false;
        }
        segment
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    } else {
        if !first.is_ascii_alphanumeric() {
            return false;
        }
        segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }
}

/// The tools one daemon offers, shared by every session.
#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, RegisteredTool>,
}

/// One registered tool: the shape the model sees plus what the executor
/// needs to rate a call.
#[derive(Debug, Clone)]
pub struct RegisteredTool {
    /// The registered spec. `deferred` tools are hidden until discovered.
    pub spec: ToolSpec,
    /// How a call's subject is pulled out of the model's input.
    pub subject: SubjectKind,
}

impl ToolRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `tool`.
    ///
    /// # Errors
    ///
    /// [`RegistryError::BadName`] for a name that is not built-in, MCP or
    /// plugin shaped, and [`RegistryError::Duplicate`] when the name is
    /// taken.
    pub fn register(&mut self, tool: RegisteredTool) -> Result<(), RegistryError> {
        valid_name(&tool.spec.name)?;
        if self.tools.contains_key(&tool.spec.name) {
            return Err(RegistryError::Duplicate(tool.spec.name));
        }
        self.tools.insert(tool.spec.name.clone(), tool);
        Ok(())
    }

    /// The registered tool under `name`.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&RegisteredTool> {
        self.tools.get(name)
    }

    /// Every registered tool, by name.
    pub fn iter(&self) -> impl Iterator<Item = &RegisteredTool> {
        self.tools.values()
    }

    /// The specs a request may offer: `tool_search`, every non-deferred
    /// tool, and the deferred tools `discovery` has found. Deferred tools
    /// stay hidden until then, so the model cannot call what it has not
    /// been told exists.
    #[must_use]
    pub fn specs(&self, discovery: &Discovery) -> Vec<ToolSpec> {
        let mut specs = vec![tool_search_spec()];
        for tool in self.tools.values() {
            if !tool.spec.deferred || discovery.contains(&tool.spec.name) {
                specs.push(tool.spec.clone());
            }
        }
        specs
    }

    /// The deferred tools whose name or description matches `query`, in
    /// name order. The query is untrusted model input: capped, with
    /// control characters dropped, and at most [`MAX_SEARCH_RESULTS`]
    /// tools are returned.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&RegisteredTool> {
        let needle = clean_query(query);
        let mut found: Vec<&RegisteredTool> = self
            .tools
            .values()
            .filter(|tool| tool.spec.deferred && tool.spec.name != TOOL_SEARCH)
            .filter(|tool| {
                needle.is_empty()
                    || tool.spec.name.contains(&needle)
                    || tool.spec.description.to_lowercase().contains(&needle)
            })
            .collect();
        found.truncate(MAX_SEARCH_RESULTS);
        found
    }
}

/// A query as the matcher sees it: capped, control characters dropped,
/// lower-cased.
fn clean_query(query: &str) -> String {
    query
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_QUERY_BYTES)
        .collect::<String>()
        .to_lowercase()
}

/// The `tool_search` spec every request offers.
fn tool_search_spec() -> ToolSpec {
    ToolSpec {
        name: TOOL_SEARCH.to_owned(),
        description: "Find a hidden tool by name or description. Deferred tools only work after this call returns them.".into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Words from the tool's name or description"}
            },
            "required": ["query"]
        }),
        deferred: false,
        risk: Risk::ReadOnly,
        concurrency: Concurrency::Parallel,
    }
}

/// The deferred tools one session has found. Capped at
/// [`MAX_DISCOVERED`], so a model cannot grow a request's tool list
/// without bound. Shared per session behind an `Arc`, so the mutex is
/// interior and the set itself is not cloned.
#[derive(Debug, Default)]
pub struct Discovery {
    found: std::sync::Mutex<BTreeSet<String>>,
}

impl Discovery {
    /// An empty discovery set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `name` was found by `tool_search`.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.lock().contains(name)
    }

    /// Marks `names` discovered, up to the cap.
    pub fn discover(&self, names: &[String]) {
        let mut found = self.lock();
        for name in names {
            if found.len() >= MAX_DISCOVERED {
                break;
            }
            found.insert(name.clone());
        }
    }

    /// How many tools were discovered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether nothing was discovered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeSet<String>> {
        self.found
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The text `tool_search` returns: the found tools' names and
/// descriptions, capped like any tool output.
#[must_use]
pub fn search_result(found: &[&RegisteredTool], capped: bool) -> String {
    let mut text = String::new();
    for tool in found {
        let line = format!("{} — {}\n", tool.spec.name, tool.spec.description);
        if text.len() + line.len() > SEARCH_OUTPUT_MAX_BYTES {
            return text;
        }
        text.push_str(&line);
    }
    if text.is_empty() {
        text.push_str("no deferred tool matched");
    }
    if capped {
        text.push_str(&format!(
            "\n[discovery limit reached: at most {MAX_DISCOVERED} tools per session]"
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(name: &str, deferred: bool) -> RegisteredTool {
        RegisteredTool {
            spec: ToolSpec {
                name: name.into(),
                description: format!("the {name} tool"),
                input_schema: serde_json::json!({"type": "object"}),
                deferred,
                risk: Risk::Write,
                concurrency: Concurrency::Exclusive,
            },
            subject: SubjectKind::Tool,
        }
    }

    #[test]
    fn builtin_mcp_and_plugin_names_are_valid() {
        for name in [
            "shell",
            "read_file",
            "mcp__github__create_issue",
            "plugin__weather__forecast",
            "mcp__srv1__x",
        ] {
            assert_eq!(valid_name(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn bad_names_are_refused_with_a_reason() {
        for (name, reason) in [
            ("", "empty"),
            ("Shell", "a built-in name is [a-z][a-z0-9_]*"),
            ("mcp__x", "want a built-in name or prefix__id__tool"),
            (
                "plugin__a__b__c",
                "want a built-in name or prefix__id__tool",
            ),
            ("other__a__b", "want a built-in name or prefix__id__tool"),
            ("mcp__srv__tool\n", "control character"),
            ("mcp__srv__", "each segment is [a-z0-9][a-z0-9_-]*"),
            ("1shell", "a built-in name is [a-z][a-z0-9_]*"),
        ] {
            let err = valid_name(name).expect_err(name);
            let RegistryError::BadName { reason: got, .. } = err else {
                panic!("a name error");
            };
            assert_eq!(got, reason, "{name}");
        }
        assert!(matches!(
            valid_name(&"x".repeat(MAX_NAME_BYTES + 1)),
            Err(RegistryError::BadName {
                reason: "too long",
                ..
            })
        ));
    }

    #[test]
    fn a_duplicate_registration_is_refused() {
        let mut registry = ToolRegistry::new();
        registry.register(tool("shell", false)).expect("first");
        assert_eq!(
            registry.register(tool("shell", false)),
            Err(RegistryError::Duplicate("shell".into()))
        );
    }

    #[test]
    fn a_deferred_tool_is_hidden_until_discovered() {
        let mut registry = ToolRegistry::new();
        registry.register(tool("shell", false)).expect("shell");
        registry
            .register(tool("mcp__notes__read", true))
            .expect("deferred");
        let discovery = Discovery::new();
        let names: Vec<String> = registry
            .specs(&discovery)
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert!(names.contains(&"shell".to_owned()));
        assert!(names.contains(&TOOL_SEARCH.to_owned()));
        assert!(!names.contains(&"mcp__notes__read".to_owned()));

        let hits = registry.search("notes");
        assert_eq!(hits.len(), 1);
        let discovery = Discovery::new();
        discovery.discover(&hits.iter().map(|t| t.spec.name.clone()).collect::<Vec<_>>());
        let names: Vec<String> = registry
            .specs(&discovery)
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert!(names.contains(&"mcp__notes__read".to_owned()));
    }

    #[test]
    fn search_matches_name_and_description_and_caps_results() {
        let mut registry = ToolRegistry::new();
        for i in 0..(MAX_SEARCH_RESULTS + 3) {
            registry
                .register(tool(&format!("mcp__srv__t{i}"), true))
                .expect("register");
        }
        registry
            .register(tool("mcp__other__zzz", true))
            .expect("register");
        // An empty query lists deferred tools, capped.
        assert_eq!(registry.search("").len(), MAX_SEARCH_RESULTS);
        // A description match.
        let hits = registry.search("the mcp__other__zzz tool");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].spec.name, "mcp__other__zzz");
        // Control characters and case do not matter.
        assert_eq!(registry.search("MCP\n__OTHER").len(), 1);
    }

    #[test]
    fn a_search_never_returns_tool_search_itself() {
        let mut registry = ToolRegistry::new();
        registry.register(tool("shell", false)).expect("shell");
        assert!(registry.search("tool_search").is_empty());
        assert!(registry.search("").is_empty());
    }

    #[test]
    fn discovery_is_capped() {
        let discovery = Discovery::new();
        let names: Vec<String> = (0..(MAX_DISCOVERED + 5))
            .map(|i| format!("mcp__srv__t{i}"))
            .collect();
        discovery.discover(&names);
        assert_eq!(discovery.len(), MAX_DISCOVERED);
    }

    #[test]
    fn the_subject_comes_from_the_field_the_kind_names() {
        let input = serde_json::json!({
            "command": "ls -la",
            "path": "/tmp/x",
            "url": "https://user@Example.COM:8443/a?b",
            "bundle_id": "com.apple.Safari",
        });
        assert_eq!(SubjectKind::Command.extract("shell", &input), "ls -la");
        assert_eq!(SubjectKind::Path.extract("read", &input), "/tmp/x");
        assert_eq!(SubjectKind::Host.extract("browser", &input), "example.com");
        assert_eq!(
            SubjectKind::Bundle.extract("app", &input),
            "com.apple.Safari"
        );
        assert_eq!(SubjectKind::Tool.extract("mcp__a__b", &input), "mcp__a__b");
        assert_eq!(SubjectKind::None.extract("shell", &input), "");
        // A missing field is an empty subject: only a no-subject rule matches.
        assert_eq!(
            SubjectKind::Command.extract("shell", &serde_json::json!({})),
            ""
        );
        assert_eq!(
            SubjectKind::Host.extract("browser", &serde_json::json!({"url": "not a url"})),
            ""
        );
    }
}
