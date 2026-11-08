//! `aulo.v1.SessionService.Converse` over `aulo-agent`.
//!
//! One live [`Session`] per chat, built on the first Attach from the stored
//! history, so a chat continues after a daemon restart. Every attached
//! client gets every event of its chat. A client that falls further behind
//! than the event buffer is disconnected rather than buffered without bound;
//! it reattaches and reloads with `ChatService.ListMessages`.
//!
//! Persistence: one usage row per provider call, and each turn's new history
//! (messages, plus the tool calls with their full arguments and output) is
//! written before its `TurnFinished` reaches any client. A turn cut short by
//! the daemon stopping is not stored.
//!
//! Live chats stay in memory for the daemon's life.

mod events;
mod history;
mod route;

pub use route::Endpoints;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::Duration;

use agent_loop::traits::ToolCx;
use agent_loop::types::{LoopError, ToolCall, ToolResult};
use async_trait::async_trait;
use aulo_agent::{
    ApprovalChoice, Discovery, EVENT_CHANNEL, MAX_USER_TEXT_BYTES, RegistryTools, Session,
    SessionConfig, SessionError, SessionEvent, SessionParts, Submission, ToolExecutor,
    ToolRegistry,
};
use aulo_config::Config;
use aulo_policy::{Grants, Policy, PolicyGate, RuleSet, Settings};
use aulo_proto::aulo::v1::converse_request::Submission as Request;
use aulo_proto::aulo::v1::session_service_server::{SessionService, SessionServiceServer};
use aulo_proto::aulo::v1::{
    ApprovalDecision, ConverseRequest, ConverseResponse, DecideApproval, Event, SwitchModel,
};
use aulo_store::Store;
use aulo_types::{CallId, ChatId, NoticeLevel};
use llm_wire::Provider;
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Response, Status, Streaming};

use crate::chat::{input, on_store, storage};
use crate::error::ServerError;
use route::{ChatContext, ChatProvider, Endpoint, Route, Selection};

/// Events queued for one client before it counts as too slow.
const CLIENT_BUFFER: usize = 64;

type Client = mpsc::Sender<Result<ConverseResponse, Status>>;

/// What every chat session is built from.
pub struct SessionSetup {
    /// Provider endpoints by config name.
    pub endpoints: Endpoints,
    /// `provider/model` for a chat that stores none: `providers.tiers.main`.
    pub default_model: Option<String>,
    /// The compiled `[policy]` rules every tool call passes.
    pub policy: Policy,
    pub config: SessionConfig,
}

impl std::fmt::Debug for SessionSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionSetup")
            .field("endpoints", &self.endpoints.keys().collect::<Vec<_>>())
            .field("default_model", &self.default_model)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl SessionSetup {
    /// Endpoints, main-tier model, policy rules and approval timeout from
    /// `config`. An endpoint that does not build is skipped with a warning.
    ///
    /// # Errors
    ///
    /// [`ServerError::Policy`] for a rule that is not valid grammar: the
    /// daemon refuses to serve rather than run tools without its guard.
    pub fn from_config(config: &Config) -> Result<Self, ServerError> {
        let endpoints = aulo_providers::build_providers(&config.providers)
            .endpoints
            .into_iter()
            .map(|(name, endpoint)| (name, Arc::new(Endpoint(endpoint)) as Arc<dyn Provider>))
            .collect();
        let default_model = config
            .providers
            .tiers
            .main
            .as_ref()
            .map(|tier| format!("{}{}{}", tier.provider, input::MODEL_SEPARATOR, tier.model));
        let rules = RuleSet {
            allow: config.policy.allow.clone(),
            ask: config.policy.ask.clone(),
            deny: config.policy.deny.clone(),
        };
        let cwd =
            std::env::current_dir().map_err(ServerError::io("read", "the working directory"))?;
        let policy = Policy::new(
            &rules,
            Settings::default(),
            std::env::home_dir().as_deref(),
            &cwd,
        )
        .map_err(ServerError::Policy)?;
        Ok(Self {
            endpoints,
            default_model,
            policy,
            config: SessionConfig {
                approval_timeout: Some(Duration::from_secs(
                    config.policy.approval_timeout_secs.get(),
                )),
                ..SessionConfig::default()
            },
        })
    }
}

/// Serves `Converse`. Clone-cheap: clones share the live chats.
#[derive(Clone)]
pub struct SessionApi {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for SessionApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionApi")
            .field("live_chats", &self.inner.live().len())
            .finish_non_exhaustive()
    }
}

struct Inner {
    store: Arc<Mutex<Store>>,
    endpoints: Arc<Endpoints>,
    default_model: Option<Selection>,
    policy: Policy,
    config: SessionConfig,
    registry: Arc<ToolRegistry>,
    executor: Arc<dyn ToolExecutor>,
    chats: Mutex<HashMap<String, Arc<LiveChat>>>,
}

