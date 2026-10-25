//! Running registered tools: the `agent_loop::Tools` implementation over
//! the registry, plus the seam real tools plug into.
//!
//! [`RegistryTools`] rates calls from the registry, runs `tool_search`
//! itself and hands everything else to a [`ToolExecutor`]. The daemon
//! wires the executor to the real tool backends (shell, files, browser,
//! MCP, plugins); tests use fakes. An unknown tool is never run: the loop
//! answers it with a failed result.

use std::sync::Arc;

use agent_loop::traits::{Rating, ToolCx, Tools};
use async_trait::async_trait;
use llm_wire::{Concurrency, Risk};
use serde_json::Value;

use crate::context::ToolSpecSource;
use crate::registry::{Discovery, TOOL_SEARCH, ToolRegistry, search_result};
use agent_loop::types::{ToolCall, ToolResult};

/// Runs an approved call the registry does not handle itself.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Runs `call`. Its output is untrusted; the loop caps what the model
    /// sees.
    async fn execute(&self, call: &ToolCall, cx: &ToolCx) -> ToolResult;
}

/// The registry as the loop's tool executor, plus the per-session
/// discovery set that decides which deferred tools a request offers.
#[derive(Clone)]
pub struct RegistryTools {
    registry: Arc<ToolRegistry>,
    discovery: Arc<Discovery>,
    executor: Arc<dyn ToolExecutor>,
}

impl std::fmt::Debug for RegistryTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistryTools")
            .field("tools", &self.registry.iter().count())
            .field("discovered", &self.discovery.len())
            .finish_non_exhaustive()
    }
}

impl RegistryTools {
    /// Wires the registry, the session's discovery set and the executor
    /// that runs the real tools.
    #[must_use]
    pub fn new(
        registry: Arc<ToolRegistry>,
        discovery: Arc<Discovery>,
        executor: Arc<dyn ToolExecutor>,
    ) -> Self {
        Self {
            registry,
            discovery,
            executor,
        }
    }

    /// The registry this executor serves.
    #[must_use]
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// The session's discovery set.
    #[must_use]
    pub fn discovery(&self) -> &Discovery {
        &self.discovery
    }
}

#[async_trait]
impl Tools for RegistryTools {
    fn rate(&self, name: &str, input: &Value) -> Option<Rating> {
        if name == TOOL_SEARCH {
            let query = input
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default();
            return Some(Rating {
                risk: Risk::ReadOnly,
                subject: query.to_owned(),
                segments: None,
            });
        }
        let tool = self.registry.get(name)?;
        Some(Rating {
            risk: tool.spec.risk,
            subject: tool.subject.extract(name, input),
            segments: None,
        })
    }

    fn concurrency(&self, call: &ToolCall) -> Concurrency {
        self.registry
            .get(&call.name)
            .map_or(Concurrency::Exclusive, |tool| tool.spec.concurrency)
    }

    async fn run(&self, call: &ToolCall, cx: ToolCx) -> ToolResult {
        if call.name == TOOL_SEARCH {
            return self.search(call);
        }
        if self.registry.get(&call.name).is_none() {
            // The loop already answers unknown tools without running them;
            // this is the belt to that braces.
            return ToolResult::failed(&format!("unknown tool {}", call.name));
        }
        self.executor.execute(call, &cx).await
    }
}

impl RegistryTools {
    /// Reveals the deferred tools matching the model's query and marks
    /// them discovered, so the next request offers them.
    fn search(&self, call: &ToolCall) -> ToolResult {
        let query = call
            .input
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let found = self.registry.search(query);
        let names: Vec<String> = found.iter().map(|t| t.spec.name.clone()).collect();
        let capped = names.len() + self.discovery.len() > crate::registry::MAX_DISCOVERED;
        self.discovery.discover(&names);
        ToolResult {
            ok: true,
            visible: search_result(&found, capped),
            archive: None,
            bytes: 0,
            duration_ms: 0,
            diff: None,
            structured: None,
        }
    }
}

