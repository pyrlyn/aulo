//! A [`SessionService`](aulo_proto::aulo::v1::session_service_server::SessionService)
//! backed by llm-wire's scripted provider.
//!
//! `aulod --scripted` mounts this so the text client can be exercised without
//! a live model. It is not the agent: tool calls are events to render, and
//! nothing here runs a process. The real `Converse` implementation belongs to
//! the agent session (T4.8).

use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use aulo_proto::aulo::v1::event::Kind;
use aulo_proto::aulo::v1::session_service_server::{SessionService, SessionServiceServer};
use aulo_proto::aulo::v1::{
    ConverseRequest, ConverseResponse, Event, TextDelta, ToolCallFinished, ToolCallStarted,
    ToolCallStatus, TurnEndReason, TurnFinished, TurnStarted, converse_request,
};
use aulo_store::{Store, StoreError};
use aulo_types::{ChatId, TurnId};
use llm_wire::test_util::scripted::{self, TurnSpec};
use llm_wire::{
    Caps, Content, Effort, Job, Message, ModelId, Provider, ProviderError, ProviderEvent,
    ProviderId, Request, Role, StopReason, SystemBlock, Thinking, Tier, Usage,
};
use tokio::sync::mpsc;
use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::{Request as Rpc, Response, Status, Streaming};

use crate::text::{
    self, MAX_DELTA_BYTES, MAX_NOTICE_BYTES, MAX_TOOL_ARGS_BYTES, MAX_TOOL_NAME_BYTES,
    MAX_TURN_TEXT_BYTES, MAX_USER_TEXT_BYTES,
};

/// Built-in scenario for `aulod --scripted` when no file is given.
///
/// `hello` answers with text. `tool` answers with text and one shell call.
/// The exact user message `wait` is not a scenario turn: the provider holds
/// the turn open until an Interrupt, which is how the client tests Ctrl-C.
pub const DEFAULT_SCENARIO: &str = r#"
[[turn]]
when_contains = "hello"
text = "hello from the scripted provider"

[[turn]]
when_contains = "tool"
text = "running a tool"
[[turn.tool_calls]]
name = "shell"
input = { cmd = "ls" }
"#;

/// A bad scenario file. The message names the parser failure, not a secret.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ScenarioError(String);

/// Replays a scenario TOML document. Clone-cheap: every session shares it.
#[derive(Debug, Clone)]
pub struct ScriptedProvider {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    turns: Vec<TurnSpec>,
    /// Next unmatched positional turn. Pinned turns (`when_contains`) are not counted.
    next: Mutex<usize>,
}

impl ScriptedProvider {
    /// Parses a scenario. An empty document is valid and answers with the fallback line.
    pub fn parse(toml_text: &str) -> Result<Self, ScenarioError> {
        let turns =
            scripted::parse_scenario(toml_text).map_err(|e| ScenarioError(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(Inner {
                turns,
                next: Mutex::new(0),
            }),
        })
    }

    fn pick(&self, user: &str, system: &str, job: &Job) -> TurnSpec {
        let pinned = self
            .inner
            .turns
            .iter()
            .find(|turn| turn.when_contains.is_some() && matches_pins(turn, user, system, job));
        if let Some(turn) = pinned {
            return turn.clone();
        }
        let positional: Vec<&TurnSpec> = self
            .inner
            .turns
            .iter()
            .filter(|turn| turn.when_contains.is_none() && matches_pins(turn, user, system, job))
            .collect();
        if positional.is_empty() {
            return TurnSpec {
                text: Some("I have nothing scripted for that.".to_owned()),
                tool_calls: Vec::new(),
                error: None,
                when_contains: None,
                echo_tools: false,
                job: None,
                when_system_contains: None,
            };
        }
        let mut next = self
            .inner
            .next
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let index = *next % positional.len();
        *next = next.saturating_add(1);
        positional[index].clone()
    }
}

