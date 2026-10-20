//! The local client's view of `aulod`.
//!
//! The CLI is a surface crate and does not depend on tonic. It talks to the
//! daemon through the types in this module. The connection is the owner-only
//! socket (a Unix socket, or the per-user named pipe on Windows).

use std::collections::BTreeSet;
use std::error::Error as StdError;
use std::io;
use std::path::Path;

use aulo_proto::aulo::v1::chat_service_client::ChatServiceClient;
use aulo_proto::aulo::v1::event::Kind;
use aulo_proto::aulo::v1::session_service_client::SessionServiceClient;
use aulo_proto::aulo::v1::{
    ApprovalRequired, ConverseRequest, CreateChatRequest, DeleteChatRequest, Event, Interrupt,
    ListChatsRequest, ModelRef, Notice, TextDelta, ToolCallFinished, ToolCallOutput,
    ToolCallStarted, ToolCallStatus, TurnEndReason, TurnFinished, UserTurn, converse_request,
};
use hyper_util::rt::TokioIo;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::{Channel, Endpoint};
use tonic::{Code, Status, Streaming};
use tonic_health::pb::HealthCheckRequest;
use tonic_health::pb::health_check_response::ServingStatus;
use tonic_health::pb::health_client::HealthClient;
use tower::service_fn;

use crate::listener::local_socket_path;
use crate::text::{
    self, MAX_NOTICE_BYTES, MAX_TOOL_ARGS_BYTES, MAX_TOOL_NAME_BYTES, MAX_TURN_TEXT_BYTES,
};

const MAX_LISTED_CHATS: usize = 10_000;
const PAGE_SIZE: u32 = 200;
const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

/// Why a call to aulod failed. Messages are safe to show: they do not include
/// tokens, keys or store paths.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("aulod is not running")]
    NotRunning,
    #[error("aulod is not serving")]
    NotServing,
    #[error("{0}")]
    Rpc(String),
    #[error("the conversation ended before the turn finished")]
    Closed,
    #[error("cannot reach aulod")]
    Transport,
}

/// Whether the daemon's health service says it is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonStatus {
    Serving,
    NotServing,
}

/// One row of `aulo chats ls`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatLine {
    pub id: String,
    pub title: String,
    pub model: String,
}

/// A page of chats, and whether the cap stopped the walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatList {
    pub chats: Vec<ChatLine>,
    pub truncated: bool,
}

/// What the CLI renders from one `Converse` event.
///
/// Text has already been stripped of terminal controls and capped. The CLI
/// still treats it as untrusted and caps it again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatEvent {
    Text(String),
    ToolCall {
        name: String,
        arguments_json: String,
    },
    ToolOutput(String),
    /// Lowercase status (`succeeded`, `failed`, `denied`, `cancelled`).
    ToolFinished(String),
    Notice(String),
    Approval {
        tool: String,
        summary: String,
        needs_click: bool,
    },
    TurnFinished {
        outcome: TurnOutcome,
        error: Option<String>,
    },
}

/// Why a turn ended, as far as the CLI needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOutcome {
    Completed,
    Interrupted,
    Failed,
}

/// A connected local daemon.
#[derive(Debug, Clone)]
pub struct DaemonClient {
    channel: Channel,
}

/// One `Converse` stream. Dropping it closes the stream.
#[derive(Debug)]
pub struct Conversation {
    outbound: mpsc::Sender<ConverseRequest>,
    inbound: Streaming<aulo_proto::aulo::v1::ConverseResponse>,
}

impl DaemonClient {
    /// Connects to the daemon for `home` (`AULO_HOME`).
    pub async fn connect(home: &Path) -> Result<Self, ClientError> {
        #[cfg(unix)]
        {
            let path = local_socket_path(home);
            if !path.exists() {
                return Err(ClientError::NotRunning);
            }
            dial_unix(&path).await
        }
        #[cfg(windows)]
        {
            let _ = home;
            dial_pipe(&crate::listener::local_pipe_name()).await
        }
    }

    /// Connects to a Unix socket. Tests bind one under a temporary home.
    #[cfg(unix)]
    pub async fn connect_unix(path: &Path) -> Result<Self, ClientError> {
        dial_unix(path).await
    }

    /// Health of the whole server.
    pub async fn status(&self) -> Result<DaemonStatus, ClientError> {
        let response = HealthClient::new(self.channel.clone())
            .check(HealthCheckRequest {
                service: String::new(),
            })
            .await
            .map_err(explain)?
            .into_inner();
        Ok(match response.status() {
            ServingStatus::Serving => DaemonStatus::Serving,
            _ => DaemonStatus::NotServing,
        })
    }

    /// Creates a chat with an empty title and returns its id.
    pub async fn create_chat(&self) -> Result<String, ClientError> {
        let chat = chats(&self.channel)
            .create_chat(CreateChatRequest::default())
            .await
            .map_err(explain)?
            .into_inner()
            .chat
            .ok_or_else(|| ClientError::Rpc("the daemon returned no chat".to_owned()))?;
        Ok(chat.id)
    }

