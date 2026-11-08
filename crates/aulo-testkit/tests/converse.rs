//! The real `SessionService` on a temp-home daemon: a chat's turns, tool
//! calls and usage are stored, and the chat continues with its full history
//! after a restart.
#![cfg(unix)] // TestDaemon serves over a Unix socket
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use aulo_proto::aulo::v1::chat_service_client::ChatServiceClient;
use aulo_proto::aulo::v1::event::Kind;
use aulo_proto::aulo::v1::{
    CreateChatRequest, Event, ListMessagesRequest, MessageRole, ModelRef, NoticeLevel,
    ToolCallStatus, TurnEndReason,
};
use aulo_store::Store;
use aulo_testkit::{ConverseSession, Scripted, TestDaemon, attach, drive, switch_model, user_turn};
use llm_wire::{Content, Role};
use tonic::transport::Channel;

const SCENARIO: &str = r#"
[[turn]]
text = "let me look"
[[turn.tool_calls]]
name = "shell"
input = { command = "ls" }

[[turn]]
text = "nothing I can run"

[[turn]]
text = "still here"
"#;

fn finished(event: &Event) -> bool {
    matches!(event.kind, Some(Kind::TurnFinished(_)))
}

fn end_reason(events: &[Event]) -> Option<i32> {
    events.iter().find_map(|event| match &event.kind {
        Some(Kind::TurnFinished(finished)) => Some(finished.reason),
        _ => None,
    })
}

async fn create_chat(channel: Channel, model: Option<ModelRef>) -> String {
    ChatServiceClient::new(channel)
        .create_chat(CreateChatRequest {
            model,
            ..CreateChatRequest::default()
        })
        .await
        .expect("create chat")
        .into_inner()
        .chat
        .expect("chat row")
        .id
}

fn scripted_model() -> Option<ModelRef> {
    Some(ModelRef {
        provider: "scripted".into(),
        model: "test".into(),
    })
}

#[tokio::test]
async fn a_chat_continues_with_its_full_history_after_a_restart() {
    let provider = Arc::new(Scripted::from_toml(SCENARIO).expect("scenario"));
    let daemon = TestDaemon::builder()
        .endpoint("scripted", provider.clone())
        .start()
        .await
        .expect("daemon");
    let chat = create_chat(daemon.channel().await.unwrap(), scripted_model()).await;

    let events = drive(
        daemon.channel().await.unwrap(),
        vec![attach(&chat), user_turn("list my files")],
        finished,
    )
    .await
    .expect("first turn");
    assert_eq!(end_reason(&events), Some(TurnEndReason::Completed.into()));
    assert!(
        events
            .iter()
            .any(|e| matches!(&e.kind, Some(Kind::ToolCallStarted(c)) if c.tool == "shell"))
    );

    let daemon = daemon.restart().await.expect("restart");
    let events = drive(
        daemon.channel().await.unwrap(),
        vec![attach(&chat), user_turn("are you there?")],
        finished,
    )
    .await
    .expect("turn after restart");
    assert_eq!(end_reason(&events), Some(TurnEndReason::Completed.into()));

    // The provider saw the whole conversation, tool use and result included.
    let requests = provider.requests();
    assert_eq!(requests.len(), 3);
    let sent = &requests[2].messages;
    let roles: Vec<Role> = sent.iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        [
            Role::User,
            Role::Assistant,
            Role::User,
            Role::Assistant,
            Role::User
        ]
    );
    assert!(matches!(&sent[0].content[0], Content::Text { text } if text == "list my files"));
    let Content::ToolUse { id, name, .. } = &sent[1].content[1] else {
        panic!("the assistant's tool use survived: {:?}", sent[1]);
    };
    assert_eq!(name, "shell");
    assert!(matches!(
        &sent[2].content[0],
        Content::ToolResult { call_id, is_error: true, .. } if call_id == id
    ));
    assert!(matches!(&sent[3].content[0], Content::Text { text } if text == "nothing I can run"));
    assert!(matches!(&sent[4].content[0], Content::Text { text } if text == "are you there?"));
    assert_eq!(requests[2].model.0, "test");

    // ListMessages serves the stored turns with the call attached.
    let messages = ChatServiceClient::new(daemon.channel().await.unwrap())
        .list_messages(ListMessagesRequest {
            chat_id: chat.clone(),
            ..ListMessagesRequest::default()
        })
        .await
        .expect("list messages")
        .into_inner()
        .messages;
    let roles: Vec<i32> = messages.iter().map(|m| m.role).collect();
    let [user, assistant, tool] =
        [MessageRole::User, MessageRole::Assistant, MessageRole::Tool].map(i32::from);
    assert_eq!(roles, [user, assistant, tool, assistant, user, assistant]);
    assert_eq!(messages[1].content, "let me look");
    assert_eq!(messages[5].content, "still here");
    let call = &messages[1].tool_calls[0];
    assert_eq!(call.name, "shell");
    assert_eq!(call.arguments_json, r#"{"command":"ls"}"#);
    assert_eq!(call.status, i32::from(ToolCallStatus::Failed));

    // One usage row per provider call, attributed to the chat's model.
    let mut store = Store::open(daemon.store_path().to_str().unwrap()).expect("store");
    let usage = store.list_usage(&chat).expect("usage");
    assert_eq!(usage.len(), 3);
    assert!(
        usage
            .iter()
            .all(|row| row.provider == "scripted" && row.model == "test")
    );
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_chat_without_a_model_runs_once_one_is_switched_in() {
    let provider = Arc::new(Scripted::from_toml("[[turn]]\ntext = \"hi\"\n").expect("scenario"));
    let daemon = TestDaemon::builder()
        .endpoint("scripted", provider.clone())
        .start()
        .await
        .expect("daemon");
    let chat = create_chat(daemon.channel().await.unwrap(), None).await;

    let mut session = ConverseSession::open(daemon.channel().await.unwrap())
        .await
        .unwrap();
    session.send(attach(&chat)).await.unwrap();
    session.send(user_turn("hello")).await.unwrap();
    let event = session.next_event().await.unwrap().expect("notice");
    assert!(matches!(
        &event.kind,
        Some(Kind::Notice(n)) if n.level == i32::from(NoticeLevel::Error)
    ));

    session.send(switch_model("nowhere", "x")).await.unwrap();
    let event = session.next_event().await.unwrap().expect("notice");
    assert!(matches!(&event.kind, Some(Kind::Notice(n)) if n.message.contains("endpoint")));

    session
        .send(switch_model("scripted", "other"))
        .await
        .unwrap();
    session.send(user_turn("hello")).await.unwrap();
    let events = session.until(finished).await.unwrap();
    assert_eq!(end_reason(&events), Some(TurnEndReason::Completed.into()));
    assert_eq!(provider.requests()[0].model.0, "other");

    let mut store = Store::open(daemon.store_path().to_str().unwrap()).expect("store");
    assert_eq!(
        store.get_chat(&chat).unwrap().expect("chat").model,
        "scripted/other"
    );
    drop(session);
    daemon.shutdown().await.expect("shutdown");
}
