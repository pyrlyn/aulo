//! ElevenLabs speech synthesis over the `stream-input` WebSocket.
//!
//! One reply is one connection: the socket opens at `begin`, each pushed text
//! goes out as a message flushed at once, and `finish` closes the text side.
//! Audio comes back as base64 inside JSON messages, is decoded and queued in
//! the same bounded 100 ms chunks as the OpenAI engine.
//!
//! Protocol, checked 2026-10-07 against ElevenLabs' API reference
//! <https://elevenlabs.io/docs/api-reference/text-to-speech/v-1-text-to-speech-voice-id-stream-input>,
//! the WebSocket guide
//! <https://elevenlabs.io/docs/eleven-api/guides/how-to/websockets/realtime-tts>
//! and the output formats on
//! <https://elevenlabs.io/docs/api-reference/text-to-speech/convert>:
//! - `wss://api.elevenlabs.io/v1/text-to-speech/{voice_id}/stream-input` with
//!   the query parameters `model_id`, `output_format` (`pcm_8000` to
//!   `pcm_48000`; 44.1 kHz needs a Pro plan) and `inactivity_timeout`
//!   (default 20, at most 180 seconds);
//! - the key is the `xi-api-key` header;
//! - the first message is `{"text": " ", "voice_settings": {...}}`, later ones
//!   `{"text": "... ", "flush": true}` (text ends in one space, `flush` forces
//!   generation of what is buffered), and `{"text": ""}` closes the stream and
//!   generates what is left;
//! - the server answers `{"audio": "<base64>", "alignment": ...}` and finally
//!   `{"isFinal": true}`.
//!
//! Unverified, because the documentation says nothing about them: the layout
//! of error messages and close codes (a message with `error` or `message` and
//! no audio is read as an error, and close code 1008 as a refused key or quota),
//! the maximum message size (capped here at 1 MiB), the `speed` voice setting
//! (taken from the REST reference, 0.7 to 1.2), and the 16-bit signed
//! little-endian mono layout of `pcm_*`.
//!
//! Nothing is retried, and no server text is ever echoed: error messages quote
//! keys and account details.

use std::sync::Arc;
use std::time::Duration;

use aulo_speech::{
    AudioFormat, Capabilities, EngineInfo, EngineKind, EngineSpec, LanguageSupport, SpeechError,
    TtsEngine, TtsPoll, TtsRequest, Voice,
};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use reqwest::Url;
use reqwest::header::HeaderValue;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::runtime::Handle;
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;
use tokio_tungstenite::connect_async_tls_with_config;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{Error as WsError, Message, client::IntoClientRequest};

use crate::config::{CloudTtsConfig, MAX_TEXT_CHARS_LIMIT};
use crate::net::{self, Endpoint};
use crate::reply::{Chunker, Msg, Reply, Slot, Stop, TextLimits, chunk_bytes, conclude};

pub const DEFAULT_BASE_URL: &str = "wss://api.elevenlabs.io/v1";
/// The sample rates `output_format` offers as `pcm_<rate>`.
pub const PCM_SAMPLE_RATES_HZ: [u32; 7] = [8_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000];
const NOUN: &str = "speech";
const KEY_HEADER: &str = "xi-api-key";
/// A few seconds of audio as base64; a bigger message is a broken or hostile server.
const MAX_MESSAGE_BYTES: usize = 1 << 20;
const MAX_INACTIVITY: Duration = Duration::from_secs(180);
const MAX_VOICE_ID_CHARS: usize = 64;
/// Characters in a whole reply: far above any answer, and a bound on spend.
pub const MAX_REPLY_CHARS_LIMIT: usize = 1 << 20;
/// Words in a server error that mean the account, not the request, is the problem.
const ACCOUNT_WORDS: &str = "auth api_key quota rate credit subscription permission";

/// An ElevenLabs engine's settings on top of the shared TTS ones: `tts.base_url`
/// is [`DEFAULT_BASE_URL`], `tts.default_model` the `model_id`, `tts.voices`
/// ElevenLabs voice ids, and `tts.request_timeout` bounds the handshake and the
/// silence while the reply is drained after `finish`.
#[derive(Debug, Clone)]
pub struct ElevenLabsTtsConfig {
    pub tts: CloudTtsConfig,
    /// One of [`PCM_SAMPLE_RATES_HZ`].
    pub sample_rate_hz: u32,
    /// Characters in one reply; more is `Invalid`.
    pub max_reply_chars: usize,
    /// How long the server keeps an idle socket open, 1 to 180 seconds. A reply
    /// that waits longer between texts than this fails.
    pub inactivity_timeout: Duration,
}