    /// Every chat, newest activity first, stopping after 10 000.
    pub async fn list_chats(&self) -> Result<ChatList, ClientError> {
        let mut client = chats(&self.channel);
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut token = String::new();
        let mut truncated = false;
        loop {
            let page = client
                .list_chats(ListChatsRequest {
                    page_size: PAGE_SIZE,
                    page_token: token,
                    bot_id: None,
                })
                .await
                .map_err(explain)?
                .into_inner();
            out.extend(page.chats.into_iter().map(chat_line));
            if out.len() >= MAX_LISTED_CHATS {
                out.truncate(MAX_LISTED_CHATS);
                truncated = true;
                break;
            }
            let Some(next) = next_token(&page.next_page_token, &mut seen) else {
                break;
            };
            token = next;
        }
        Ok(ChatList {
            chats: out,
            truncated,
        })
    }

    /// Deletes a chat. The id is whatever the person typed; the daemon checks it.
    pub async fn delete_chat(&self, id: &str) -> Result<(), ClientError> {
        chats(&self.channel)
            .delete_chat(DeleteChatRequest {
                chat_id: id.to_owned(),
            })
            .await
            .map_err(explain)?;
        Ok(())
    }

    /// Opens a conversation on `chat_id`. The first message is the Attach.
    pub async fn converse(&self, chat_id: &str) -> Result<Conversation, ClientError> {
        let (outbound, rx) = mpsc::channel(4);
        // Buffered before the RPC starts, so the server can read Attach while
        // this task is still waiting for the response headers.
        outbound
            .send(ConverseRequest {
                submission: Some(converse_request::Submission::Attach(
                    aulo_proto::aulo::v1::Attach {
                        chat_id: chat_id.to_owned(),
                    },
                )),
            })
            .await
            .map_err(|_| ClientError::Closed)?;
        let inbound = session(&self.channel)
            .converse(ReceiverStream::new(rx))
            .await
            .map_err(explain)?
            .into_inner();
        Ok(Conversation { outbound, inbound })
    }
}

impl Conversation {
    pub async fn user_turn(&self, text: &str) -> Result<(), ClientError> {
        self.outbound
            .send(ConverseRequest {
                submission: Some(converse_request::Submission::UserTurn(UserTurn {
                    text: text.to_owned(),
                })),
            })
            .await
            .map_err(|_| ClientError::Closed)
    }

    /// Asks the daemon to stop the turn in progress.
    pub async fn interrupt(&self) -> Result<(), ClientError> {
        self.outbound
            .send(ConverseRequest {
                submission: Some(converse_request::Submission::Interrupt(Interrupt {
                    all: false,
                })),
            })
            .await
            .map_err(|_| ClientError::Closed)
    }

    /// The next event the CLI should render. Voice events are skipped.
    pub async fn next_event(&mut self) -> Result<Option<ChatEvent>, ClientError> {
        loop {
            let Some(response) = self.inbound.message().await.map_err(explain)? else {
                return Ok(None);
            };
            if let Some(event) = response.event.and_then(chat_event) {
                return Ok(Some(event));
            }
        }
    }
}

fn chats(channel: &Channel) -> ChatServiceClient<Channel> {
    ChatServiceClient::new(channel.clone())
        .max_decoding_message_size(MAX_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_MESSAGE_BYTES)
}

fn session(channel: &Channel) -> SessionServiceClient<Channel> {
    SessionServiceClient::new(channel.clone())
        .max_decoding_message_size(MAX_MESSAGE_BYTES)
        .max_encoding_message_size(MAX_MESSAGE_BYTES)
}

/// `None` when the walk should stop: no further page, or a token repeated.
pub(crate) fn next_token(next: &str, seen: &mut BTreeSet<String>) -> Option<String> {
    if next.is_empty() || !seen.insert(next.to_owned()) {
        None
    } else {
        Some(next.to_owned())
    }
}

fn chat_line(chat: aulo_proto::aulo::v1::Chat) -> ChatLine {
    let (title, _) = text::plain_text(&chat.title, 200);
    ChatLine {
        id: chat.id,
        title: if title.is_empty() {
            "(untitled)".to_owned()
        } else {
            title
        },
        model: model_label(chat.model),
    }
}

fn model_label(model: Option<ModelRef>) -> String {
    let Some(model) = model else {
        return "-".to_owned();
    };
    let (provider, _) = text::plain_text(&model.provider, 64);
    let (name, _) = text::plain_text(&model.model, 128);
    match (provider.is_empty(), name.is_empty()) {
        (true, true) => "-".to_owned(),
        (true, false) => name,
        (false, true) => provider,
        (false, false) => format!("{provider}/{name}"),
    }
}

