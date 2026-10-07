use std::sync::Arc;
use std::time::Duration;

use aulo_speech::{
    AudioFormat, Capabilities, EngineInfo, EngineKind, EngineSpec, LanguageSupport, SpeechError,
    TtsEngine, TtsPoll, TtsRequest, Voice,
};
use reqwest::header::HeaderValue;
use reqwest::{Client, Url};
use tokio::runtime::Handle;

use crate::config::CloudTtsConfig;
use crate::net::{self, Endpoint};
use crate::reply::{Reply, Slot, TextLimits};
use crate::speech::{self, Job};

/// What `response_format: "pcm"` produces.
pub const SAMPLE_RATE_HZ: u32 = 24_000;
const ENDPOINT_PATH: &str = "/audio/speech";
#[derive(Debug)]
pub(crate) struct Shared {
    pub(crate) client: Client,
    pub(crate) endpoint: Url,
    pub(crate) bearer: Option<HeaderValue>,
    pub(crate) timeout: Duration,
    pub(crate) max_audio_bytes: usize,
    name: String,
    default_model: String,
    default_voice: String,
    voices: Vec<Voice>,
    max_text_chars: usize,
    capabilities: Capabilities,
    handle: Handle,
}

/// Builds [`CloudTts`] engines for the registry from one validated config.
#[derive(Debug, Clone)]
pub struct CloudTtsFactory {
    shared: Arc<Shared>,
}

impl CloudTtsFactory {
    /// `runtime` runs the requests; engines never block on it.
    pub fn new(config: CloudTtsConfig, runtime: Handle) -> Result<Self, SpeechError> {
        let Endpoint { url, local } =
            net::endpoint(&config.base_url, ENDPOINT_PATH, config.api_key.is_some())?;
        config.validate()?;
        Ok(Self {
            shared: Arc::new(Shared {
                client: net::client(None)?,
                endpoint: url,
                bearer: net::bearer(config.api_key.as_ref())?,
                timeout: config.request_timeout,
                max_audio_bytes: config.max_audio_bytes,
                name: config.name,
                default_model: config.default_model,
                default_voice: config.default_voice,
                voices: config.voices,
                max_text_chars: config.max_text_chars,
                capabilities: Capabilities {
                    // Each pushed text is its own request, so the first sentence plays while the rest is generated.
                    streaming: true,
                    languages: LanguageSupport::Any,
                    // A server on this machine (runa) works with the network down.
                    offline: local,
                    needs_network: true,
                },
                handle: runtime,
            }),
        })
    }

    /// An unknown `spec.voice` is `Unsupported`, so the registry can fall back.
    pub fn build(&self, spec: &EngineSpec) -> Result<Box<dyn TtsEngine>, SpeechError> {
        let shared = &self.shared;
        let voice = spec
            .voice
            .clone()
            .unwrap_or_else(|| shared.default_voice.clone());
        check_voice(shared, &voice)?;
        Ok(Box::new(CloudTts {
            info: EngineInfo {
                id: spec.engine.clone(),
                kind: EngineKind::Tts,
                name: shared.name.clone(),
                capabilities: shared.capabilities.clone(),
            },
            model: spec
                .model
                .clone()
                .unwrap_or_else(|| shared.default_model.clone()),
            voice,
            shared: Arc::clone(shared),
            reply: Slot::default(),
        }))
    }

    /// The closure form `EngineRegistry::register` takes.
    pub fn into_factory(
        self,
    ) -> impl Fn(&EngineSpec) -> Result<Box<dyn TtsEngine>, SpeechError> + Send + Sync + 'static
    {
        move |spec| self.build(spec)
    }
}

fn check_voice(shared: &Shared, voice: &str) -> Result<(), SpeechError> {
    if shared.voices.iter().any(|v| v.id == voice) {
        Ok(())
    } else {
        Err(SpeechError::unsupported(voice))
    }
}

/// OpenAI-compatible speech engine.
#[derive(Debug)]
pub struct CloudTts {
    info: EngineInfo,
    shared: Arc<Shared>,
    model: String,
    voice: String,
    reply: Slot,
}

impl TtsEngine for CloudTts {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn voices(&self) -> &[Voice] {
        &self.shared.voices
    }

    fn begin(&mut self, request: &TtsRequest<'_>) -> Result<AudioFormat, SpeechError> {
        self.cancel();
        let voice = request.voice.unwrap_or(&self.voice);
        check_voice(&self.shared, voice)?;
        let job = Job {
            model: self.model.clone(),
            voice: voice.to_owned(),
            speed: f64::from(request.rate.get()),
        };
        let format = AudioFormat::new(SAMPLE_RATE_HZ, 1)?;
        let limits = TextLimits {
            each: self.shared.max_text_chars,
            total: usize::MAX,
        };
        let shared = Arc::clone(&self.shared);
        self.reply
            .set(Reply::start(&self.shared.handle, limits, |texts, audio| {
                speech::run(shared, job, texts, audio)
            }));
        Ok(format)
    }

    fn push_text(&mut self, text: &str) -> Result<(), SpeechError> {
        self.reply.push_text(text)
    }

    fn finish(&mut self) -> Result<(), SpeechError> {
        self.reply.finish()
    }

    fn poll(&mut self, out: &mut [f32]) -> Result<TtsPoll, SpeechError> {
        self.reply.poll(out)
    }

    fn cancel(&mut self) {
        self.reply.cancel();
    }
}