impl ElevenLabsTtsConfig {
    pub fn new(
        model_id: impl Into<String>,
        voices: Vec<Voice>,
        default_voice: impl Into<String>,
    ) -> Self {
        Self {
            tts: CloudTtsConfig::new(
                "ElevenLabs",
                DEFAULT_BASE_URL,
                model_id,
                voices,
                default_voice,
            ),
            sample_rate_hz: 24_000,
            max_reply_chars: MAX_TEXT_CHARS_LIMIT,
            inactivity_timeout: Duration::from_secs(20),
        }
    }
}

#[derive(Debug)]
struct Shared {
    /// Up to `.../text-to-speech`; the voice and query are added per reply.
    endpoint: Url,
    key: Option<HeaderValue>,
    config: ElevenLabsTtsConfig,
    capabilities: Capabilities,
    handle: Handle,
}

/// Builds [`ElevenLabsTts`] engines for the registry from one validated config.
#[derive(Debug, Clone)]
pub struct ElevenLabsTtsFactory {
    shared: Arc<Shared>,
}

impl ElevenLabsTtsFactory {
    /// `runtime` runs the sockets; engines never block on it.
    pub fn new(config: ElevenLabsTtsConfig, runtime: Handle) -> Result<Self, SpeechError> {
        let has_key = config.tts.api_key.is_some();
        let Endpoint { url, local } =
            net::ws_endpoint(&config.tts.base_url, "/text-to-speech", has_key)?;
        config.tts.validate()?;
        if !PCM_SAMPLE_RATES_HZ.contains(&config.sample_rate_hz) {
            return Err(SpeechError::invalid("sample rate", "a pcm_* rate"));
        }
        if config.max_reply_chars == 0 || config.max_reply_chars > MAX_REPLY_CHARS_LIMIT {
            return Err(SpeechError::invalid("max reply", "1 to 1048576 characters"));
        }
        if config.inactivity_timeout.is_zero() || config.inactivity_timeout > MAX_INACTIVITY {
            return Err(SpeechError::invalid(
                "inactivity timeout",
                "1 to 180 seconds",
            ));
        }
        // Ids go into the URL path, so anything but the characters ids are made of is refused.
        if !config.tts.voices.iter().all(|v| voice_id_ok(&v.id)) {
            return Err(SpeechError::invalid(
                "voice list",
                "ids are letters, digits, - and _",
            ));
        }
        Ok(Self {
            shared: Arc::new(Shared {
                endpoint: url,
                key: net::secret_header(config.tts.api_key.as_ref(), "")?,
                capabilities: Capabilities {
                    streaming: true,
                    languages: LanguageSupport::Any,
                    // A local stand-in server works with the network down.
                    offline: local,
                    needs_network: true,
                },
                config,
                handle: runtime,
            }),
        })
    }