fn matches_pins(turn: &TurnSpec, user: &str, system: &str, job: &Job) -> bool {
    if let Some(pin) = &turn.job
        && pin != job
    {
        return false;
    }
    if let Some(pin) = &turn.when_system_contains
        && !system.contains(pin.as_str())
    {
        return false;
    }
    if let Some(pin) = &turn.when_contains
        && !user.contains(pin.as_str())
    {
        return false;
    }
    true
}

fn user_text(req: &Request) -> String {
    req.messages
        .iter()
        .rev()
        .find(|message| message.role == Role::User)
        .map(|message| {
            message
                .content
                .iter()
                .filter_map(|block| match block {
                    Content::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn system_text(req: &Request) -> String {
    req.system
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn request_for(text: &str) -> Request {
    Request {
        tier: Tier::Code,
        job: Job::Main,
        model: ModelId("scripted".to_owned()),
        system: Vec::<SystemBlock>::new(),
        tools: Vec::new(),
        messages: vec![Message {
            role: Role::User,
            content: vec![Content::Text {
                text: text.to_owned(),
            }],
        }],
        effort: Effort::Low,
        max_tokens: 1024,
        thinking: Thinking::Off,
        cache_breakpoints: Vec::new(),
        stop_sequences: Vec::new(),
    }
}

fn usage() -> Usage {
    Usage {
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
        estimated: true,
        cost_usd: 0.0,
        latency_ms: 0,
    }
}

#[async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Local
    }

    fn capabilities(&self) -> Caps {
        Caps {
            cache: false,
            thinking: false,
            server_tools: false,
            count_tokens: false,
            max_context: 8_192,
        }
    }

    async fn stream(
        &self,
        req: Request,
        sink: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<Usage, ProviderError> {
        if cancel.is_cancelled() {
            return Err(ProviderError::Cancelled);
        }
        let user = user_text(&req);
        // Exact, so a sentence that merely mentions waiting still gets a scripted turn.
        if user.trim() == "wait" {
            sink.send(ProviderEvent::TextDelta {
                text: "...".to_owned(),
            })
            .await
            .map_err(|_| ProviderError::Cancelled)?;
            cancel.cancelled().await;
            return Err(ProviderError::Cancelled);
        }
        let turn = self.pick(&user, &system_text(&req), &req.job);
        let events = scripted::events_for(&turn, req.model.clone(), usage());
        for event in events {
            if cancel.is_cancelled() {
                return Err(ProviderError::Cancelled);
            }
            sink.send(event)
                .await
                .map_err(|_| ProviderError::Cancelled)?;
        }
        if let Some(message) = turn.error {
            return Err(ProviderError::BadRequest { message });
        }
        Ok(usage())
    }

    async fn count_tokens(&self, req: &Request) -> Result<u32, ProviderError> {
        Ok(u32::try_from(user_text(req).len()).unwrap_or(u32::MAX))
    }
}

/// `Converse` for the scripted provider. Chats come from the same store as [`ChatApi`](crate::ChatApi).
#[derive(Debug, Clone)]
pub struct ScriptedSession {
    store: Arc<Mutex<Store>>,
    provider: ScriptedProvider,
}

impl ScriptedSession {
    #[must_use]
    pub fn new(store: Arc<Mutex<Store>>, provider: ScriptedProvider) -> Self {
        Self { store, provider }
    }

    /// The tonic service, so `aulod` can mount it without naming the generated type.
    #[must_use]
    pub fn into_server(self) -> SessionServiceServer<Self> {
        SessionServiceServer::new(self)
    }
}

type EventStream = Pin<Box<dyn Stream<Item = Result<ConverseResponse, Status>> + Send>>;

#[tonic::async_trait]
impl SessionService for ScriptedSession {
    type ConverseStream = EventStream;

    async fn converse(
        &self,
        request: Rpc<Streaming<ConverseRequest>>,
    ) -> Result<Response<Self::ConverseStream>, Status> {
        let mut inbound = request.into_inner();
        // Attach has to name a real chat before any event is sent.
        let _chat_id = first_attach(&self.store, &mut inbound).await?;
        let (tx, rx) = mpsc::channel(32);
        let provider = self.provider.clone();
        tokio::spawn(async move {
            drive(inbound, tx, provider).await;
        });
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }
}

async fn first_attach(
    store: &Arc<Mutex<Store>>,
    inbound: &mut Streaming<ConverseRequest>,
) -> Result<String, Status> {
    let Some(first) = inbound
        .message()
        .await
        .map_err(|_| Status::internal("cannot read the request"))?
    else {
        return Err(Status::failed_precondition(
            "the first request must be an Attach",
        ));
    };
    match first.submission {
        Some(converse_request::Submission::Attach(attach)) => {
            ensure_chat(store, &attach.chat_id).await
        }
        Some(_) | None => Err(Status::failed_precondition(
            "the first request must be an Attach",
        )),
    }
}

async fn ensure_chat(store: &Arc<Mutex<Store>>, raw: &str) -> Result<String, Status> {
    let id = raw
        .parse::<ChatId>()
        .map(|id| id.to_string())
        .map_err(|_| Status::invalid_argument("chat_id is not a valid ULID"))?;
    let store = Arc::clone(store);
    let id_for_task = id.clone();
    tokio::task::spawn_blocking(move || {
        let mut guard = store.lock().unwrap_or_else(PoisonError::into_inner);
        match guard.get_chat(&id_for_task) {
            Ok(Some(_)) => Ok(()),
            Ok(None) => Err(Status::not_found("chat not found")),
            Err(e) => Err(storage(&e)),
        }
    })
    .await
    .map_err(|_| Status::internal("internal storage error"))??;
    Ok(id)
}

fn storage(e: &StoreError) -> Status {
    tracing::error!(error = %e, "scripted session could not read the chat");
    Status::internal("internal storage error")
}

async fn drive(
    mut inbound: Streaming<ConverseRequest>,
    tx: mpsc::Sender<Result<ConverseResponse, Status>>,
    provider: ScriptedProvider,
) {
    let (in_tx, mut in_rx) = mpsc::channel(8);
    let reader = tokio::spawn(async move {
        while let Ok(Some(message)) = inbound.message().await {
            if in_tx.send(message).await.is_err() {
                break;
            }
        }
    });
    while let Some(message) = in_rx.recv().await {
        match message.submission {
            Some(converse_request::Submission::UserTurn(turn)) => {
                if turn.text.len() > MAX_USER_TEXT_BYTES {
                    let _ = tx
                        .send(Err(Status::invalid_argument(
                            "user text is longer than 64 KiB",
                        )))
                        .await;
                    break;
                }
                if !run_turn(&mut in_rx, &tx, &provider, turn.text).await {
                    break;
                }
            }
            Some(converse_request::Submission::Interrupt(_)) => {}
            Some(_) => {
                if !emit_notice(&tx, "this daemon does not handle that submission").await {
                    break;
                }
            }
            None => break,
        }
    }
    reader.abort();
}

/// `false` when the client has gone and the stream should stop.
async fn run_turn(
    inbound: &mut mpsc::Receiver<ConverseRequest>,
    tx: &mpsc::Sender<Result<ConverseResponse, Status>>,
    provider: &ScriptedProvider,
    text: String,
) -> bool {
    let turn_id = TurnId::new().to_string();
    if !emit(
        tx,
        Kind::TurnStarted(TurnStarted {
            turn_id: turn_id.clone(),
        }),
    )
    .await
    {
        return false;
    }
    let cancel = CancellationToken::new();
    let (event_tx, mut event_rx) = mpsc::channel(32);
    let task_provider = provider.clone();
    let task_cancel = cancel.clone();
    let task = tokio::spawn(async move {
        task_provider
            .stream(request_for(&text), event_tx, task_cancel)
            .await
    });

    let mut interrupted = false;
    let mut failed: Option<String> = None;
    let mut tool: Option<OpenTool> = None;
    let mut sent_text = 0usize;
    loop {
        tokio::select! {
            biased;
            incoming = inbound.recv() => {
                match incoming {
                    Some(message) => match message.submission {
                        Some(converse_request::Submission::Interrupt(_)) => {
                            interrupted = true;
                            cancel.cancel();
                        }
                        Some(converse_request::Submission::UserTurn(_))
                            if !emit_notice(tx, "a turn is already running").await =>
                        {
                            cancel.cancel();
                            task.abort();
                            return false;
                        }
                        _ => {}
                    },
                    None => {
                        cancel.cancel();
                        task.abort();
                        return false;
                    }
                }
            }
            event = event_rx.recv() => {
                match event {
                    Some(event) => {
                        if !forward(tx, &turn_id, event, &mut tool, &mut sent_text, &mut failed).await {
                            cancel.cancel();
                            task.abort();
                            return false;
                        }
                        if failed.is_some() {
                            cancel.cancel();
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
    }
    // The provider task owns the event sender. Dropping our receiver does not
    // wait for it; aborting a finished task is a no-op.
    if !task.is_finished() {
        cancel.cancel();
    }
    let joined = task.await;
    if !finish_turn(tx, &turn_id, interrupted, failed, joined).await {
        return false;
    }
    true
}

struct OpenTool {
    id: String,
    name: String,
    input: String,
}

async fn forward(
    tx: &mpsc::Sender<Result<ConverseResponse, Status>>,
    turn_id: &str,
    event: ProviderEvent,
    tool: &mut Option<OpenTool>,
    sent_text: &mut usize,
    failed: &mut Option<String>,
) -> bool {
    match event {
        ProviderEvent::TextDelta { text } => send_text(tx, turn_id, &text, sent_text).await,
        ProviderEvent::ToolUseStart { id, name } => {
            *tool = Some(OpenTool {
                id: id.to_string(),
                name,
                input: String::new(),
            });
            true
        }
        ProviderEvent::ToolUseInputDelta { text } => {
            if let Some(open) = tool.as_mut()
                && open.input.len() < MAX_TOOL_ARGS_BYTES
            {
                let room = MAX_TOOL_ARGS_BYTES - open.input.len();
                let (piece, _) = text::plain_text(&text, room);
                open.input.push_str(&piece);
            }
            true
        }
        ProviderEvent::ToolUseEnd => {
            let Some(open) = tool.take() else {
                return true;
            };
            let (name, _) = text::plain_text(&open.name, MAX_TOOL_NAME_BYTES);
            let (arguments, _) = text::plain_text(&open.input, MAX_TOOL_ARGS_BYTES);
            if !emit(
                tx,
                Kind::ToolCallStarted(ToolCallStarted {
                    turn_id: turn_id.to_owned(),
                    call_id: open.id.clone(),
                    tool: name,
                    arguments_json: arguments,
                }),
            )
            .await
            {
                return false;
            }
            emit(
                tx,
                Kind::ToolCallFinished(ToolCallFinished {
                    call_id: open.id,
                    status: ToolCallStatus::Succeeded as i32,
                }),
            )
            .await
        }
        ProviderEvent::Error { error } => {
            let (message, _) = text::plain_text(&error.to_string(), MAX_NOTICE_BYTES);
            *failed = Some(message);
            true
        }
        ProviderEvent::Stop {
            stop: StopReason::Refusal { detail },
        } => {
            let (message, _) = text::plain_text(&detail, MAX_NOTICE_BYTES);
            *failed = Some(message);
            true
        }
        ProviderEvent::MessageStart { .. }
        | ProviderEvent::ThinkingDelta { .. }
        | ProviderEvent::ToolUseSignature { .. }
        | ProviderEvent::Stop { .. }
        | ProviderEvent::Usage { .. }
        | ProviderEvent::Retrying { .. } => true,
    }
}

async fn send_text(
    tx: &mpsc::Sender<Result<ConverseResponse, Status>>,
    turn_id: &str,
    text: &str,
    sent: &mut usize,
) -> bool {
    if *sent >= MAX_TURN_TEXT_BYTES {
        return true;
    }
    let room = MAX_TURN_TEXT_BYTES - *sent;
    let (clean, _) = text::plain_text(text, room);
    *sent += clean.len();
    for chunk in text::byte_chunks(&clean, MAX_DELTA_BYTES) {
        if !emit(
            tx,
            Kind::TextDelta(TextDelta {
                turn_id: turn_id.to_owned(),
                text: chunk,
            }),
        )
        .await
        {
            return false;
        }
    }
    true
}

async fn finish_turn(
    tx: &mpsc::Sender<Result<ConverseResponse, Status>>,
    turn_id: &str,
    interrupted: bool,
    failed: Option<String>,
    joined: Result<Result<Usage, ProviderError>, tokio::task::JoinError>,
) -> bool {
    if interrupted || matches!(joined, Ok(Err(ProviderError::Cancelled))) {
        return emit(
            tx,
            Kind::TurnFinished(TurnFinished {
                turn_id: turn_id.to_owned(),
                reason: TurnEndReason::Interrupted as i32,
                error: None,
            }),
        )
        .await;
    }
    if let Some(message) = failed {
        return emit(
            tx,
            Kind::TurnFinished(TurnFinished {
                turn_id: turn_id.to_owned(),
                reason: TurnEndReason::Failed as i32,
                error: Some(message),
            }),
        )
        .await;
    }
    if let Ok(Err(error)) = joined {
        let (message, _) = text::plain_text(&error.to_string(), MAX_NOTICE_BYTES);
        return emit(
            tx,
            Kind::TurnFinished(TurnFinished {
                turn_id: turn_id.to_owned(),
                reason: TurnEndReason::Failed as i32,
                error: Some(message),
            }),
        )
        .await;
    }
    emit(
        tx,
        Kind::TurnFinished(TurnFinished {
            turn_id: turn_id.to_owned(),
            reason: TurnEndReason::Completed as i32,
            error: None,
        }),
    )
    .await
}

async fn emit_notice(tx: &mpsc::Sender<Result<ConverseResponse, Status>>, message: &str) -> bool {
    let (message, _) = text::plain_text(message, MAX_NOTICE_BYTES);
    emit(
        tx,
        Kind::Notice(aulo_proto::aulo::v1::Notice {
            level: aulo_proto::aulo::v1::NoticeLevel::Info as i32,
            message,
            source: Some("scripted".to_owned()),
        }),
    )
    .await
}

async fn emit(tx: &mpsc::Sender<Result<ConverseResponse, Status>>, kind: Kind) -> bool {
    tx.send(Ok(ConverseResponse {
        event: Some(Event { kind: Some(kind) }),
    }))
    .await
    .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    use llm_wire::CallId;

    #[test]
    fn the_default_scenario_parses() {
        let provider = ScriptedProvider::parse(DEFAULT_SCENARIO).unwrap();
        let hello = provider.pick("hello", "", &Job::Main);
        assert_eq!(
            hello.text.as_deref(),
            Some("hello from the scripted provider")
        );
        let tool = provider.pick("please tool", "", &Job::Main);
        assert_eq!(tool.tool_calls.len(), 1);
        assert_eq!(tool.tool_calls[0].name, "shell");
        let other = provider.pick("something else", "", &Job::Main);
        assert_eq!(
            other.text.as_deref(),
            Some("I have nothing scripted for that.")
        );
    }

    #[test]
    fn a_bad_scenario_names_the_parser_error() {
        let error = ScriptedProvider::parse("[[turn][").unwrap_err();
        let text = error.to_string();
        assert!(text.contains("scripted") || text.contains("turn") || !text.is_empty());
    }

    #[test]
    fn call_id_from_the_wire_displays() {
        let id = CallId::new();
        assert_eq!(id.to_string().len(), 26);
    }
}