impl Inner {
    fn live(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<LiveChat>>> {
        self.chats.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

struct LiveChat {
    id: String,
    session: Session,
    route: Arc<Route>,
    events: broadcast::Sender<Event>,
}

/// No tool backend is wired yet; the registry holds only `tool_search`, so
/// a call that reaches here names a tool aulo does not have.
struct NoBackends;

#[async_trait]
impl ToolExecutor for NoBackends {
    async fn execute(&self, _call: &ToolCall, _cx: &ToolCx) -> ToolResult {
        ToolResult::failed("no tool backend is installed")
    }
}

/// Standing grants are written by the approval flow (T5.3), which also
/// decides how a stored grant maps to a `(tool, subject)` pair. Until then
/// only rules and this chat's session grants allow a call.
struct NoStandingGrants;

impl Grants for NoStandingGrants {
    fn active(&self, _tool: &str) -> Vec<(String, String)> {
        Vec::new()
    }
}

impl SessionApi {
    #[must_use]
    pub fn new(store: Arc<Mutex<Store>>, setup: SessionSetup) -> Self {
        Self {
            inner: Arc::new(Inner {
                store,
                endpoints: Arc::new(setup.endpoints),
                default_model: setup.default_model.as_deref().and_then(Selection::parse),
                policy: setup.policy,
                config: setup.config,
                registry: Arc::new(ToolRegistry::new()),
                executor: Arc::new(NoBackends),
                chats: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// The tonic service `ApiServer::add_service` mounts.
    #[must_use]
    pub fn service(self) -> SessionServiceServer<Self> {
        SessionServiceServer::new(self)
    }

    /// The chat's live session, built from the store on first use.
    async fn attach(&self, chat_id: &str) -> Result<Arc<LiveChat>, Status> {
        if let Some(live) = self.inner.live().get(chat_id) {
            return Ok(Arc::clone(live));
        }
        let id = chat_id.to_owned();
        let (chat, rows) = on_store(&self.inner.store, "converse_attach", move |s| {
            let chat = s
                .get_chat(&id)
                .map_err(|e| storage("converse_attach", &e))?
                .ok_or_else(|| Status::not_found("chat not found"))?;
            let rows = s
                .chat_history(&id)
                .map_err(|e| storage("converse_attach", &e))?;
            Ok((chat, rows))
        })
        .await?;
        let key: ChatId = chat_id
            .parse()
            .map_err(|_| Status::invalid_argument("chat_id is not a valid ULID"))?;
        let history = history::from_rows(rows);
        let persisted = history.len();
        let route = Arc::new(Route::new(
            Selection::parse(&chat.model).or_else(|| self.inner.default_model.clone()),
        ));
        let tools = Arc::new(RegistryTools::new(
            Arc::clone(&self.inner.registry),
            Arc::new(Discovery::new()),
            Arc::clone(&self.inner.executor),
        ));
        let parts = SessionParts {
            provider: Arc::new(ChatProvider {
                endpoints: Arc::clone(&self.inner.endpoints),
                route: Arc::clone(&route),
            }),
            tools: Arc::clone(&tools) as _,
            // One gate per chat, so "allow for this chat" ends with it.
            approvals: Arc::new(PolicyGate::new(
                self.inner.policy.clone(),
                Arc::new(NoStandingGrants),
            )),
            context: Arc::new(ChatContext {
                route: Arc::clone(&route),
                tools,
            }),
        };
        let (outbox, rx) = mpsc::channel(EVENT_CHANNEL);
        let session = Session::with_history(key, parts, self.inner.config.clone(), history, outbox);
        let (events, _) = broadcast::channel(EVENT_CHANNEL);
        let live = Arc::new(LiveChat {
            id: chat_id.to_owned(),
            session,
            route,
            events,
        });
        {
            let mut chats = self.inner.live();
            // Two first attaches raced: keep the winner; the loser's session
            // has run nothing and ends with its last reference.
            if let Some(existing) = chats.get(chat_id) {
                return Ok(Arc::clone(existing));
            }
            chats.insert(chat_id.to_owned(), Arc::clone(&live));
        }
        tokio::spawn(pump(
            Arc::clone(&self.inner.store),
            Arc::downgrade(&live),
            live.events.clone(),
            rx,
            persisted,
        ));
        Ok(live)
    }

    async fn serve_client(
        &self,
        incoming: &mut Streaming<ConverseRequest>,
        tx: &Client,
    ) -> Result<(), Status> {
        let first = incoming.message().await?.ok_or_else(attach_first)?;
        let Some(Request::Attach(attach)) = first.submission else {
            return Err(attach_first());
        };
        let chat_id = input::chat_id(&attach.chat_id)?;
        let live = self.attach(&chat_id).await?;
        let forwarder = tokio::spawn(forward(live.events.subscribe(), tx.clone()));
        let result = loop {
            tokio::select! {
                message = incoming.message() => match message {
                    Ok(Some(request)) => {
                        if let Err(status) = self.handle(&live, request, tx).await {
                            break Err(status);
                        }
                    }
                    Ok(None) => break Ok(()),
                    Err(status) => {
                        tracing::debug!(%status, "converse client went away");
                        break Ok(());
                    }
                },
                () = tx.closed() => break Ok(()),
            }
        };
        forwarder.abort();
        result
    }

    async fn handle(
        &self,
        live: &Arc<LiveChat>,
        request: ConverseRequest,
        tx: &Client,
    ) -> Result<(), Status> {
        match request.submission {
            Some(Request::Attach(_)) => {
                return Err(Status::failed_precondition(
                    "this stream is already attached",
                ));
            }
            Some(Request::UserTurn(turn)) => self.user_turn(live, turn.text, tx).await,
            Some(Request::DecideApproval(decision)) => decide(live, decision, tx).await,
            Some(Request::Interrupt(interrupt)) => {
                // The kill switch: every chat, at once. Narrowing it to the
                // `control` scope is T13.3.
                if interrupt.all {
                    for chat in self.inner.live().values() {
                        chat.session.interrupt();
                    }
                } else {
                    live.session.interrupt();
                }
            }
            Some(Request::SwitchModel(switch)) => self.switch_model(live, switch, tx).await?,
            Some(Request::TakeoverDone(_)) => {
                tell(tx, NoticeLevel::Info, "takeover is not available yet").await;
            }
            None => {
                return Err(Status::invalid_argument(
                    "the request carries no submission",
                ));
            }
        }
        Ok(())
    }

    async fn user_turn(&self, live: &Arc<LiveChat>, text: String, tx: &Client) {
        // Rejected, not truncated: a session never runs a prompt the user
        // did not write.
        if text.is_empty() || text.len() > MAX_USER_TEXT_BYTES {
            tell(
                tx,
                NoticeLevel::Warn,
                "a turn takes 1 byte to 64 KiB of text; it did not run",
            )
            .await;
            return;
        }
        let ready = live
            .route
            .next()
            .is_some_and(|s| self.inner.endpoints.contains_key(&s.endpoint));
        if !ready {
            tell(
                tx,
                NoticeLevel::Error,
                "this chat has no usable model; send SwitchModel with a configured provider",
            )
            .await;
            return;
        }
        let session = live.session.clone();
        let inner = Arc::clone(&self.inner);
        let chat = Arc::downgrade(live);
        // A turn runs apart from the request loop, so Interrupt and approvals
        // from this client are read while it runs.
        tokio::spawn(async move {
            let turn = Submission::UserTurn {
                text,
                attachments: Vec::new(),
            };
            match session.submit(turn).await {
                Ok(()) => {}
                Err(SessionError::Loop(LoopError::Sink { message })) => {
                    // The session cannot report anymore; the next Attach
                    // rebuilds it from the store.
                    tracing::error!(%message, "chat session failed; retiring it");
                    retire(&inner, &chat);
                }
                Err(error) => tracing::debug!(%error, "turn ended early"),
            }
        });
    }

    async fn switch_model(
        &self,
        live: &LiveChat,
        switch: SwitchModel,
        tx: &Client,
    ) -> Result<(), Status> {
        let stored = match switch.model.as_ref().map(input::model) {
            Some(Ok(stored)) => stored,
            Some(Err(status)) => {
                tell(tx, NoticeLevel::Warn, status.message()).await;
                return Ok(());
            }
            None => {
                tell(tx, NoticeLevel::Warn, "model is required").await;
                return Ok(());
            }
        };
        let Some(selection) =
            Selection::parse(&stored).filter(|s| self.inner.endpoints.contains_key(&s.endpoint))
        else {
            tell(
                tx,
                NoticeLevel::Warn,
                "model.provider names no configured endpoint",
            )
            .await;
            return Ok(());
        };
        let id = live.id.clone();
        let found = on_store(&self.inner.store, "switch_model", move |s| {
            s.set_chat_model(&id, &stored)
                .map_err(|e| storage("switch_model", &e))
        })
        .await?;
        if !found {
            return Err(Status::not_found("chat not found"));
        }
        live.route.set_next(selection);
        Ok(())
    }
}

#[tonic::async_trait]
impl SessionService for SessionApi {
    type ConverseStream = ReceiverStream<Result<ConverseResponse, Status>>;

    async fn converse(
        &self,
        request: tonic::Request<Streaming<ConverseRequest>>,
    ) -> Result<Response<Self::ConverseStream>, Status> {
        let mut incoming = request.into_inner();
        let (tx, rx) = mpsc::channel(CLIENT_BUFFER);
        let api = self.clone();
        tokio::spawn(async move {
            if let Err(status) = api.serve_client(&mut incoming, &tx).await {
                let _ = tx.send(Err(status)).await;
            }
        });
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

fn attach_first() -> Status {
    Status::failed_precondition("the first Converse request must be Attach")
}

async fn decide(live: &LiveChat, decision: DecideApproval, tx: &Client) {
    let Ok(call_id) = decision.call_id.parse::<CallId>() else {
        tell(tx, NoticeLevel::Warn, "call_id is not a valid ULID").await;
        return;
    };
    let choice = match ApprovalDecision::try_from(decision.decision) {
        Ok(ApprovalDecision::AllowOnce) => ApprovalChoice::Once,
        Ok(ApprovalDecision::AllowChat) => ApprovalChoice::ForChat,
        Ok(ApprovalDecision::AllowAlways) => ApprovalChoice::Always,
        Ok(ApprovalDecision::DenyOnce | ApprovalDecision::DenyAlways) => ApprovalChoice::Never,
        Ok(ApprovalDecision::Unspecified) | Err(_) => {
            tell(tx, NoticeLevel::Warn, "decision is not a known approval").await;
            return;
        }
    };
    if let Err(error) = live
        .session
        .submit(Submission::Approve { call_id, choice })
        .await
    {
        let text = match error {
            SessionError::NoPendingApproval => "no approval is pending for this call",
            _ => "the approval was not delivered",
        };
        tell(tx, NoticeLevel::Warn, text).await;
    }
}

/// A notice for one client only: its own request was not acted on.
async fn tell(tx: &Client, level: NoticeLevel, message: &str) {
    let event = Event {
        kind: Some(events::notice(level, message.to_owned(), None)),
    };
    let _ = tx.send(Ok(ConverseResponse { event: Some(event) })).await;
}

fn retire(inner: &Inner, chat: &Weak<LiveChat>) {
    let Some(chat) = chat.upgrade() else { return };
    let mut chats = inner.live();
    if chats
        .get(&chat.id)
        .is_some_and(|live| Arc::ptr_eq(live, &chat))
    {
        chats.remove(&chat.id);
    }
}

/// Copies the chat's events to one client until either side goes away.
async fn forward(mut events: broadcast::Receiver<Event>, tx: Client) {
    loop {
        match events.recv().await {
            Ok(event) => {
                if tx
                    .send(Ok(ConverseResponse { event: Some(event) }))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                let _ = tx
                    .send(Err(Status::resource_exhausted(
                        "this client fell behind the chat's events; reattach and reload the history",
                    )))
                    .await;
                return;
            }
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}

/// Drains one session's events: stores usage and finished turns, then fans
/// each event out to the attached clients. Holds the chat weakly, so it ends
/// once the chat's session is gone.
async fn pump(
    store: Arc<Mutex<Store>>,
    chat: Weak<LiveChat>,
    events: broadcast::Sender<Event>,
    mut rx: mpsc::Receiver<SessionEvent>,
    mut persisted: usize,
) {
    while let Some(event) = rx.recv().await {
        match &event {
            SessionEvent::Usage { usage, .. } => {
                if let Some(live) = chat.upgrade() {
                    record_usage(&store, &live, usage).await;
                }
            }
            SessionEvent::TurnFinished { .. } => {
                if let Some(live) = chat.upgrade() {
                    persisted = persist(&store, &live, persisted).await;
                }
            }
            _ => {}
        }
        if let Some(event) = events::event(event) {
            // No receiver only means no client is attached right now.
            let _ = events.send(event);
        }
    }
}

async fn record_usage(store: &Arc<Mutex<Store>>, live: &LiveChat, usage: &llm_wire::Usage) {
    let Some(selection) = live.route.current() else {
        return;
    };
    let id = live.id.clone();
    let (input, output) = (
        i64::from(usage.input_tokens),
        i64::from(usage.output_tokens),
    );
    let _ = on_store(store, "record_usage", move |s| {
        s.record_usage(&id, &selection.endpoint, &selection.model, input, output)
            .map(drop)
            .map_err(|e| storage("record_usage", &e))
    })
    .await;
}

/// Writes the history the store does not have yet; returns how much of the
/// session's history is stored. A failed write is retried with the next turn.
async fn persist(store: &Arc<Mutex<Store>>, live: &LiveChat, persisted: usize) -> usize {
    let messages = live.session.history().await;
    let end = history::complete_len(&messages);
    let Some(new) = messages.get(persisted..end).filter(|new| !new.is_empty()) else {
        return persisted;
    };
    let rows = history::to_rows(new);
    let id = live.id.clone();
    match on_store(store, "append_turn", move |s| {
        s.append_turn(&id, &rows)
            .map_err(|e| storage("append_turn", &e))
    })
    .await
    {
        Ok(()) => end,
        Err(_) => persisted,
    }
}
