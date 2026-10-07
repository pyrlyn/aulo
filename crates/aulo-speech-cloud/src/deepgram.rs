//! Streaming speech-to-text over Deepgram's live API, `wss://api.deepgram.com/v1/listen`.
//!
//! Sources, all checked 2026-10-07 on developers.deepgram.com:
//!
//! - The API reference, `/reference/speech-to-text/listen-streaming`: the
//!   handshake, `Authorization: Token <API_KEY>`, the query parameters
//!   (`model`, `language`, `encoding`, `sample_rate`, `channels`,
//!   `interim_results`, `punctuate`), binary audio frames, the `KeepAlive`,
//!   `CloseStream` and `Finalize` text messages, and the `Results`, `Metadata`,
//!   `UtteranceEnd`, `SpeechStarted` and `Error` replies.
//! - `/docs/audio-keep-alive`: a `KeepAlive` text frame every 3 to 5 seconds,
//!   because 10 seconds without audio or `KeepAlive` closes the stream.
//! - `/docs/close-stream`: `CloseStream` makes the server process the audio it
//!   still holds, return the final results and a `Metadata` message, and then
//!   close the socket.
//! - `/docs/understand-endpointing-interim-results`: an `is_final: false`
//!   result is a preliminary transcript of the audio since the last final one;
//!   `is_final: true` results are appended to build the utterance. Deepgram
//!   sets `speech_final` on the last of them when it hears a pause.
//! - `/docs/stt-troubleshooting-websocket-data-and-net-errors`: a failed
//!   upgrade is an HTTP 4xx or 5xx, and a stream that breaks later ends with a
//!   close frame (1008 `DATA-0000`, 1011 `NET-0000` to `NET-0002`).
//! - `/reference/errors`: 401 and 403 for a key or model the project may not
//!   use, 402 for no credits, 429 for the rate limit.
//! - `/docs/multilingual-code-switching`: `language=multi` on Nova models.
//!
//! Not verified against the live service (no key was available): that
//! `language=multi` is accepted with every model a caller may configure, and
//! how a quota that runs out in mid-stream is reported (it is treated as any
//! other close).
//!
//! The engine contract delivers one final transcript, after `finish`, so a
//! `speech_final` result does not end the utterance here: end of turn is the
//! turn detector's job. Every `is_final` result is committed to the transcript
//! and every result is reported as a cumulative partial; the final is what the
//! server flushes after `CloseStream`.
//!
//! The Deepgram SDK crate was not used. 0.11.0 is maintained (MIT, released
//! 2026-09-14), but its `listen` feature builds on tokio-tungstenite 0.28 with
//! its own rustls and webpki setup, next to the 0.30 the speech synthesis
//! engine uses; it enables tokio's `full` feature; and it does not let the
//! caller cap WebSocket message sizes. One socket stack lets the limits and
//! the trust roots follow the rest of this crate.

mod session;

use std::sync::Arc;
use std::time::Duration;

use aulo_speech::{
    AudioFormat, AudioFrame, Capabilities, EngineInfo, EngineKind, EngineSpec, LanguageSupport,
    PIPELINE_SAMPLE_RATE_HZ, SpeechError, SttEngine, SttPoll, Transcript, TurnId,
};
use reqwest::Url;
use reqwest::header::HeaderValue;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio_util::task::AbortOnDropHandle;

use crate::config::{ApiKey, MAX_UTTERANCE_LIMIT};
use crate::engine::{Event, Waiting, to_pcm16};
use crate::net::{self, Endpoint};
use session::Session;

/// Deepgram's hosted API, up to and including the version segment.
pub const DEEPGRAM_BASE_URL: &str = "wss://api.deepgram.com/v1";

const ENDPOINT_PATH: &str = "/listen";
const DEFAULT_MAX_UTTERANCE: Duration = Duration::from_secs(60);
const DEFAULT_KEEPALIVE: Duration = Duration::from_secs(4);
const DEFAULT_FINISH_TIMEOUT: Duration = Duration::from_secs(10);
/// Room for partials; a full queue drops a partial, never the final.
const EVENT_QUEUE: usize = 16;
/// Chunks, not frames: a frame is cut into pieces of at most `CHUNK_SAMPLES`,
/// so the queue holds at most `AUDIO_QUEUE_CHUNKS * 6.4 KB` however large the
/// frames are. At 100 ms a chunk that is a little over six seconds of audio.
const AUDIO_QUEUE_CHUNKS: usize = 64;
const CHUNK_SAMPLES: usize = 3200;
const MAX_LANGUAGE_TAG_BYTES: usize = 35;

