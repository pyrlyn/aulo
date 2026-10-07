use std::sync::Arc;
use std::time::Duration;

use aulo_speech::{
    AudioFormat, Capabilities, EngineInfo, EngineKind, EngineSpec, LanguageSupport, SpeechError,
    TtsEngine, TtsPoll, TtsRequest, Voice,
};
use reqwest::header::HeaderValue;
use reqwest::{Client, Url};
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, error::TryRecvError};
use tokio_util::task::AbortOnDropHandle;

use crate::config::{CloudTtsConfig, MAX_AUDIO_BYTES_LIMIT, MAX_TEXT_CHARS_LIMIT};
use crate::net::{self, Endpoint};
use crate::speech::{self, Job, Msg};

/// What `response_format: "pcm"` produces.
pub const SAMPLE_RATE_HZ: u32 = 24_000;
const ENDPOINT_PATH: &str = "/audio/speech";
/// Sentences waiting for the request in flight. A full queue is `Overflow`
/// rather than a block, because `push_text` must not wait.
const TEXT_QUEUE: usize = 64;
/// Chunks the network side may run ahead of playback: with the chunk size
/// this bounds audio memory to under a second, and stops reading the body
/// when `poll` falls behind.
const AUDIO_QUEUE: usize = 8;
const PCM_SCALE: f32 = 32768.0;

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
        if config.max_text_chars == 0 || config.max_text_chars > MAX_TEXT_CHARS_LIMIT {
            return Err(SpeechError::invalid("max text", "1 to 65536 characters"));
        }
        if config.max_audio_bytes == 0 || config.max_audio_bytes > MAX_AUDIO_BYTES_LIMIT {
            return Err(SpeechError::invalid("max audio", "1 byte to 64 MiB"));
        }
        if config.request_timeout.is_zero() {
            return Err(SpeechError::invalid("request timeout", "must not be zero"));
        }
        if !config.voices.iter().any(|v| v.id == config.default_voice) {
            return Err(SpeechError::invalid(
                "default voice",
                "not in the voice list",
            ));
        }
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
            state: State::Idle,
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

#[derive(Debug)]
enum State {
    Idle,
    Active(Reply),
}

/// One reply. Dropping it aborts the request in flight, which is how `cancel`
/// and `begin` stay instant.
#[derive(Debug)]
struct Reply {
    /// `None` once `finish` closed the text side.
    texts: Option<mpsc::Sender<String>>,
    audio: mpsc::Receiver<Msg>,
    _task: AbortOnDropHandle<()>,
    chunk: Vec<u8>,
    offset: usize,
    /// How the reply ended, held back until the audio before it is delivered.
    verdict: Option<Result<(), SpeechError>>,
}

/// OpenAI-compatible speech engine.
#[derive(Debug)]
pub struct CloudTts {
    info: EngineInfo,
    shared: Arc<Shared>,
    model: String,
    voice: String,
    state: State,
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
        let (texts, text_queue) = mpsc::channel(TEXT_QUEUE);
        let (audio_sender, audio) = mpsc::channel(AUDIO_QUEUE);
        let task = self.shared.handle.spawn(speech::run(
            Arc::clone(&self.shared),
            job,
            text_queue,
            audio_sender,
        ));
        self.state = State::Active(Reply {
            texts: Some(texts),
            audio,
            _task: AbortOnDropHandle::new(task),
            chunk: Vec::new(),
            offset: 0,
            verdict: None,
        });
        Ok(format)
    }

    fn push_text(&mut self, text: &str) -> Result<(), SpeechError> {
        let State::Active(Reply {
            texts: Some(texts), ..
        }) = &self.state
        else {
            return Err(SpeechError::OutOfOrder("text outside a reply"));
        };
        let text = text.trim();
        if text.is_empty() {
            // Nothing to say; a request for it would only bill and fail.
            return Ok(());
        }
        // `nth` stops at the limit, so a huge string is not walked to its end.
        if text.chars().nth(self.shared.max_text_chars).is_some() {
            return Err(SpeechError::invalid(
                "text",
                "longer than the engine allows",
            ));
        }
        match texts.try_send(text.to_owned()) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(text)) => Err(SpeechError::Overflow {
                dropped: text.chars().count(),
            }),
            // The request task already failed; `poll` reports why.
            Err(mpsc::error::TrySendError::Closed(_)) => Ok(()),
        }
    }

    fn finish(&mut self) -> Result<(), SpeechError> {
        let State::Active(reply) = &mut self.state else {
            return Err(SpeechError::OutOfOrder("finish before begin"));
        };
        reply.texts = None;
        Ok(())
    }

    fn poll(&mut self, out: &mut [f32]) -> Result<TtsPoll, SpeechError> {
        let State::Active(reply) = &mut self.state else {
            return Ok(TtsPoll::Done);
        };
        let mut written = 0;
        while written < out.len() {
            if reply.offset < reply.chunk.len() {
                written += reply.drain_into(&mut out[written..]);
                continue;
            }
            if reply.verdict.is_some() {
                break;
            }
            match reply.audio.try_recv() {
                Ok(Msg::Audio(bytes)) => {
                    reply.chunk = bytes;
                    reply.offset = 0;
                }
                Ok(Msg::End) => reply.verdict = Some(Ok(())),
                Ok(Msg::Failed(error)) => reply.verdict = Some(Err(error)),
                Err(TryRecvError::Empty) => break,
                // The task ended without a verdict: the runtime shut down under it.
                Err(TryRecvError::Disconnected) => {
                    reply.verdict = Some(Err(SpeechError::failed("speech task ended early")));
                }
            }
        }
        if written > 0 {
            return Ok(TtsPoll::Audio { samples: written });
        }
        let Some(verdict) = reply.verdict.take() else {
            return Ok(TtsPoll::Pending);
        };
        self.state = State::Idle;
        verdict.map(|()| TtsPoll::Done)
    }

    fn cancel(&mut self) {
        self.state = State::Idle;
    }
}

impl Reply {
    /// Converts as many whole samples as fit; the chunk always holds whole ones.
    fn drain_into(&mut self, out: &mut [f32]) -> usize {
        let pending = self.chunk.get(self.offset..).unwrap_or_default();
        let mut written = 0;
        let (pairs, _) = pending.as_chunks::<2>();
        for (slot, pair) in out.iter_mut().zip(pairs) {
            let sample = i16::from_le_bytes(*pair);
            *slot = f32::from(sample) / PCM_SCALE;
            written += 1;
        }
        self.offset += written * 2;
        written
    }
}
