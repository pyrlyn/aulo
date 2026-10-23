//! The default request assembly for a turn: the model, the system prompt
//! and the tool specs a request offers.
//!
//! Persona and spoken-answer prompting (T4.11), compaction (T4.10) and the
//! per-tier routing (T4.5) all land on this seam; the loop only sees
//! [`agent_loop::Context`].

use std::sync::Arc;

use agent_loop::traits::{Context, Next};
use async_trait::async_trait;
use llm_wire::{Effort, Job, Message, ModelId, Request, SystemBlock, Thinking, Tier, ToolSpec};

/// Output cap until the model catalog's max output applies (the same
/// placeholder `aulo-providers` uses for a turn request).
const DEFAULT_MAX_TOKENS: u32 = 4_096;

/// Where the tool specs a request offers come from. The tool registry
/// (T4.7) implements this over the shared registry and the session's
/// discovered set, so a deferred tool only appears once found.
pub trait ToolSpecSource: Send + Sync {
    /// The specs the next request offers, in a stable order.
    fn specs(&self) -> Vec<ToolSpec>;
}

/// Builds every provider request of a session: one model, one system
/// prompt, the offered tools and the conversation so far.
#[derive(Clone)]
pub struct TurnContext {
    model: ModelId,
    system: Vec<SystemBlock>,
    max_tokens: u32,
    effort: Effort,
    tools: Vec<ToolSpec>,
    tool_source: Option<Arc<dyn ToolSpecSource>>,
}

impl std::fmt::Debug for TurnContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TurnContext")
            .field("model", &self.model)
            .field("system_blocks", &self.system.len())
            .field("tools", &self.tools.len())
            .field("tool_source", &self.tool_source.is_some())
            .finish_non_exhaustive()
    }
}

impl TurnContext {
    /// A context for `model` with no system prompt and no tools.
    #[must_use]
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: ModelId(model.into()),
            system: Vec::new(),
            max_tokens: DEFAULT_MAX_TOKENS,
            effort: Effort::Medium,
            tools: Vec::new(),
            tool_source: None,
        }
    }

    /// One cacheable system block with `text`.
    #[must_use]
    pub fn system(mut self, text: impl Into<String>) -> Self {
        self.system = vec![SystemBlock {
            text: text.into(),
            cache: true,
        }];
        self
    }

    /// The request's max output tokens.
    #[must_use]
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// The request's reasoning effort.
    #[must_use]
    pub fn effort(mut self, effort: Effort) -> Self {
        self.effort = effort;
        self
    }

    /// The tools offered when no [`Self::tool_source`] is set.
    #[must_use]
    pub fn tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = tools;
        self
    }

    /// Offers the specs `source` returns, recomputed per request.
    #[must_use]
    pub fn tool_source(mut self, source: Arc<dyn ToolSpecSource>) -> Self {
        self.tool_source = Some(source);
        self
    }
}

#[async_trait]
impl Context for TurnContext {
    async fn request(
        &self,
        _turn: agent_loop::TurnId,
        _step: u32,
        history: &mut Vec<Message>,
    ) -> Next {
        let tools = match &self.tool_source {
            Some(source) => source.specs(),
            None => self.tools.clone(),
        };
        Next::Send(Request {
            // aulo's main tier is the wire's `code` tier; llm-router keeps
            // aulo's own tier enum and the daemon maps them (T4.5).
            tier: Tier::Code,
            job: Job::Main,
            model: self.model.clone(),
            system: self.system.clone(),
            tools,
            messages: history.clone(),
            effort: self.effort,
            max_tokens: self.max_tokens,
            thinking: Thinking::Off,
            cache_breakpoints: Vec::new(),
            stop_sequences: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_wire::{Concurrency, Content, Risk, Role};

    fn spec(name: &str) -> ToolSpec {
        ToolSpec {
            name: name.into(),
            description: String::new(),
            input_schema: serde_json::json!({}),
            deferred: false,
            risk: Risk::ReadOnly,
            concurrency: Concurrency::Parallel,
        }
    }

    #[tokio::test]
    async fn the_request_carries_the_model_the_system_and_the_history() {
        let context = TurnContext::new("scripted")
            .system("be brief")
            .tools(vec![spec("echo")]);
        let mut history = vec![Message {
            role: Role::User,
            content: vec![Content::Text { text: "hi".into() }],
        }];
        let next = context
            .request(agent_loop::TurnId::new(), 1, &mut history)
            .await;
        let Next::Send(request) = next else {
            panic!("a request");
        };
        assert_eq!(request.model.0, "scripted");
        assert_eq!(request.system[0].text, "be brief");
        assert_eq!(request.tools.len(), 1);
        assert_eq!(request.max_tokens, DEFAULT_MAX_TOKENS);
        assert_eq!(request.messages, history);
    }

    #[tokio::test]
    async fn a_tool_source_is_asked_per_request() {
        struct TwoTools;
        impl ToolSpecSource for TwoTools {
            fn specs(&self) -> Vec<ToolSpec> {
                vec![spec("a"), spec("b")]
            }
        }
        let context = TurnContext::new("scripted").tool_source(Arc::new(TwoTools));
        let mut history = Vec::new();
        let Next::Send(request) = context
            .request(agent_loop::TurnId::new(), 1, &mut history)
            .await
        else {
            panic!("a request");
        };
        let names: Vec<_> = request.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
    }
}