/// `Debug` is safe to print: the key redacts itself.
#[derive(Debug, Clone)]
pub struct DeepgramSttConfig {
    /// Name for engine pickers, for example "Deepgram".
    pub name: String,
    /// `wss`, or `ws` only to this machine (a self-hosted Deepgram), up to and
    /// including the version segment. [`DEEPGRAM_BASE_URL`] for the hosted API.
    pub base_url: String,
    /// Used when the resolved engine spec names no model, for example `nova-3`.
    pub default_model: String,
    /// `None` only for a self-hosted server that takes no key.
    pub api_key: Option<ApiKey>,
    /// Audio past this is dropped with `SpeechError::Overflow`.
    pub max_utterance: Duration,
    /// Idle time after which a `KeepAlive` is sent; Deepgram closes after 10 s.
    pub keepalive_interval: Duration,
    /// How long the server may take to flush the last results after `finish`.
    pub finish_timeout: Duration,
}

impl DeepgramSttConfig {
    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        default_model: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            default_model: default_model.into(),
            api_key: None,
            max_utterance: DEFAULT_MAX_UTTERANCE,
            keepalive_interval: DEFAULT_KEEPALIVE,
            finish_timeout: DEFAULT_FINISH_TIMEOUT,
        }
    }
}

#[derive(Debug)]
struct Shared {
    endpoint: Url,
    auth: Option<HeaderValue>,
    name: String,
    default_model: String,
    max_samples: usize,
    keepalive: Duration,
    finish_timeout: Duration,
    capabilities: Capabilities,
    handle: Handle,
}

/// Builds [`DeepgramStt`] engines for the registry from one validated config.
#[derive(Debug, Clone)]
pub struct DeepgramSttFactory {
    shared: Arc<Shared>,
}

impl DeepgramSttFactory {
    /// `runtime` runs the sockets; engines never block on it.
    pub fn new(config: DeepgramSttConfig, runtime: Handle) -> Result<Self, SpeechError> {
        let Endpoint { url, local } =
            net::ws_endpoint(&config.base_url, ENDPOINT_PATH, config.api_key.is_some())?;
        if config.max_utterance.is_zero() || config.max_utterance > MAX_UTTERANCE_LIMIT {
            return Err(SpeechError::invalid("max utterance", "0 to 10 minutes"));
        }
        if config.keepalive_interval.is_zero() || config.finish_timeout.is_zero() {
            return Err(SpeechError::invalid("timeout", "must not be zero"));
        }
        let max_samples =
            (config.max_utterance.as_secs_f64() * f64::from(PIPELINE_SAMPLE_RATE_HZ)) as usize;
        Ok(Self {
            shared: Arc::new(Shared {
                endpoint: url,
                // Deepgram takes `Token <key>`; the bearer form is for its temporary tokens.
                auth: net::secret_header(config.api_key.as_ref(), "Token ")?,
                name: config.name,
                default_model: config.default_model,
                max_samples,
                keepalive: config.keepalive_interval,
                finish_timeout: config.finish_timeout,
                capabilities: Capabilities {
                    streaming: true,
                    languages: LanguageSupport::Any,
                    offline: local,
                    needs_network: true,
                },
                handle: runtime,
            }),
        })
    }

