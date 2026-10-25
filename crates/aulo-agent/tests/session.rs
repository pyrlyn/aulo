//! Scripted-provider tests for one chat session (T4.6): a text turn, a
//! tool turn and an interrupt. The provider is `aulo-testkit`'s
//! [`Scripted`], so no network, key or device is involved.

use std::sync::Arc;
use std::time::Duration;

use aulo_agent::{
    ApprovalChoice, EVENT_CHANNEL, Session, SessionConfig, SessionEvent, SessionParts, Submission,
    TurnContext, TurnStop,
};
use aulo_testkit::Scripted;
use aulo_types::ChatId;
use llm_wire::{Concurrency, Risk, ToolSpec};
use tokio::sync::mpsc;

use agent_loop::traits::{Approvals, Rating, ToolCx, Tools, Verdict};
use agent_loop::types::{ToolCall, ToolResult};

/// How long a test waits for the turn it spawned before failing.
const TURN_TIMEOUT: Duration = Duration::from_secs(5);

/// A gate that allows everything: policy is T5.1's, not the session's.
struct AllowAll;

#[async_trait::async_trait]
impl Approvals for AllowAll {
    async fn decide(&self, _call: &ToolCall) -> Verdict {
        Verdict::Allow
    }
}

/// A read-only tool that echoes its `text` argument back.
struct Echo;

#[async_trait::async_trait]
impl Tools for Echo {
    fn rate(&self, name: &str, _input: &serde_json::Value) -> Option<Rating> {
        (name == "echo").then(|| Rating {
            risk: Risk::ReadOnly,
            subject: "echo".into(),
            segments: None,
        })
    }

    fn concurrency(&self, _call: &ToolCall) -> Concurrency {
        Concurrency::Parallel
    }

    async fn run(&self, call: &ToolCall, _cx: ToolCx) -> ToolResult {
        let text = call
            .input
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let bytes = text.len() as u64;
        ToolResult {
            ok: true,
            visible: text,
            archive: None,
            bytes,
            duration_ms: 1,
            diff: None,
            structured: None,
        }
    }
}

/// An exclusive tool that runs until the turn is cancelled.
struct Slow;

#[async_trait::async_trait]
impl Tools for Slow {
    fn rate(&self, name: &str, _input: &serde_json::Value) -> Option<Rating> {
        (name == "slow").then(|| Rating {
            risk: Risk::Exec,
            subject: "slow".into(),
            segments: None,
        })
    }

    fn concurrency(&self, _call: &ToolCall) -> Concurrency {
        Concurrency::Exclusive
    }

    async fn run(&self, _call: &ToolCall, cx: ToolCx) -> ToolResult {
        cx.cancel.cancelled().await;
        ToolResult::failed("cancelled")
    }
}

/// A session plus the receiving end of its event channel.
fn session(
    provider: Scripted,
    tools: Arc<dyn Tools>,
) -> (Session, mpsc::Receiver<SessionEvent>, Arc<Scripted>) {
    let provider = Arc::new(provider);
    let (outbox, events) = mpsc::channel(EVENT_CHANNEL);
    let session = Session::new(
        ChatId::new(),
        SessionParts {
            provider: Arc::clone(&provider) as Arc<dyn llm_wire::Provider>,
            tools,
            approvals: Arc::new(AllowAll),
            context: Arc::new(
                TurnContext::new("scripted")
                    .system("you are aulo")
                    .tools(vec![echo_spec()]),
            ),
        },
        SessionConfig::default(),
        outbox,
    );
    (session, events, provider)
}

fn echo_spec() -> ToolSpec {
    ToolSpec {
        name: "echo".into(),
        description: "echoes text".into(),
        input_schema: serde_json::json!({"type": "object"}),
        deferred: false,
        risk: Risk::ReadOnly,
        concurrency: Concurrency::Parallel,
    }
}

/// Everything already buffered on the channel. The loop sends inline, so
/// once `submit` returned every event of the turn is here.
fn drain(events: &mut mpsc::Receiver<SessionEvent>) -> Vec<SessionEvent> {
    let mut drained = Vec::new();
    while let Ok(event) = events.try_recv() {
        drained.push(event);
    }
    drained
}

