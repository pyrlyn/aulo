use std::time::Duration;

use aulo_speech::{SpeechError, Voice};
pub use aulo_types::ApiKey;

/// Longest utterance the engine buffers. A 10 minute 16 kHz mono WAV is about
/// 19 MB, under OpenAI's 25 MB upload limit, and bounds memory per engine.
pub const MAX_UTTERANCE_LIMIT: Duration = Duration::from_secs(600);

const DEFAULT_MAX_UTTERANCE: Duration = Duration::from_secs(30);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// How the server is asked to answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseShape {
    /// One JSON object after the whole utterance is decoded.
    #[default]
    Json,
    /// Server-sent events with growing partial transcripts (`stream=true`).
    Stream,
}

#[derive(Debug, Clone)]
pub struct CloudSttConfig {
    /// Name for engine pickers, for example "OpenAI".
    pub name: String,
    /// Up to and including the version segment, for example
    /// `https://api.openai.com/v1`.
    pub base_url: String,
    /// Used when the resolved engine spec names no model.
    pub default_model: String,
    /// `None` for servers that take no key, such as a local runa.
    pub api_key: Option<ApiKey>,
    pub shape: ResponseShape,
    /// Audio past this is dropped with `SpeechError::Overflow`.
    pub max_utterance: Duration,
    /// Whole request, upload and reply included; there is no retry.
    pub request_timeout: Duration,
}

impl CloudSttConfig {
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
            shape: ResponseShape::default(),
            max_utterance: DEFAULT_MAX_UTTERANCE,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }
}

/// OpenAI rejects longer input, and a local server that accepts more is still
/// better served sentence by sentence.
pub const DEFAULT_MAX_TEXT_CHARS: usize = 4096;
/// Bounds the text queued for one request, so an oversized sentence cannot
/// become an oversized upload.
pub const MAX_TEXT_CHARS_LIMIT: usize = 65_536;
/// About 5 minutes of 24 kHz 16-bit mono.
const DEFAULT_MAX_AUDIO_BYTES: usize = 14 * 1024 * 1024;
/// About 22 minutes of 24 kHz 16-bit mono; one reply never needs more, and
/// the cap is what stops a server that never ends the stream.
pub const MAX_AUDIO_BYTES_LIMIT: usize = 64 * 1024 * 1024;

/// The voices `createSpeech` documents, for [`CloudTtsConfig::voices`].
pub fn openai_voices() -> Vec<Voice> {
    [
        "alloy", "ash", "ballad", "coral", "echo", "fable", "nova", "onyx", "sage", "shimmer",
        "verse",
    ]
    .into_iter()
    .map(|id| Voice {
        id: id.to_owned(),
        name: id.to_owned(),
        // The model follows the language of the text.
        language: None,
    })
    .collect()
}

#[derive(Debug, Clone)]
pub struct CloudTtsConfig {
    /// Name for engine pickers, for example "OpenAI".
    pub name: String,
    /// Up to and including the version segment, for example
    /// `https://api.openai.com/v1`.
    pub base_url: String,
    /// Used when the resolved engine spec names no model.
    pub default_model: String,
    /// What `Engine::voices` lists; any other voice is `Unsupported`.
    pub voices: Vec<Voice>,
    /// Must be one of `voices`; used when neither the spec nor the request names one.
    pub default_voice: String,
    /// `None` for servers that take no key, such as a local runa.
    pub api_key: Option<ApiKey>,
    /// Longest text sent in one request, in characters.
    pub max_text_chars: usize,
    /// Audio past this fails the reply.
    pub max_audio_bytes: usize,
    /// Wait for the reply headers and between audio chunks. It is not a total:
    /// audio is read at playback speed, so a long reply takes long.
    pub request_timeout: Duration,
}

impl CloudTtsConfig {
    /// The limits every engine built from this config relies on.
    pub(crate) fn validate(&self) -> Result<(), SpeechError> {
        if self.max_text_chars == 0 || self.max_text_chars > MAX_TEXT_CHARS_LIMIT {
            return Err(SpeechError::invalid("max text", "1 to 65536 characters"));
        }
        if self.max_audio_bytes == 0 || self.max_audio_bytes > MAX_AUDIO_BYTES_LIMIT {
            return Err(SpeechError::invalid("max audio", "1 byte to 64 MiB"));
        }
        if self.request_timeout.is_zero() {
            return Err(SpeechError::invalid("request timeout", "must not be zero"));
        }
        if !self.voices.iter().any(|v| v.id == self.default_voice) {
            return Err(SpeechError::invalid(
                "default voice",
                "not in the voice list",
            ));
        }
        Ok(())
    }

    pub fn new(
        name: impl Into<String>,
        base_url: impl Into<String>,
        default_model: impl Into<String>,
        voices: Vec<Voice>,
        default_voice: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            base_url: base_url.into(),
            default_model: default_model.into(),
            voices,
            default_voice: default_voice.into(),
            api_key: None,
            max_text_chars: DEFAULT_MAX_TEXT_CHARS,
            max_audio_bytes: DEFAULT_MAX_AUDIO_BYTES,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }
}
