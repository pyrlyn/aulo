//! A deferred tool reaches the model only after `tool_search` found it
//! (T4.7): proven through a full session turn, so the specs the second
//! provider request carries are the ones the registry offered after the
//! search.

use std::sync::Arc;

use aulo_agent::{
    Discovery, EVENT_CHANNEL, RegisteredTool, RegistryTools, Session, SessionConfig, SessionEvent,
    SessionParts, Submission, TOOL_SEARCH, ToolExecutor, ToolRegistry, TurnContext, TurnStop,
};
use aulo_testkit::Scripted;
use aulo_types::ChatId;
use llm_wire::{Concurrency, Risk, ToolSpec};
use tokio::sync::mpsc;

use agent_loop::traits::{Approvals, ToolCx, Verdict};
use agent_loop::types::{ToolCall, ToolResult};

struct AllowAll;

#[async_trait::async_trait]
impl Approvals for AllowAll {
    async fn decide(&self, _call: &ToolCall) -> Verdict {
        Verdict::Allow
    }
}

/// Records what ran and answers with a fixed payload.
struct Notes;

#[async_trait::async_trait]
impl ToolExecutor for Notes {
    async fn execute(&self, call: &ToolCall, _cx: &ToolCx) -> ToolResult {
        ToolResult {
            ok: true,
            visible: format!("contents of {}", call.name),
            archive: None,
            bytes: 0,
            duration_ms: 1,
            diff: None,
            structured: None,
        }
    }
}

fn tool(name: &str, deferred: bool, risk: Risk) -> RegisteredTool {
    RegisteredTool {
        spec: ToolSpec {
            name: name.into(),
            description: format!("the {name} tool"),
            input_schema: serde_json::json!({"type": "object"}),
            deferred,
            risk,
            concurrency: Concurrency::Exclusive,
        },
        subject: aulo_agent::SubjectKind::Tool,
    }
}

#[tokio::test]
async fn a_deferred_tool_appears_only_after_tool_search() {
    let mut registry = ToolRegistry::new();
    registry
        .register(tool("shell", false, Risk::Exec))
        .expect("shell");
    registry
        .register(tool("mcp__notes__read", true, Risk::ReadOnly))
        .expect("deferred");
    let registry = Arc::new(registry);
    let tools = RegistryTools::new(
        Arc::clone(&registry),
        Arc::new(Discovery::new()),
        Arc::new(Notes),
    );

    let provider = Arc::new(
        Scripted::from_toml(
            r#"
[[turn]]
tool_calls = [{ name = "tool_search", input = { query = "notes" } }]

[[turn]]
tool_calls = [{ name = "mcp__notes__read", input = {} }]

[[turn]]
text = "the note says hi"
"#,
        )
        .expect("scenario"),
    );
    let (outbox, mut events) = mpsc::channel(EVENT_CHANNEL);
    let session = Session::new(
        ChatId::new(),
        SessionParts {
            provider: Arc::clone(&provider) as Arc<dyn llm_wire::Provider>,
            tools: Arc::new(tools.clone()),
            approvals: Arc::new(AllowAll),
            context: Arc::new(
                TurnContext::new("scripted")
                    .system("you are aulo")
                    .tool_source(Arc::new(tools)),
            ),
        },
        SessionConfig::default(),
        outbox,
    );

    session
        .submit(Submission::UserTurn {
            text: "read my note".into(),
            attachments: vec![],
        })
        .await
        .expect("the turn runs");

    let mut seen = Vec::new();
    while let Ok(event) = events.try_recv() {
        seen.push(event);
    }
    assert!(matches!(
        seen.last(),
        Some(SessionEvent::TurnFinished {
            stop: TurnStop::Completed,
            ..
        })
    ));
    // The search itself ran and found the deferred tool.
    assert!(seen.iter().any(|e| matches!(
        e,
        SessionEvent::ToolCallFinished { ok: true, output, .. }
            if output.as_deref().is_some_and(|t| t.contains("mcp__notes__read"))
    )));
    // The deferred tool then ran for real.
    assert!(seen.iter().any(|e| matches!(
        e,
        SessionEvent::ToolCallFinished { ok: true, output, .. }
            if output.as_deref() == Some("contents of mcp__notes__read")
    )));

    // The requests show the reveal: the first offers tool_search without
    // the deferred tool, the second offers both.
    let requests = provider.requests();
    assert_eq!(requests.len(), 3);
    let names = |index: usize| -> Vec<String> {
        requests[index]
            .tools
            .iter()
            .map(|t| t.name.clone())
            .collect()
    };
    let first = names(0);
    assert!(first.contains(&TOOL_SEARCH.to_owned()));
    assert!(first.contains(&"shell".to_owned()));
    assert!(!first.contains(&"mcp__notes__read".to_owned()));
    let second = names(1);
    assert!(second.contains(&TOOL_SEARCH.to_owned()));
    assert!(second.contains(&"mcp__notes__read".to_owned()));
}