#[tokio::test]
async fn a_text_turn_streams_the_reply_and_finishes() {
    let provider = Scripted::from_toml("[[turn]]\ntext = \"hello there\"\n").expect("scenario");
    let (session, mut events, scripted) = session(provider, Arc::new(Echo));

    session
        .submit(Submission::UserTurn {
            text: "hi".into(),
            attachments: vec![],
        })
        .await
        .expect("the turn runs");

    let events = drain(&mut events);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SessionEvent::TurnStarted { seq: 1, .. })),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SessionEvent::TextDelta { text, .. } if text == "hello there")),
        "{events:?}"
    );
    assert_eq!(
        events.last(),
        Some(&SessionEvent::TurnFinished {
            turn: events
                .iter()
                .find_map(|e| match e {
                    SessionEvent::TurnStarted { turn, .. } => Some(*turn),
                    _ => None,
                })
                .expect("a turn id"),
            stop: TurnStop::Completed,
        })
    );
    // One provider call, and the request carried the user text.
    let requests = scripted.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].messages.iter().any(|m| {
        m.content
            .iter()
            .any(|c| matches!(c, llm_wire::Content::Text { text } if text == "hi"))
    }));
    assert_eq!(session.state(), aulo_agent::SessionState::Idle);
}

#[tokio::test]
async fn a_tool_turn_runs_the_call_and_feeds_its_result_back() {
    let provider = Scripted::from_toml(
        r#"
[[turn]]
tool_calls = [{ name = "echo", input = { text = "hi" } }]

[[turn]]
text = "done"
"#,
    )
    .expect("scenario");
    let (session, mut events, scripted) = session(provider, Arc::new(Echo));

    session
        .submit(Submission::UserTurn {
            text: "echo hi".into(),
            attachments: vec![],
        })
        .await
        .expect("the turn runs");

    let events = drain(&mut events);
    let started = events
        .iter()
        .find_map(|e| match e {
            SessionEvent::ToolCallStarted { call, tool, .. } if tool == "echo" => Some(*call),
            _ => None,
        })
        .expect("the tool call started");
    assert!(
        events.iter().any(
            |e| matches!(e, SessionEvent::ToolCallFinished { call, ok: true, output, .. } if *call == started && output.as_deref() == Some("hi"))
        ),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SessionEvent::TextDelta { text, .. } if text == "done")),
        "{events:?}"
    );
    assert!(matches!(
        events.last(),
        Some(SessionEvent::TurnFinished {
            stop: TurnStop::Completed,
            ..
        })
    ));
    // The second provider call saw the tool result in its history.
    let requests = scripted.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].messages.iter().any(|m| m.content.iter().any(
        |c| matches!(c, llm_wire::Content::ToolResult { content, is_error: false, .. } if content == "hi")
    )));
}

#[tokio::test]
async fn an_interrupt_cancels_a_running_tool_and_the_turn() {
    let provider =
        Scripted::from_toml("[[turn]]\ntool_calls = [{ name = \"slow\", input = {} }]\n")
            .expect("scenario");
    let (session, mut events, _scripted) = session(provider, Arc::new(Slow));
    // A clone shares the conversation, so the turn runs in its own task
    // while this one interrupts.
    let running = {
        let session = session.clone();
        tokio::spawn(async move {
            session
                .submit(Submission::UserTurn {
                    text: "go".into(),
                    attachments: vec![],
                })
                .await
        })
    };

    // Wait for the tool to start, then cancel.
    let mut seen = Vec::new();
    loop {
        let event = tokio::time::timeout(TURN_TIMEOUT, events.recv())
            .await
            .expect("an event arrives")
            .expect("the channel stays open");
        let started = matches!(event, SessionEvent::ToolCallStarted { .. });
        seen.push(event);
        if started {
            break;
        }
    }
    let asked = std::time::Instant::now();
    session.interrupt();
    tokio::time::timeout(TURN_TIMEOUT, running)
        .await
        .expect("the turn ends")
        .expect("the task joins")
        .expect("the turn returns");
    let elapsed = asked.elapsed();
    assert!(elapsed < Duration::from_millis(200), "took {elapsed:?}");

    while let Ok(event) = events.try_recv() {
        seen.push(event);
    }
    assert!(
        seen.iter().any(
            |e| matches!(e, SessionEvent::ToolCallFinished { ok: false, output, .. } if output.as_deref() == Some("cancelled"))
        ),
        "{seen:?}"
    );
    assert_eq!(
        seen.last(),
        Some(&SessionEvent::TurnFinished {
            turn: seen
                .iter()
                .find_map(|e| match e {
                    SessionEvent::TurnStarted { turn, .. } => Some(*turn),
                    _ => None,
                })
                .expect("a turn id"),
            stop: TurnStop::Interrupted,
        })
    );
    assert_eq!(session.state(), aulo_agent::SessionState::Idle);
}

