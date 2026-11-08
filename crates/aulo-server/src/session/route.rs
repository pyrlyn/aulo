//! Which endpoint and model a chat's provider calls go to.
//!
//! A session's provider and context are fixed when it is built, but
//! `SwitchModel` changes the model of a live chat. Both therefore read one
//! shared [`Route`]: a switch lands in `next`, and the context copies it to
//! `current` at the first step of a turn, so a turn never changes model
//! halfway.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use agent_loop::TurnId;
use agent_loop::traits::{Context, Next};
use async_trait::async_trait;
use aulo_agent::{ToolSpecSource, TurnContext};
use aulo_providers::EndpointProvider;
use llm_wire::{Caps, Message, Provider, ProviderError, ProviderEvent, ProviderId, Request, Usage};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::chat::input::MODEL_SEPARATOR;

/// Configured endpoints by their `providers.endpoints` name.
pub type Endpoints = BTreeMap<String, Arc<dyn Provider>>;

/// A chat's model, split from the stored `provider/model` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Selection {
    pub endpoint: String,
    pub model: String,
}

impl Selection {
    /// `None` for an empty or malformed string: a chat created without a
    /// model stores `""`.
    pub fn parse(stored: &str) -> Option<Self> {
        let (endpoint, model) = stored.split_once(MODEL_SEPARATOR)?;
        if endpoint.is_empty() || model.is_empty() {
            return None;
        }
        Some(Self {
            endpoint: endpoint.to_owned(),
            model: model.to_owned(),
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct Route {
    next: Mutex<Option<Selection>>,
    current: Mutex<Option<Selection>>,
}

impl Route {
    pub fn new(selection: Option<Selection>) -> Self {
        Self {
            next: Mutex::new(selection.clone()),
            current: Mutex::new(selection),
        }
    }

    /// What the next turn will use.
    pub fn next(&self) -> Option<Selection> {
        lock(&self.next).clone()
    }

    pub fn set_next(&self, selection: Selection) {
        *lock(&self.next) = Some(selection);
    }

    /// What the running (or last) turn uses.
    pub fn current(&self) -> Option<Selection> {
        lock(&self.current).clone()
    }

    fn begin_turn(&self) {
        let next = self.next();
        *lock(&self.current) = next;
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Builds each request with the route's current model.
pub(crate) struct ChatContext {
    pub route: Arc<Route>,
    pub tools: Arc<dyn ToolSpecSource>,
}

#[async_trait]
impl Context for ChatContext {
    async fn request(&self, turn: TurnId, step: u32, history: &mut Vec<Message>) -> Next {
        if step == 1 {
            self.route.begin_turn();
        }
        let model = self.route.current().map(|s| s.model).unwrap_or_default();
        TurnContext::new(model)
            .tool_source(Arc::clone(&self.tools))
            .request(turn, step, history)
            .await
    }
}

/// Sends each call to the endpoint the route currently names.
pub(crate) struct ChatProvider {
    pub endpoints: Arc<Endpoints>,
    pub route: Arc<Route>,
}

impl ChatProvider {
    fn endpoint(&self) -> Option<&Arc<dyn Provider>> {
        self.endpoints.get(&self.route.current()?.endpoint)
    }
}

/// Reported only when the route names no endpoint; `stream` then refuses
/// the call, so nothing relies on these values.
const NO_CAPS: Caps = Caps {
    cache: false,
    thinking: false,
    server_tools: false,
    count_tokens: false,
    max_context: 0,
};

#[async_trait]
impl Provider for ChatProvider {
    fn id(&self) -> ProviderId {
        self.endpoint().map_or(ProviderId::Local, |p| p.id())
    }

    fn capabilities(&self) -> Caps {
        self.endpoint().map_or(NO_CAPS, |p| p.capabilities())
    }

    fn accepts_images(&self, model: &str) -> bool {
        self.endpoint().is_some_and(|p| p.accepts_images(model))
    }

    async fn stream(
        &self,
        req: Request,
        sink: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<Usage, ProviderError> {
        let Some(endpoint) = self.endpoint() else {
            return Err(no_endpoint());
        };
        endpoint.stream(req, sink, cancel).await
    }

    async fn count_tokens(&self, req: &Request) -> Result<u32, ProviderError> {
        match self.endpoint() {
            Some(endpoint) => endpoint.count_tokens(req).await,
            None => Err(no_endpoint()),
        }
    }
}

fn no_endpoint() -> ProviderError {
    ProviderError::BadRequest {
        message: "the chat's model names no configured provider endpoint".into(),
    }
}

/// An endpoint built from config, owned so it can sit behind `Arc<dyn Provider>`.
pub(crate) struct Endpoint(pub EndpointProvider);

#[async_trait]
impl Provider for Endpoint {
    fn id(&self) -> ProviderId {
        self.0.as_provider().id()
    }

    fn capabilities(&self) -> Caps {
        self.0.as_provider().capabilities()
    }

    fn accepts_images(&self, model: &str) -> bool {
        self.0.as_provider().accepts_images(model)
    }

    async fn stream(
        &self,
        req: Request,
        sink: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<Usage, ProviderError> {
        self.0.as_provider().stream(req, sink, cancel).await
    }

    async fn count_tokens(&self, req: &Request) -> Result<u32, ProviderError> {
        self.0.as_provider().count_tokens(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_model_splits_at_the_first_slash() {
        assert_eq!(
            Selection::parse("ollama/library/qwen3"),
            Some(Selection {
                endpoint: "ollama".into(),
                model: "library/qwen3".into(),
            })
        );
        assert_eq!(Selection::parse(""), None);
        assert_eq!(Selection::parse("ollama/"), None);
        assert_eq!(Selection::parse("/qwen3"), None);
    }

    #[test]
    fn a_switch_waits_for_the_next_turn() {
        let route = Route::new(Selection::parse("a/one"));
        route.set_next(Selection::parse("b/two").expect("valid"));
        assert_eq!(route.current(), Selection::parse("a/one"));
        route.begin_turn();
        assert_eq!(route.current(), Selection::parse("b/two"));
    }
}