    /// An unknown `spec.voice` is `Unsupported`, so the registry can fall back.
    pub fn build(&self, spec: &EngineSpec) -> Result<Box<dyn TtsEngine>, SpeechError> {
        let tts = &self.shared.config.tts;
        let voice = spec
            .voice
            .clone()
            .unwrap_or_else(|| tts.default_voice.clone());
        check_voice(&self.shared, &voice)?;
        Ok(Box::new(ElevenLabsTts {
            info: EngineInfo {
                id: spec.engine.clone(),
                kind: EngineKind::Tts,
                name: tts.name.clone(),
                capabilities: self.shared.capabilities.clone(),
            },
            model: spec
                .model
                .clone()
                .unwrap_or_else(|| tts.default_model.clone()),
            voice,
            shared: Arc::clone(&self.shared),
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

fn voice_id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_VOICE_ID_CHARS
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn check_voice(shared: &Shared, voice: &str) -> Result<(), SpeechError> {
    if shared.config.tts.voices.iter().any(|v| v.id == voice) {
        Ok(())
    } else {
        Err(SpeechError::unsupported(voice))
    }
}

/// ElevenLabs streaming speech engine.
#[derive(Debug)]
pub struct ElevenLabsTts {
    info: EngineInfo,
    shared: Arc<Shared>,
    model: String,
    voice: String,
    reply: Slot,
}

/// What one socket needs, fixed at `begin`.
struct Session {
    url: Url,
    /// The first message: a lone space, which the server requires, and the voice settings.
    init: String,
}

impl ElevenLabsTts {
    fn session(&self, voice: &str, speed: f32) -> Result<Session, SpeechError> {
        let config = &self.shared.config;
        let mut url = self.shared.endpoint.clone();
        url.path_segments_mut()
            .map_err(|()| SpeechError::invalid("base url", "cannot hold a path"))?
            .extend([voice, "stream-input"]);
        url.query_pairs_mut()
            .append_pair("model_id", &self.model)
            // Raw samples need no container parser, so audio plays as it arrives.
            .append_pair("output_format", &format!("pcm_{}", config.sample_rate_hz))
            .append_pair(
                "inactivity_timeout",
                &config.inactivity_timeout.as_secs().to_string(),
            );
        let mut init = json!({"text": " "});
        if speed != 1.0 {
            // Two decimals: `f32` widened to `f64` would print as 1.2000000476837158.
            let speed = (f64::from(speed.clamp(0.7, 1.2)) * 100.0).round() / 100.0;
            init["voice_settings"] = json!({ "speed": speed });
        }
        Ok(Session {
            url,
            init: init.to_string(),
        })
    }
}

impl TtsEngine for ElevenLabsTts {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn voices(&self) -> &[Voice] {
        &self.shared.config.tts.voices
    }

    fn begin(&mut self, request: &TtsRequest<'_>) -> Result<AudioFormat, SpeechError> {
        self.cancel();
        let voice = request.voice.unwrap_or(&self.voice);
        check_voice(&self.shared, voice)?;
        let format = AudioFormat::new(self.shared.config.sample_rate_hz, 1)?;
        let session = self.session(voice, request.rate.get())?;
        let limits = TextLimits {
            each: self.shared.config.tts.max_text_chars,
            total: self.shared.config.max_reply_chars,
        };
        let shared = Arc::clone(&self.shared);
        self.reply
            .set(Reply::start(&self.shared.handle, limits, |texts, audio| {
                run(shared, session, texts, audio)
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

async fn run(
    shared: Arc<Shared>,
    session: Session,
    texts: mpsc::Receiver<String>,
    audio: mpsc::Sender<Msg>,
) {
    let outcome = converse(&shared, &session, texts, &audio).await;
    conclude(&audio, outcome).await;
}

async fn converse(
    shared: &Shared,
    session: &Session,
    texts: mpsc::Receiver<String>,
    audio: &mpsc::Sender<Msg>,
) -> Result<(), Stop> {
    let mut request = session
        .url
        .as_str()
        .into_client_request()
        .map_err(|_| SpeechError::failed("cannot build the speech request"))?;
    if let Some(key) = &shared.key {
        request.headers_mut().insert(KEY_HEADER, key.clone());
    }
    let limits = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));
    let limit = shared.config.tts.request_timeout;
    let (socket, _) = timeout(
        limit,
        connect_async_tls_with_config(request, Some(limits), false, None),
    )
    .await
    .map_err(|_| SpeechError::unavailable("cannot reach the speech server"))?
    .map_err(connect_error)?;
    let (mut sink, mut stream) = socket.split();
    let (closing, mut closed) = watch::channel(false);
    let write = write_texts(&mut sink, &session.init, texts, closing);
    let read = read_audio(&mut stream, audio, shared, &mut closed);
    tokio::pin!(write, read);
    tokio::select! {
        verdict = &mut read => verdict,
        written = &mut write => {
            // A server that hangs up breaks the write side first; the reader still
            // delivers the audio sent before that and knows better why it ended.
            let read = read.await;
            match written {
                Ok(()) => read,
                Err(error) => read.and(Err(Stop::from(error))),
            }
        }
    }
}

/// Sends the texts as they come, then asks the server to finish.
async fn write_texts<S>(
    sink: &mut S,
    init: &str,
    mut texts: mpsc::Receiver<String>,
    closing: watch::Sender<bool>,
) -> Result<(), SpeechError>
where
    S: Sink<Message, Error = WsError> + Unpin,
{
    let mut sent = false;
    let outcome = async {
        sink.send(Message::text(init)).await?;
        while let Some(text) = texts.recv().await {
            sent = true;
            // `flush` makes each sentence speak now instead of waiting for the server's chunk schedule.
            let message = json!({"text": format!("{text} "), "flush": true});
            sink.send(Message::text(message.to_string())).await?;
        }
        closing.send_replace(true);
        // With nothing said there is nothing to generate, and an empty reply would only be billed or refused.
        let last = if sent {
            Message::text(r#"{"text":""}"#)
        } else {
            Message::Close(None)
        };
        sink.send(last).await
    }
    .await;
    match outcome {
        // The reader reports why the server hung up.
        Ok(()) | Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => Ok(()),
        Err(error) => Err(stream_error(&error)),
    }
}

#[derive(Deserialize)]
struct Frame {
    audio: Option<String>,
    #[serde(rename = "isFinal")]
    is_final: Option<bool>,
    error: Option<Value>,
    message: Option<Value>,
    code: Option<u16>,
}

async fn read_audio<S>(
    stream: &mut S,
    audio: &mpsc::Sender<Msg>,
    shared: &Shared,
    closed: &mut watch::Receiver<bool>,
) -> Result<(), Stop>
where
    S: Stream<Item = Result<Message, WsError>> + Unpin,
{
    let sample_rate = shared.config.sample_rate_hz;
    let mut chunker = Chunker::new(chunk_bytes(sample_rate));
    let mut total = 0usize;
    loop {
        let message = match next_message(stream, closed, shared.config.tts.request_timeout).await? {
            Some(Ok(message)) => message,
            Some(Err(WsError::ConnectionClosed)) | None => return hung_up(None, *closed.borrow()),
            Some(Err(error)) => return Err(stream_error(&error).into()),
        };
        match message {
            Message::Text(text) => {
                let frame: Frame = serde_json::from_str(&text).map_err(|_| {
                    SpeechError::failed("the speech server sent an unreadable message")
                })?;
                if let Some(encoded) = frame.audio.as_deref().filter(|a| !a.is_empty()) {
                    let bytes = STANDARD.decode(encoded).map_err(|_| {
                        SpeechError::failed("the speech server sent undecodable audio")
                    })?;
                    total = total.saturating_add(bytes.len());
                    if total > shared.config.tts.max_audio_bytes {
                        return Err(SpeechError::failed("audio is too long").into());
                    }
                    // A full queue makes this wait, which stops reading the socket: the server is slowed, not buffered.
                    chunker.feed(&bytes, audio).await?;
                } else if frame.error.is_some() || frame.message.is_some() {
                    return Err(server_error(&frame).into());
                } else if frame.is_final == Some(true) {
                    chunker.clear();
                    return Ok(());
                }
            }
            Message::Close(frame) => return hung_up(frame.map(|f| f.code), *closed.borrow()),
            Message::Binary(_) => {
                return Err(
                    SpeechError::failed("the speech server sent an unexpected message").into(),
                );
            }
            _ => {}
        }
    }
}

/// Waits without a limit while texts may still come, since the server's own
/// inactivity timeout ends a socket nobody uses; once `finish` closed the text
/// side, each message must arrive within `limit`.
async fn next_message<S>(
    stream: &mut S,
    closed: &mut watch::Receiver<bool>,
    limit: Duration,
) -> Result<Option<Result<Message, WsError>>, SpeechError>
where
    S: Stream<Item = Result<Message, WsError>> + Unpin,
{
    if !*closed.borrow() {
        tokio::select! {
            message = stream.next() => return Ok(message),
            _ = closed.changed() => {}
        }
    }
    timeout(limit, stream.next())
        .await
        .map_err(|_| net::timed_out(NOUN))
}

/// The server closed the socket: fine once the stream was closed on our side.
fn hung_up(code: Option<CloseCode>, closing: bool) -> Result<(), Stop> {
    match code {
        // Policy violation is what a refused key or an exhausted quota closes with.
        Some(CloseCode::Policy) => {
            Err(SpeechError::unavailable("the speech server refused the request").into())
        }
        None | Some(CloseCode::Normal | CloseCode::Status) if closing => Ok(()),
        _ => Err(SpeechError::failed("the speech server closed the connection early").into()),
    }
}

fn server_error(frame: &Frame) -> SpeechError {
    let text = [&frame.error, &frame.message]
        .into_iter()
        .filter_map(|v| v.as_ref()?.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    // Only the category is kept: the text itself is untrusted and may quote the key.
    let account = matches!(frame.code, Some(401 | 403 | 429))
        || ACCOUNT_WORDS.split(' ').any(|word| text.contains(word));
    if account {
        SpeechError::unavailable(
            "the speech server rejected the account (key, quota or rate limit)",
        )
    } else {
        SpeechError::failed("the speech server reported an error")
    }
}

fn connect_error(error: WsError) -> SpeechError {
    match error {
        WsError::Http(response) => net::status_error(response.status()),
        WsError::Io(_) | WsError::Tls(_) => {
            SpeechError::unavailable("cannot reach the speech server")
        }
        _ => SpeechError::failed("the speech server refused the connection"),
    }
}

fn stream_error(error: &WsError) -> SpeechError {
    match error {
        WsError::Capacity(_) => {
            SpeechError::failed("the speech server sent a message that is too large")
        }
        _ => SpeechError::failed("the speech connection failed"),
    }
}
