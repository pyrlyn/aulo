//! The OpenAI Chat and Responses wires (`llm-wire-openai`, T1.3) as aulo
//! builds them from config: recorded SSE replies stream text and a tool call.
//!
//! The crate has its own fixture tests. These pin aulo's side: the endpoint
//! kind picks the wire, the base URL picks the route, `prepare_turn`'s model
//! is what goes out, and a keyless client sends no `Authorization` header.
//!
//! Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(clippy::unwrap_used)]

use std::path::Path;

use aulo_config::{ProviderConfig, ProviderKind, ProvidersConfig};
use aulo_providers::{build_providers, prepare_turn, stored_model};
use llm_wire::{ProviderEvent, StopReason, Usage};
use serde_json::json;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

/// What one streamed reply said, with the deltas joined.
#[derive(Debug, Default, PartialEq)]
struct Reply {
    text: String,
    /// `(tool name, joined JSON input)` per call, in order.
    calls: Vec<(String, String)>,
    stop: Option<StopReason>,
}

fn fold(events: Vec<ProviderEvent>) -> Reply {
    let mut reply = Reply::default();
    for event in events {
        match event {
            ProviderEvent::TextDelta { text } => reply.text.push_str(&text),
            ProviderEvent::ToolUseStart { name, .. } => reply.calls.push((name, String::new())),
            ProviderEvent::ToolUseInputDelta { text } => {
                reply.calls.last_mut().unwrap().1.push_str(&text);
            }
            ProviderEvent::Stop { stop } => reply.stop = Some(stop),
            _ => {}
        }
    }
    reply
}

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sse")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

/// Builds the `ep` endpoint of `kind` against a local server that replays
/// `fixture_name` once on `route`, and streams one turn for `model` through it.
async fn replay(
    kind: ProviderKind,
    route: &str,
    model: &str,
    fixture_name: &str,
) -> (Reply, Usage) {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1{route}")))
        .and(body_partial_json(json!({ "model": model })))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(fixture(fixture_name), "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut config = ProvidersConfig::default();
    config.endpoints.insert(
        "ep".to_owned(),
        ProviderConfig {
            kind,
            base_url: Some(format!("{}/v1", server.uri())),
        },
    );
    let set = build_providers(&config);
    assert!(set.warnings.is_empty(), "{:?}", set.warnings);
    let provider = set.endpoints["ep"].as_provider();

    let turn = prepare_turn(&["ep"], &stored_model("ep", model).unwrap(), "hello").unwrap();
    // Every fixture fits, so the stream never waits on the receiver.
    let (tx, mut rx) = mpsc::channel(64);
    let usage = provider
        .stream(turn.request, tx, CancellationToken::new())
        .await
        .unwrap();
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    let sent: Vec<Request> = server.received_requests().await.unwrap();
    assert!(
        sent.iter()
            .all(|r| !r.headers.contains_key("authorization")),
        "a keyless client must not send a bearer header"
    );
    (fold(events), usage)
}

#[tokio::test]
async fn chat_wire_streams_text() {
    let (reply, usage) = replay(
        ProviderKind::Ollama,
        "/chat/completions",
        "qwen3-coder",
        "chat_text.sse",
    )
    .await;
    assert_eq!(
        reply,
        Reply {
            text: "The daemon is aulod.".to_owned(),
            calls: Vec::new(),
            stop: Some(StopReason::EndTurn),
        }
    );
    assert_eq!((usage.input_tokens, usage.output_tokens), (21, 6));
}

#[tokio::test]
async fn chat_wire_streams_a_tool_call() {
    let (reply, usage) = replay(
        ProviderKind::OpenaiCompatible,
        "/chat/completions",
        "qwen3-coder",
        "chat_tool_call.sse",
    )
    .await;
    assert_eq!(
        reply,
        Reply {
            text: "Opening it.".to_owned(),
            calls: vec![("open_app".to_owned(), r#"{"app":"Notes"}"#.to_owned())],
            stop: Some(StopReason::EndTurn),
        }
    );
    assert_eq!((usage.input_tokens, usage.output_tokens), (40, 17));
}

#[tokio::test]
async fn responses_wire_streams_text() {
    let (reply, usage) = replay(
        ProviderKind::Openai,
        "/responses",
        "gpt-5.5",
        "responses_text.sse",
    )
    .await;
    assert_eq!(
        reply,
        Reply {
            text: "The daemon is aulod.".to_owned(),
            calls: Vec::new(),
            stop: Some(StopReason::EndTurn),
        }
    );
    assert_eq!((usage.input_tokens, usage.output_tokens), (21, 6));
}

#[tokio::test]
async fn responses_wire_streams_a_tool_call() {
    let (reply, usage) = replay(
        ProviderKind::Openai,
        "/responses",
        "gpt-5.5",
        "responses_tool_call.sse",
    )
    .await;
    assert_eq!(
        reply,
        Reply {
            text: String::new(),
            calls: vec![("open_app".to_owned(), r#"{"app":"Notes"}"#.to_owned())],
            stop: Some(StopReason::EndTurn),
        }
    );
    assert_eq!((usage.input_tokens, usage.output_tokens), (40, 17));
}
