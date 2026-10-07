use std::io::Cursor;
use std::mem;
use std::net::IpAddr;
use std::sync::Arc;

use aulo_speech::{
    AudioFormat, AudioFrame, Capabilities, EngineInfo, EngineKind, EngineSpec, LanguageSupport,
    PIPELINE_SAMPLE_RATE_HZ, SpeechError, SttEngine, SttPoll, Transcript, TranscriptKind, TurnId,
};
use reqwest::header::HeaderValue;
use reqwest::{Client, Url, redirect};
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, error::TryRecvError};
use tokio_util::task::AbortOnDropHandle;

use crate::config::{CloudSttConfig, MAX_UTTERANCE_LIMIT, ResponseShape};
use crate::http::{self, Request};

/// Longest transcript handed to the pipeline, in bytes. A 10 minute utterance
/// is a few kilobytes of text, so this only bites on a misbehaving server,
/// whose reply must not become an unbounded prompt.
pub const MAX_TRANSCRIPT_BYTES: usize = 16 * 1024;

/// Room for partials; a full queue drops a partial, never the final.
const EVENT_QUEUE: usize = 16;
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const ENDPOINT_PATH: &str = "/audio/transcriptions";
const PCM_BITS: u16 = 16;

/// What the background request reports back to `poll`.
#[derive(Debug)]
pub(crate) enum Event {
    Partial(String),
    Final(String),
    Failed(SpeechError),
}

/// Everything engines built from one config share: the connection pool and the
/// validated endpoint.
#[derive(Debug)]
pub(crate) struct Shared {
    pub(crate) client: Client,
    pub(crate) endpoint: Url,
    pub(crate) bearer: Option<HeaderValue>,
    pub(crate) shape: ResponseShape,
    name: String,
    default_model: String,
    max_samples: usize,
    capabilities: Capabilities,
    handle: Handle,
}

/// Builds [`CloudStt`] engines for the registry from one validated config.
#[derive(Debug, Clone)]
pub struct CloudSttFactory {
    shared: Arc<Shared>,
}

impl CloudSttFactory {
    /// `runtime` runs the uploads; engines never block on it.
    pub fn new(config: CloudSttConfig, runtime: Handle) -> Result<Self, SpeechError> {
        let endpoint = Url::parse(&format!(
            "{}{ENDPOINT_PATH}",
            config.base_url.trim_end_matches('/')
        ))
        .map_err(|_| SpeechError::invalid("base url", "not a valid URL"))?;
        let local = is_loopback(&endpoint);
        // A bearer token over plain http to another host is readable on the path.
        let secure = match endpoint.scheme() {
            "https" => true,
            "http" => local || config.api_key.is_none(),
            _ => false,
        };
        if !secure {
            return Err(SpeechError::invalid(
                "base url",
                "https, or http only to localhost",
            ));
        }
        if config.max_utterance.is_zero() || config.max_utterance > MAX_UTTERANCE_LIMIT {
            return Err(SpeechError::invalid("max utterance", "0 to 10 minutes"));
        }
        let bearer = config
            .api_key
            .as_ref()
            .map(|key| HeaderValue::from_str(&format!("Bearer {}", key.expose())))
            .transpose()
            .map_err(|_| SpeechError::invalid("api key", "not a valid header value"))?
            .map(|mut value| {
                // Keeps the token out of reqwest's own Debug output.
                value.set_sensitive(true);
                value
            });
        let client = Client::builder()
            .user_agent(concat!("aulo-speech-cloud/", env!("CARGO_PKG_VERSION")))
            // A redirect would resend the audio, and the key, somewhere the user never chose.
            .redirect(redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(config.request_timeout)
            .build()
            .map_err(|e| SpeechError::unavailable(&e.without_url().to_string()))?;
        let max_samples =
            (config.max_utterance.as_secs_f64() * f64::from(PIPELINE_SAMPLE_RATE_HZ)) as usize;
        Ok(Self {
            shared: Arc::new(Shared {
                client,
                endpoint,
                bearer,
                shape: config.shape,
                name: config.name,
                default_model: config.default_model,
                max_samples,
                capabilities: Capabilities {
                    streaming: config.shape == ResponseShape::Stream,
                    languages: LanguageSupport::Any,
                    // A server on this machine (runa) works with the network down.
                    offline: local,
                    needs_network: true,
                },
                handle: runtime,
            }),
        })
    }

    pub fn build(&self, spec: &EngineSpec) -> Result<Box<dyn SttEngine>, SpeechError> {
        let shared = &self.shared;
        Ok(Box::new(CloudStt {
            info: EngineInfo {
                id: spec.engine.clone(),
                kind: EngineKind::Stt,
                name: shared.name.clone(),
                capabilities: shared.capabilities.clone(),
            },
            model: spec
                .model
                .clone()
                .unwrap_or_else(|| shared.default_model.clone()),
            shared: Arc::clone(shared),
            samples: Vec::new(),
            state: State::Idle,
        }))
    }

    /// The closure form `EngineRegistry::register` takes.
    pub fn into_factory(
        self,
    ) -> impl Fn(&EngineSpec) -> Result<Box<dyn SttEngine>, SpeechError> + Send + Sync + 'static
    {
        move |spec| self.build(spec)
    }
}

fn is_loopback(url: &Url) -> bool {
    url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .trim_matches(['[', ']'])
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    })
}