    pub fn build(&self, spec: &EngineSpec) -> Result<Box<dyn SttEngine>, SpeechError> {
        let shared = &self.shared;
        Ok(Box::new(DeepgramStt {
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

#[derive(Debug)]
enum State {
    Idle,
    Recording(Live),
    Finished(Waiting),
}

/// An utterance whose socket is open, or opening, in the background.
#[derive(Debug)]
struct Live {
    turn: TurnId,
    language: Option<String>,
    events: Waiting,
    /// Dropping it ends the audio: the session drains what is queued, sends
    /// `CloseStream` and waits for the final results.
    audio: mpsc::Sender<Vec<u8>>,
    sent: usize,
}

/// Deepgram live transcription engine. The socket opens at `begin`, so the
/// handshake runs while the speaker is still starting.
#[derive(Debug)]
pub struct DeepgramStt {
    info: EngineInfo,
    shared: Arc<Shared>,
    model: String,
    state: State,
}

impl DeepgramStt {
    fn session_url(&self, language: &str) -> Url {
        let mut url = self.shared.endpoint.clone();
        url.query_pairs_mut()
            .append_pair("model", &self.model)
            .append_pair("language", language)
            .append_pair("encoding", "linear16")
            .append_pair("sample_rate", &PIPELINE_SAMPLE_RATE_HZ.to_string())
            .append_pair("channels", "1")
            .append_pair("interim_results", "true")
            .append_pair("punctuate", "true");
        url
    }
}

impl SttEngine for DeepgramStt {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn begin(&mut self, turn_id: TurnId, language: Option<&str>) -> Result<(), SpeechError> {
        // The previous socket closes first so two never overlap.
        self.state = State::Idle;
        if let Some(tag) = language {
            let plain = |c: char| c.is_ascii_alphanumeric() || c == '-';
            if tag.is_empty() || tag.len() > MAX_LANGUAGE_TAG_BYTES || !tag.chars().all(plain) {
                return Err(SpeechError::unsupported("language tag"));
            }
        }
        let session = Session {
            // Deepgram's default is English; `multi` is its detect-the-language mode.
            url: self.session_url(language.unwrap_or("multi")),
            auth: self.shared.auth.clone(),
            keepalive: self.shared.keepalive,
            finish_timeout: self.shared.finish_timeout,
        };
        let (audio, audio_rx) = mpsc::channel(AUDIO_QUEUE_CHUNKS);
        let (sender, receiver) = mpsc::channel(EVENT_QUEUE);
        let task = AbortOnDropHandle::new(
            self.shared
                .handle
                .spawn(session::run(session, audio_rx, sender)),
        );
        let language = language.map(str::to_owned);
        self.state = State::Recording(Live {
            turn: turn_id,
            language: language.clone(),
            events: Waiting::new(turn_id, language, receiver, Some(task)),
            audio,
            sent: 0,
        });
        Ok(())
    }

    fn push(&mut self, frame: AudioFrame<'_>) -> Result<(), SpeechError> {
        let State::Recording(live) = &mut self.state else {
            return Err(SpeechError::OutOfOrder("push outside an utterance"));
        };
        if frame.format() != AudioFormat::PIPELINE {
            return Err(SpeechError::invalid("audio format", "16 kHz mono only"));
        }
        let room = self.shared.max_samples.saturating_sub(live.sent);
        let (kept, over) = frame.samples().split_at(frame.len().min(room));
        let mut dropped = over.len();
        for chunk in kept.chunks(CHUNK_SAMPLES) {
            let bytes = chunk
                .iter()
                .flat_map(|sample| to_pcm16(*sample).to_le_bytes())
                .collect();
            match live.audio.try_send(bytes) {
                Ok(()) => live.sent += chunk.len(),
                Err(mpsc::error::TrySendError::Full(_)) => dropped += chunk.len(),
                // The session ended; `poll` reports why.
                Err(mpsc::error::TrySendError::Closed(_)) => {}
            }
        }
        match dropped {
            0 => Ok(()),
            dropped => Err(SpeechError::Overflow { dropped }),
        }
    }

    fn finish(&mut self) -> Result<(), SpeechError> {
        match std::mem::replace(&mut self.state, State::Idle) {
            State::Recording(live) if live.sent == 0 => {
                // Nothing was said: the contract still wants one final, and an
                // empty stream is something Deepgram answers with an error.
                let (sender, receiver) = mpsc::channel(1);
                let _ = sender.try_send(Event::Final(String::new()));
                self.state =
                    State::Finished(Waiting::new(live.turn, live.language, receiver, None));
                Ok(())
            }
            State::Recording(live) => {
                drop(live.audio);
                self.state = State::Finished(live.events);
                Ok(())
            }
            other => {
                self.state = other;
                Err(SpeechError::OutOfOrder("finish before begin"))
            }
        }
    }

    fn poll(&mut self, out: &mut Transcript) -> Result<SttPoll, SpeechError> {
        let waiting = match &mut self.state {
            State::Idle => return Ok(SttPoll::Pending),
            State::Recording(live) => &mut live.events,
            State::Finished(waiting) => waiting,
        };
        let polled = waiting.poll(out);
        if polled.is_err() {
            self.state = State::Idle;
        }
        polled
    }

    fn cancel(&mut self) {
        self.state = State::Idle;
    }
}