#[tokio::test]
async fn the_step_limit_ends_the_turn_before_the_next_call() {
    let provider = Scripted::from_toml(
        r#"
[[turn]]
tool_calls = [{ name = "echo", input = { text = "1" } }]

[[turn]]
text = "should not be reached"
"#,
    )
    .expect("scenario");
    let provider = Arc::new(provider);
    let (outbox, mut events) = mpsc::channel(EVENT_CHANNEL);
    let session = Session::new(
        ChatId::new(),
        SessionParts {
            provider: Arc::clone(&provider) as Arc<dyn llm_wire::Provider>,
            tools: Arc::new(Echo),
            approvals: Arc::new(AllowAll),
            context: Arc::new(
                TurnContext::new("scripted")
                    .system("you are aulo")
                    .tools(vec![echo_spec()]),
            ),
        },
        SessionConfig {
            max_steps: 1,
            ..SessionConfig::default()
        },
        outbox,
    );

    session
        .submit(Submission::UserTurn {
            text: "loop".into(),
            attachments: vec![],
        })
        .await
        .expect("the turn runs");

    let events = drain(&mut events);
    assert!(matches!(
        events.last(),
        Some(SessionEvent::TurnFinished {
            stop: TurnStop::MaxSteps,
            ..
        })
    ));
    assert_eq!(provider.requests().len(), 1);
}

#[tokio::test]
async fn an_approval_answer_runs_the_call_it_names() {
    let provider = Scripted::from_toml(
        r#"
[[turn]]
tool_calls = [{ name = "echo", input = { text = "hi" } }]

[[turn]]
text = "done"
"#,
    )
    .expect("scenario");
    let provider = Arc::new(provider);
    let (outbox, mut events) = mpsc::channel(EVENT_CHANNEL);
    let session = Session::new(
        ChatId::new(),
        SessionParts {
            provider: Arc::clone(&provider) as Arc<dyn llm_wire::Provider>,
            tools: Arc::new(Echo),
            // Everything asks: the answer is what lets the call run.
            approvals: Arc::new(AskAll),
            context: Arc::new(
                TurnContext::new("scripted")
                    .system("you are aulo")
                    .tools(vec![echo_spec()]),
            ),
        },
        SessionConfig::default(),
        outbox,
    );
    let running = {
        let session = session.clone();
        tokio::spawn(async move {
            session
                .submit(Submission::UserTurn {
                    text: "echo hi".into(),
                    attachments: vec![],
                })
                .await
        })
    };

    let call = loop {
        let event = tokio::time::timeout(TURN_TIMEOUT, events.recv())
            .await
            .expect("an event arrives")
            .expect("the channel stays open");
        if let SessionEvent::Shared(aulo_types::AuloEvent::ApprovalRequired { call_id, .. }) = event
        {
            break call_id;
        }
    };
    session
        .submit(Submission::Approve {
            call_id: call,
            choice: ApprovalChoice::Once,
        })
        .await
        .expect("the answer is taken");
    tokio::time::timeout(TURN_TIMEOUT, running)
        .await
        .expect("the turn ends")
        .expect("the task joins")
        .expect("the turn returns");

    let mut seen = Vec::new();
    while let Ok(event) = events.try_recv() {
        seen.push(event);
    }
    assert!(
        seen.iter().any(
            |e| matches!(e, SessionEvent::ToolCallFinished { ok: true, output, .. } if output.as_deref() == Some("hi"))
        ),
        "{seen:?}"
    );
    assert!(matches!(
        seen.last(),
        Some(SessionEvent::TurnFinished {
            stop: TurnStop::Completed,
            ..
        })
    ));
}

/// A gate that asks for every call.
struct AskAll;

#[async_trait::async_trait]
impl Approvals for AskAll {
    async fn decide(&self, call: &ToolCall) -> Verdict {
        Verdict::Ask(agent_loop::types::Why::Risk { risk: call.risk })
    }
}
