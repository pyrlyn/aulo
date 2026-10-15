//! Two tiers go to two providers, and a missing tier fails closed.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use llm_router::{Job, RouteError, Router, Tier};
use llm_wire::{Caps, Provider, ProviderError, ProviderEvent, ProviderId, Request, Usage};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct Named {
    id: ProviderId,
    calls: AtomicUsize,
}

impl Named {
    fn new(id: ProviderId) -> Arc<Self> {
        Arc::new(Self {
            id,
            calls: AtomicUsize::new(0),
        })
    }
}

#[async_trait]
impl Provider for Named {
    fn id(&self) -> ProviderId {
        self.id
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
        _req: Request,
        _sink: mpsc::Sender<ProviderEvent>,
        _cancel: CancellationToken,
    ) -> Result<Usage, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
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

fn sample_request() -> Request {
    Request {
        tier: llm_wire::Tier::Cheap,
        job: llm_wire::Job::Title,
        model: llm_wire::ModelId("test".to_owned()),
        system: Vec::new(),
        tools: Vec::new(),
        messages: Vec::new(),
        effort: llm_wire::Effort::Low,
        max_tokens: 16,
        thinking: llm_wire::Thinking::Off,
        cache_breakpoints: Vec::new(),
        stop_sequences: Vec::new(),
    }
}

#[tokio::test]
async fn two_tiers_route_to_two_providers() {
    let voice = Named::new(ProviderId::Local);
    let main = Named::new(ProviderId::OpenAi);
    let router = Router::builder()
        .provider(Tier::Voice, voice.clone())
        .provider(Tier::Main, main.clone())
        .build();

    let voice_provider = router.route(Job::Voice).expect("voice tier has a provider");
    let main_provider = router.route(Job::Main).expect("main tier has a provider");
    assert_eq!(voice_provider.id(), ProviderId::Local);
    assert_eq!(main_provider.id(), ProviderId::OpenAi);

    let cancel = CancellationToken::new();
    let (voice_tx, _voice_rx) = mpsc::channel(1);
    let (main_tx, _main_rx) = mpsc::channel(1);
    voice_provider
        .stream(sample_request(), voice_tx, cancel.clone())
        .await
        .expect("voice stream");
    main_provider
        .stream(sample_request(), main_tx, cancel)
        .await
        .expect("main stream");

    assert_eq!(voice.calls.load(Ordering::SeqCst), 1);
    assert_eq!(main.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn a_missing_tier_fails_closed() {
    let router = Router::builder()
        .provider(Tier::Voice, Named::new(ProviderId::Local))
        .build();
    assert!(matches!(
        router.route(Job::Main),
        Err(RouteError::NoProvider { tier: Tier::Main })
    ));
}

#[test]
fn title_defaults_to_cheap_and_an_override_wins() {
    let router = Router::builder().build();
    assert_eq!(router.tier_of(Job::Title), Tier::Cheap);
    assert_eq!(router.tier_of(Job::Summary), Tier::Cheap);
    assert_eq!(router.tier_of(Job::Sentinel), Tier::Sentinel);

    let cheap = Named::new(ProviderId::Local);
    let main = Named::new(ProviderId::Anthropic);
    let overridden = Router::builder()
        .provider(Tier::Cheap, cheap)
        .provider(Tier::Main, main.clone())
        .job(Job::Title, Tier::Main)
        .build();
    assert_eq!(overridden.tier_of(Job::Title), Tier::Main);
    assert_eq!(
        overridden.route(Job::Title).expect("title was moved").id(),
        ProviderId::Anthropic
    );
    assert_eq!(
        overridden
            .route(Job::Summary)
            .expect("summary stays cheap")
            .id(),
        ProviderId::Local
    );
}