impl ToolSpecSource for RegistryTools {
    fn specs(&self) -> Vec<llm_wire::ToolSpec> {
        self.registry.specs(&self.discovery)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{MAX_DISCOVERED, RegisteredTool, SubjectKind};
    use llm_wire::ToolSpec;
    use tokio_util::sync::CancellationToken;

    fn spec(name: &str, deferred: bool, subject: SubjectKind) -> RegisteredTool {
        RegisteredTool {
            spec: ToolSpec {
                name: name.into(),
                description: format!("the {name} tool"),
                input_schema: serde_json::json!({"type": "object"}),
                deferred,
                risk: Risk::ReadOnly,
                concurrency: Concurrency::Parallel,
            },
            subject,
        }
    }

    fn tools(registry: ToolRegistry) -> RegistryTools {
        RegistryTools::new(
            Arc::new(registry),
            Arc::new(Discovery::new()),
            Arc::new(NoExecutor),
        )
    }

    struct NoExecutor;

    #[async_trait]
    impl ToolExecutor for NoExecutor {
        async fn execute(&self, _call: &ToolCall, _cx: &ToolCx) -> ToolResult {
            ToolResult::failed("not run")
        }
    }

    struct Recording;

    #[async_trait]
    impl ToolExecutor for Recording {
        async fn execute(&self, call: &ToolCall, _cx: &ToolCx) -> ToolResult {
            ToolResult {
                ok: true,
                visible: format!("ran {}", call.name),
                archive: None,
                bytes: 0,
                duration_ms: 0,
                diff: None,
                structured: None,
            }
        }
    }

    fn call(name: &str, input: Value) -> ToolCall {
        ToolCall {
            id: llm_wire::CallId::new(),
            name: name.into(),
            input,
            risk: Risk::ReadOnly,
            subject: String::new(),
            segments: None,
        }
    }

    fn cx() -> ToolCx {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        ToolCx {
            turn: agent_loop::TurnId::new(),
            cancel: CancellationToken::new(),
            output: tx,
        }
    }

    #[test]
    fn an_unknown_tool_is_never_rated() {
        let registry = tools(ToolRegistry::new());
        assert!(registry.rate("nope", &serde_json::json!({})).is_none());
    }

    #[test]
    fn tool_search_is_rated_read_only_with_the_query_as_subject() {
        let registry = tools(ToolRegistry::new());
        let rating = registry
            .rate(TOOL_SEARCH, &serde_json::json!({"query": "notes"}))
            .expect("rated");
        assert_eq!(rating.risk, Risk::ReadOnly);
        assert_eq!(rating.subject, "notes");
    }

    #[test]
    fn a_registered_tool_is_rated_with_its_risk_and_subject() {
        let mut registry = ToolRegistry::new();
        registry
            .register(spec("shell", false, SubjectKind::Command))
            .expect("register");
        let registry = tools(registry);
        let rating = registry
            .rate("shell", &serde_json::json!({"command": "ls"}))
            .expect("rated");
        assert_eq!(rating.risk, Risk::ReadOnly);
        assert_eq!(rating.subject, "ls");
        assert_eq!(
            registry.concurrency(&call("shell", serde_json::json!({}))),
            Concurrency::Parallel
        );
    }

    #[tokio::test]
    async fn a_tool_search_call_reveals_the_deferred_tool_for_the_next_request() {
        let mut registry = ToolRegistry::new();
        registry
            .register(spec("shell", false, SubjectKind::Command))
            .expect("register");
        registry
            .register(spec("mcp__notes__read", true, SubjectKind::Tool))
            .expect("register");
        let registry = Arc::new(registry);
        let discovery = Arc::new(Discovery::new());
        let executor = RegistryTools::new(
            Arc::clone(&registry),
            Arc::clone(&discovery),
            Arc::new(Recording),
        );

        // Before the search the deferred tool is not offered.
        let names: Vec<String> = executor.specs().into_iter().map(|s| s.name).collect();
        assert!(!names.contains(&"mcp__notes__read".to_owned()));

        let result = executor
            .run(
                &call(TOOL_SEARCH, serde_json::json!({"query": "notes"})),
                cx(),
            )
            .await;
        assert!(result.ok);
        assert!(result.visible.contains("mcp__notes__read"));

        // After it, the next request offers it.
        let names: Vec<String> = executor.specs().into_iter().map(|s| s.name).collect();
        assert!(names.contains(&"mcp__notes__read".to_owned()));
    }

    #[tokio::test]
    async fn an_approved_call_reaches_the_executor() {
        let mut registry = ToolRegistry::new();
        registry
            .register(spec("shell", false, SubjectKind::Command))
            .expect("register");
        let executor = RegistryTools::new(
            Arc::new(registry),
            Arc::new(Discovery::new()),
            Arc::new(Recording),
        );
        let result = executor
            .run(&call("shell", serde_json::json!({"command": "ls"})), cx())
            .await;
        assert_eq!(result.visible, "ran shell");
    }

    #[tokio::test]
    async fn a_search_past_the_discovery_cap_says_so() {
        let mut registry = ToolRegistry::new();
        for i in 0..(MAX_DISCOVERED + 2) {
            registry
                .register(spec(&format!("mcp__srv__n{i:02}"), true, SubjectKind::Tool))
                .expect("register");
        }
        let executor = RegistryTools::new(
            Arc::new(registry),
            Arc::new(Discovery::new()),
            Arc::new(NoExecutor),
        );
        // Each query finds exactly one tool, so the cap is reached by
        // repeated searches rather than by one big result.
        let mut last = None;
        for i in 0..(MAX_DISCOVERED + 2) {
            last = Some(
                executor
                    .run(
                        &call(
                            TOOL_SEARCH,
                            serde_json::json!({"query": format!("n{i:02}")}),
                        ),
                        cx(),
                    )
                    .await,
            );
        }
        let last = last.expect("a search");
        assert!(last.visible.contains("discovery limit reached"));
        assert_eq!(executor.discovery().len(), MAX_DISCOVERED);
    }
}
