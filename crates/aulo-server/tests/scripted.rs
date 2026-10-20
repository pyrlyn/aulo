//! Scripted Converse over a real Unix socket: text, a tool call, interrupt,
//! and the attach rules.
#![cfg(unix)]
#![allow(clippy::unwrap_used)]

mod common;

use std::sync::{Arc, Mutex};

use aulo_proto::aulo::v1::chat_service_server::ChatServiceServer;
use aulo_server::{
    ChatApi, ChatEvent, DEFAULT_SCENARIO, DaemonClient, Limits, ScriptedProvider, ScriptedSession,
    TurnOutcome,
};
use aulo_store::Store;
use common::start_with;

async fn fixture(scenario: &str) -> (common::Running, DaemonClient, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aulo.db");
    let store = Arc::new(Mutex::new(Store::open(path.to_str().unwrap()).unwrap()));
    let provider = ScriptedProvider::parse(scenario).unwrap();
    let api = ChatApi::new(Arc::clone(&store));
    let session = ScriptedSession::new(store, provider);
    let server = start_with(Limits::default(), |s| {
        s.add_service(ChatServiceServer::new(api))
            .add_service(session.into_server())
    })
    .await;
    let client = DaemonClient::connect_unix(&server.socket).await.unwrap();
    (server, client, dir)
}

async fn say(client: &DaemonClient, chat: &str, text: &str) -> Vec<ChatEvent> {
    let mut conversation = client.converse(chat).await.unwrap();
    conversation.user_turn(text).await.unwrap();
    let mut events = Vec::new();
    while let Some(event) = conversation.next_event().await.unwrap() {
        let done = matches!(event, ChatEvent::TurnFinished { .. });
        events.push(event);
        if done {
            break;
        }
    }
    events
}

fn text_of(events: &[ChatEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            ChatEvent::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_scripted_turn_streams_text_and_a_tool_call() {
    let (_server, client, _dir) = fixture(DEFAULT_SCENARIO).await;
    let chat = client.create_chat().await.unwrap();
    let events = say(&client, &chat, "hello").await;
    assert_eq!(text_of(&events), "hello from the scripted provider");
    assert!(matches!(
        events.last(),
        Some(ChatEvent::TurnFinished {
            outcome: TurnOutcome::Completed,
            ..
        })
    ));

    let events = say(&client, &chat, "use the tool").await;
    assert_eq!(text_of(&events), "running a tool");
    assert!(events.iter().any(|event| matches!(
        event,
        ChatEvent::ToolCall {
            name,
            arguments_json,
        } if name == "shell" && arguments_json.contains("ls")
    )));
}

#[tokio::test]
async fn interrupt_ends_a_held_turn() {
    let (_server, client, _dir) = fixture(DEFAULT_SCENARIO).await;
    let chat = client.create_chat().await.unwrap();
    let mut conversation = client.converse(&chat).await.unwrap();
    conversation.user_turn("wait").await.unwrap();
    let first = conversation.next_event().await.unwrap();
    assert_eq!(first, Some(ChatEvent::Text("...".into())));
    conversation.interrupt().await.unwrap();
    let finished = conversation.next_event().await.unwrap();
    assert_eq!(
        finished,
        Some(ChatEvent::TurnFinished {
            outcome: TurnOutcome::Interrupted,
            error: None,
        })
    );
}

#[tokio::test]
async fn attach_rules_reject_a_missing_chat_and_a_late_attach() {
    let (_server, client, _dir) = fixture(DEFAULT_SCENARIO).await;
    let missing = client
        .converse("01ARZ3NDEKTSV4RRFFQ69G5FAV")
        .await
        .unwrap_err();
    assert_eq!(missing.to_string(), "chat not found");

    let bad = client.converse("not-a-ulid").await.unwrap_err();
    assert_eq!(bad.to_string(), "chat_id is not a valid ULID");
}

#[tokio::test]
async fn long_text_is_split_and_capped() {
    let scenario = format!(
        "[[turn]]\nwhen_contains = \"long\"\ntext = \"{}\"\n",
        "a".repeat(100_000)
    );
    let (_server, client, _dir) = fixture(&scenario).await;
    let chat = client.create_chat().await.unwrap();
    let events = say(&client, &chat, "long").await;
    let deltas: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            ChatEvent::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(deltas.len() > 1, "expected more than one delta");
    assert!(deltas.iter().all(|delta| delta.len() <= 16 * 1024));
    let joined = deltas.concat();
    assert_eq!(joined.len(), 64 * 1024);
    assert!(joined.chars().all(|ch| ch == 'a'));
}

#[tokio::test]
async fn model_text_loses_its_escape_before_it_is_an_event() {
    let scenario = "[[turn]]\nwhen_contains = \"esc\"\ntext = \"a\\u001B[31mred\"\n";
    let (_server, client, _dir) = fixture(scenario).await;
    let chat = client.create_chat().await.unwrap();
    let events = say(&client, &chat, "esc").await;
    let text = text_of(&events);
    assert_eq!(text, "a[31mred");
    assert!(!text.contains('\u{1b}'));
}