#[derive(Debug)]
enum State {
    Idle,
    Recording {
        turn: TurnId,
        language: Option<String>,
    },
    Waiting(Waiting),
}

/// An uploaded utterance. Dropping it aborts the request, which is how
/// `cancel` and `begin` stay instant.
#[derive(Debug)]
struct Waiting {
    turn: TurnId,
    language: Option<String>,
    events: mpsc::Receiver<Event>,
    _task: Option<AbortOnDropHandle<()>>,
    delivered: bool,
}

/// OpenAI-compatible transcription engine. Audio is buffered in storage sized
/// once for the longest utterance, so `push` never allocates or blocks.
#[derive(Debug)]
pub struct CloudStt {
    info: EngineInfo,
    shared: Arc<Shared>,
    model: String,
    samples: Vec<i16>,
    state: State,
}

impl SttEngine for CloudStt {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn begin(&mut self, turn_id: TurnId, language: Option<&str>) -> Result<(), SpeechError> {
        self.samples.clear();
        self.samples.reserve_exact(self.shared.max_samples);
        self.state = State::Recording {
            turn: turn_id,
            language: language.map(str::to_owned),
        };
        Ok(())
    }

    fn push(&mut self, frame: AudioFrame<'_>) -> Result<(), SpeechError> {
        if !matches!(self.state, State::Recording { .. }) {
            return Err(SpeechError::OutOfOrder("push outside an utterance"));
        }
        if frame.format() != AudioFormat::PIPELINE {
            return Err(SpeechError::invalid("audio format", "16 kHz mono only"));
        }
        if self.samples.len() + frame.len() > self.shared.max_samples {
            return Err(SpeechError::Overflow {
                dropped: frame.len(),
            });
        }
        self.samples
            .extend(frame.samples().iter().map(|s| to_pcm16(*s)));
        Ok(())
    }

    fn finish(&mut self) -> Result<(), SpeechError> {
        let State::Recording { turn, language } = &self.state else {
            return Err(SpeechError::OutOfOrder("finish before begin"));
        };
        let (turn, language) = (*turn, language.clone());
        let (sender, events) = mpsc::channel(EVENT_QUEUE);
        let task = if self.samples.is_empty() {
            // Nothing was said, so there is nothing to upload; the contract still wants one final.
            let _ = sender.try_send(Event::Final(String::new()));
            None
        } else {
            let wav = encode_wav(&self.samples).inspect_err(|_| self.state = State::Idle)?;
            self.samples.clear();
            let request = Request {
                wav,
                model: self.model.clone(),
                language: language.as_deref().and_then(iso_639_1),
            };
            Some(AbortOnDropHandle::new(self.shared.handle.spawn(http::run(
                Arc::clone(&self.shared),
                request,
                sender,
            ))))
        };
        self.state = State::Waiting(Waiting {
            turn,
            language,
            events,
            _task: task,
            delivered: false,
        });
        Ok(())
    }

    fn poll(&mut self, out: &mut Transcript) -> Result<SttPoll, SpeechError> {
        let State::Waiting(waiting) = &mut self.state else {
            return Ok(SttPoll::Pending);
        };
        if waiting.delivered {
            return Ok(SttPoll::Done);
        }
        let language = waiting.language.as_deref();
        match waiting.events.try_recv() {
            Ok(Event::Partial(text)) => {
                out.set(waiting.turn, TranscriptKind::Partial, &text, language);
                Ok(SttPoll::Updated)
            }
            Ok(Event::Final(text)) => {
                waiting.delivered = true;
                out.set(waiting.turn, TranscriptKind::Final, &text, language);
                Ok(SttPoll::Updated)
            }
            Ok(Event::Failed(error)) => {
                self.state = State::Idle;
                Err(error)
            }
            Err(TryRecvError::Empty) => Ok(SttPoll::Pending),
            Err(TryRecvError::Disconnected) => {
                // The task ended without a verdict: the runtime shut down under it.
                self.state = State::Idle;
                Err(SpeechError::failed("transcription task ended early"))
            }
        }
    }

    fn cancel(&mut self) {
        self.state = State::Idle;
        self.samples.clear();
    }
}

/// `f32` samples are in -1.0..=1.0; the clamp keeps overshoot from wrapping.
fn to_pcm16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16
}

fn encode_wav(samples: &[i16]) -> Result<Vec<u8>, SpeechError> {
    const HEADER_BYTES: usize = 44;
    let fail = |_: hound::Error| SpeechError::failed("cannot encode the utterance as WAV");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: PIPELINE_SAMPLE_RATE_HZ,
        bits_per_sample: PCM_BITS,
        sample_format: hound::SampleFormat::Int,
    };
    // hound patches the header sizes on finalize, so it needs the buffer to seek.
    let mut buffer = Cursor::new(Vec::with_capacity(HEADER_BYTES + mem::size_of_val(samples)));
    let mut writer = hound::WavWriter::new(&mut buffer, spec).map_err(fail)?;
    for sample in samples {
        writer.write_sample(*sample).map_err(fail)?;
    }
    writer.finalize().map_err(fail)?;
    Ok(buffer.into_inner())
}

/// The API wants ISO 639-1, two letters; anything else is left to detection.
fn iso_639_1(tag: &str) -> Option<String> {
    let primary = tag.split('-').next()?;
    (primary.len() == 2 && primary.bytes().all(|b| b.is_ascii_alphabetic()))
        .then(|| primary.to_ascii_lowercase())
}