fn chat_event(event: Event) -> Option<ChatEvent> {
    let kind = event.kind?;
    Some(match kind {
        Kind::TextDelta(TextDelta { text, .. }) => {
            ChatEvent::Text(text::plain_text(&text, MAX_TURN_TEXT_BYTES).0)
        }
        Kind::ToolCallStarted(ToolCallStarted {
            tool,
            arguments_json,
            ..
        }) => ChatEvent::ToolCall {
            name: one_line(&tool, MAX_TOOL_NAME_BYTES),
            arguments_json: one_line(&arguments_json, MAX_TOOL_ARGS_BYTES),
        },
        Kind::ToolCallOutput(ToolCallOutput { text, .. }) => {
            ChatEvent::ToolOutput(text::plain_text(&text, MAX_TOOL_ARGS_BYTES).0)
        }
        Kind::ToolCallFinished(ToolCallFinished { status, .. }) => {
            ChatEvent::ToolFinished(status_name(status))
        }
        Kind::Notice(Notice { message, .. }) => {
            ChatEvent::Notice(one_line(&message, MAX_NOTICE_BYTES))
        }
        Kind::ApprovalRequired(ApprovalRequired {
            tool,
            summary,
            needs_click,
            ..
        }) => ChatEvent::Approval {
            tool: one_line(&tool, MAX_TOOL_NAME_BYTES),
            summary: one_line(&summary, MAX_NOTICE_BYTES),
            needs_click,
        },
        Kind::TurnFinished(TurnFinished { reason, error, .. }) => ChatEvent::TurnFinished {
            outcome: outcome(reason),
            error: error.map(|message| one_line(&message, MAX_NOTICE_BYTES)),
        },
        _ => return None,
    })
}

fn one_line(input: &str, max_bytes: usize) -> String {
    text::plain_text(input, max_bytes)
        .0
        .replace(['\n', '\t'], " ")
}

fn status_name(status: i32) -> String {
    match ToolCallStatus::try_from(status) {
        Ok(ToolCallStatus::Succeeded) => "succeeded",
        Ok(ToolCallStatus::Failed) => "failed",
        Ok(ToolCallStatus::Denied) => "denied",
        Ok(ToolCallStatus::Cancelled) => "cancelled",
        Ok(ToolCallStatus::Running) => "running",
        _ => "unspecified",
    }
    .to_owned()
}

fn outcome(reason: i32) -> TurnOutcome {
    match TurnEndReason::try_from(reason) {
        Ok(TurnEndReason::Completed) => TurnOutcome::Completed,
        Ok(TurnEndReason::Interrupted) => TurnOutcome::Interrupted,
        _ => TurnOutcome::Failed,
    }
}

fn explain(status: Status) -> ClientError {
    if status.code() == Code::Unavailable {
        return ClientError::NotRunning;
    }
    if status.code() == Code::Unimplemented {
        return ClientError::Rpc("this aulod has no conversation service".to_owned());
    }
    let message = status.message().trim();
    if message.is_empty() {
        ClientError::Rpc(status.code().to_string())
    } else {
        ClientError::Rpc(message.to_owned())
    }
}

#[cfg(unix)]
async fn dial_unix(path: &Path) -> Result<DaemonClient, ClientError> {
    let path = path.to_owned();
    let channel = Endpoint::from_static("http://[::]:50051")
        .connect_with_connector(service_fn(move |_| {
            let path = path.clone();
            async move {
                let stream = tokio::net::UnixStream::connect(&path).await?;
                Ok::<_, io::Error>(TokioIo::new(stream))
            }
        }))
        .await
        .map_err(transport)?;
    Ok(DaemonClient { channel })
}

#[cfg(windows)]
async fn dial_pipe(name: &str) -> Result<DaemonClient, ClientError> {
    let name = name.to_owned();
    let channel = Endpoint::from_static("http://[::]:50051")
        .connect_with_connector(service_fn(move |_| {
            let name = name.clone();
            async move {
                let client = tokio::net::windows::named_pipe::ClientOptions::new().open(&name)?;
                Ok::<_, io::Error>(TokioIo::new(client))
            }
        }))
        .await
        .map_err(transport)?;
    Ok(DaemonClient { channel })
}

fn transport(error: tonic::transport::Error) -> ClientError {
    if error_chain_is_down(&error) {
        ClientError::NotRunning
    } else {
        tracing::debug!(error = %error, "local daemon connection failed");
        ClientError::Transport
    }
}

fn error_chain_is_down(error: &(dyn StdError + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(io_error) = error.downcast_ref::<io::Error>() {
            return matches!(
                io_error.kind(),
                io::ErrorKind::NotFound
                    | io::ErrorKind::ConnectionRefused
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::BrokenPipe
            );
        }
        current = error.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_tokens_stop_on_empty_and_on_a_repeat() {
        let mut seen = BTreeSet::new();
        assert_eq!(next_token("", &mut seen), None);
        assert_eq!(next_token("a", &mut seen).as_deref(), Some("a"));
        assert_eq!(next_token("a", &mut seen), None);
        assert_eq!(next_token("b", &mut seen).as_deref(), Some("b"));
    }

    #[test]
    fn events_drop_controls_before_the_cli_sees_them() {
        let event = chat_event(Event {
            kind: Some(Kind::TextDelta(TextDelta {
                turn_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".into(),
                text: "a\u{1b}[31mred".into(),
            })),
        });
        assert_eq!(event, Some(ChatEvent::Text("a[31mred".into())));
    }
}
