//! The harness T4.6 and T4.8 call: a temp-home daemon, a scripted provider,
//! and a Converse client. The session here is a stand-in that streams the
//! scripted reply; the real session replaces it without changing the client.
// TestDaemon serves on an owner-only Unix socket; other platforms only have a placeholder.
#![cfg(unix)]
#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use aulo_proto::aulo::v1::chat_service_client::ChatServiceClient;
use aulo_proto::aulo::v1::config_service_client::ConfigServiceClient;
use aulo_proto::aulo::v1::converse_request::Submission;
use aulo_proto::aulo::v1::event::Kind;
use aulo_proto::aulo::v1::session_service_server::{SessionService, SessionServiceServer};
use aulo_proto::aulo::v1::{
    ConverseRequest, ConverseResponse, CreateChatRequest, Event, ListProvidersRequest, TextDelta,
    TurnEndReason, TurnFinished, TurnStarted,
};
use aulo_testkit::{Scripted, TestDaemon, attach, drive, user_turn};
use llm_wire::{
    Content, Effort, Job, Message, ModelId, Provider, ProviderEvent, Request, Role, Thinking, Tier,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::{Request as Rpc, Response, Status, Streaming};

const TURN_ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

#[derive(Clone)]
struct ScriptedSession {
    provider: Arc<Scripted>,
}

#[tonic::async_trait]
impl SessionService for ScriptedSession {
    type ConverseStream = ReceiverStream<Result<ConverseResponse, Status>>;

    async fn converse(
        &self,
        request: Rpc<Streaming<ConverseRequest>>,
    ) -> Result<Response<Self::ConverseStream>, Status> {
        let mut incoming = request.into_inner();
        let (tx, rx) = mpsc::channel(8);
        let provider = Arc::clone(&self.provider);
        tokio::spawn(async move {
            if let Err(status) = answer(&provider, &mut incoming, &tx).await {
                let _ = tx.send(Err(status)).await;
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

async fn answer(
    provider: &Scripted,
    incoming: &mut Streaming<ConverseRequest>,
    tx: &mpsc::Sender<Result<ConverseResponse, Status>>,
) -> Result<(), Status> {
    let first = incoming
        .message()
        .await?
        .ok_or_else(|| Status::failed_precondition("the first Converse request must be Attach"))?;
    if !matches!(first.submission, Some(Submission::Attach(_))) {
        return Err(Status::failed_precondition(
            "the first Converse request must be Attach",
        ));
    }
    let next = incoming
        .message()
        .await?
        .ok_or_else(|| Status::invalid_argument("expected a user turn"))?;
    let Some(Submission::UserTurn(turn)) = next.submission else {
        return Err(Status::invalid_argument("expected a user turn"));
    };
    send(
        tx,
        Event {
            kind: Some(Kind::TurnStarted(TurnStarted {
                turn_id: TURN_ID.into(),
            })),
        },
    )
    .await;
    let (events, mut inbound) = mpsc::channel(8);
    let streaming = provider.stream(
        Request {
            tier: Tier::Code,
            job: Job::Main,
            model: ModelId("scripted".into()),
            system: vec![],
            tools: vec![],
            messages: vec![Message {
                role: Role::User,
                content: vec![Content::Text { text: turn.text }],
            }],
            effort: Effort::High,
            max_tokens: 64,
            thinking: Thinking::Off,
            cache_breakpoints: vec![],
            stop_sequences: vec![],
        },
        events,
        CancellationToken::new(),
    );
    tokio::pin!(streaming);
    loop {
        tokio::select! {
            biased;
            event = inbound.recv() => {
                match event {
                    Some(ProviderEvent::TextDelta { text }) => {
                        send(tx, Event {
                            kind: Some(Kind::TextDelta(TextDelta {
                                turn_id: TURN_ID.into(),
                                text,
                            })),
                        }).await;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            result = &mut streaming => {
                result.map_err(|_| Status::internal("scripted provider failed"))?;
                break;
            }
        }
    }
    while let Some(event) = inbound.recv().await {
        if let ProviderEvent::TextDelta { text } = event {
            send(
                tx,
                Event {
                    kind: Some(Kind::TextDelta(TextDelta {
                        turn_id: TURN_ID.into(),
                        text,
                    })),
                },
            )
            .await;
        }
    }
    send(
        tx,
        Event {
            kind: Some(Kind::TurnFinished(TurnFinished {
                turn_id: TURN_ID.into(),
                reason: i32::from(TurnEndReason::Completed),
                error: None,
            })),
        },
    )
    .await;
    Ok(())
}

async fn send(tx: &mpsc::Sender<Result<ConverseResponse, Status>>, event: Event) {
    let _ = tx.send(Ok(ConverseResponse { event: Some(event) })).await;
}

fn finished(event: &Event) -> bool {
    matches!(event.kind, Some(Kind::TurnFinished(_)))
}

#[tokio::test]
async fn the_temp_home_daemon_serves_config_and_chat() {
    let daemon = TestDaemon::start().await.expect("daemon");
    let channel = daemon.channel().await.expect("channel");
    let providers = ConfigServiceClient::new(channel.clone())
        .list_providers(ListProvidersRequest {})
        .await
        .expect("providers")
        .into_inner()
        .providers;
    let scripted = providers
        .iter()
        .find(|provider| provider.id == "scripted")
        .expect("scripted endpoint");
    assert_eq!(scripted.kind, "ollama");
    assert!(!scripted.key_set);

    let chat = ChatServiceClient::new(channel)
        .create_chat(CreateChatRequest {
            title: Some("scripted".into()),
            ..CreateChatRequest::default()
        })
        .await
        .expect("chat")
        .into_inner()
        .chat
        .expect("chat row");
    assert_eq!(chat.id.len(), 26);
    assert_eq!(chat.title, "scripted");
    assert!(daemon.store_path().is_file());
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn driving_converse_uses_the_scripted_reply() {
    let provider = Arc::new(
        Scripted::from_toml("[[turn]]\ntext = \"hello from the script\"\n").expect("scenario"),
    );
    let session = ScriptedSession {
        provider: Arc::clone(&provider),
    };
    let daemon = TestDaemon::builder()
        .start_with(|server| server.add_service(SessionServiceServer::new(session)))
        .await
        .expect("daemon");
    let chat = ChatServiceClient::new(daemon.channel().await.expect("channel"))
        .create_chat(CreateChatRequest::default())
        .await
        .expect("chat")
        .into_inner()
        .chat
        .expect("chat row");

    let events = drive(
        daemon.channel().await.expect("channel"),
        vec![attach(&chat.id), user_turn("say hello")],
        finished,
    )
    .await
    .expect("drive");
    let text: String = events
        .iter()
        .filter_map(|event| match &event.kind {
            Some(Kind::TextDelta(delta)) => Some(delta.text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "hello from the script");
    assert!(finished(events.last().expect("event")));
    let requests = provider.requests();
    assert_eq!(requests.len(), 1);
    assert!(matches!(
        &requests[0].messages[0].content[0],
        Content::Text { text } if text == "say hello"
    ));
    daemon.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_turn_before_attach_fails_the_stream() {
    let daemon = TestDaemon::builder()
        .start_with(|server| {
            server.add_service(SessionServiceServer::new(ScriptedSession {
                provider: Arc::new(Scripted::from_turns(vec![])),
            }))
        })
        .await
        .expect("daemon");
    let mut session = aulo_testkit::ConverseSession::open(daemon.channel().await.expect("channel"))
        .await
        .expect("open");
    session.send(user_turn("too soon")).await.expect("send");
    let error = session.next_event().await.expect_err("rejected");
    let message = error.to_string();
    assert!(
        message.contains("Attach"),
        "status should name the missing attach, got {message}"
    );
    daemon.shutdown().await.expect("shutdown");
}
