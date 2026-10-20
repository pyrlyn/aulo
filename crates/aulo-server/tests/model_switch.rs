//! A chat stores its model. Switching it is what the next provider request sends.
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use aulo_providers::{Turn, prepare_turn, stored_model};
use aulo_store::Store;
use llm_wire::{Caps, Provider, ProviderError, ProviderEvent, ProviderId, Request, Usage};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct Recording {
    seen: Mutex<Vec<String>>,
}

impl Recording {
    fn seen(&self) -> Vec<String> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    async fn accept(&self, turn: Turn) {
        let cancel = CancellationToken::new();
        let (tx, _rx) = mpsc::channel(1);
        self.stream(turn.request, tx, cancel).await.unwrap();
    }
}

#[async_trait]
impl Provider for Recording {
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
        _sink: mpsc::Sender<ProviderEvent>,
        _cancel: CancellationToken,
    ) -> Result<Usage, ProviderError> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(req.model.0);
        Ok(Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            estimated: false,
            cost_usd: 0.0,
            latency_ms: 0,
        })
    }

    async fn count_tokens(&self, _req: &Request) -> Result<u32, ProviderError> {
        Ok(0)
    }
}

#[tokio::test]
async fn switching_model_between_turns_changes_the_next_request() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("aulo.db");
    let mut store = Store::open(path.to_str().unwrap()).unwrap();
    let initial = stored_model("openai", "gpt-5.5").unwrap();
    let chat = store.create_chat(None, "switch", &initial).unwrap();

    let openai = Arc::new(Recording {
        seen: Mutex::new(Vec::new()),
    });
    let anthropic = Arc::new(Recording {
        seen: Mutex::new(Vec::new()),
    });
    let endpoints = ["openai", "anthropic"];

    let stored = store.get_chat(&chat.id).unwrap().unwrap().model;
    let first = prepare_turn(&endpoints, &stored, "hello").unwrap();
    assert_eq!(first.endpoint, "openai");
    openai.accept(first).await;

    let switched = stored_model("anthropic", "claude-sonnet-5").unwrap();
    assert!(store.set_chat_model(&chat.id, &switched).unwrap());
    let stored = store.get_chat(&chat.id).unwrap().unwrap().model;
    assert_eq!(stored, switched);
    let second = prepare_turn(&endpoints, &stored, "and now").unwrap();
    assert_eq!(second.endpoint, "anthropic");
    anthropic.accept(second).await;

    assert_eq!(openai.seen(), vec!["gpt-5.5".to_owned()]);
    assert_eq!(anthropic.seen(), vec!["claude-sonnet-5".to_owned()]);
}
