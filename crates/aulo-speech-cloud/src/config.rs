use std::fmt;
use std::time::Duration;

/// Longest utterance the engine buffers. A 10 minute 16 kHz mono WAV is about
/// 19 MB, under OpenAI's 25 MB upload limit, and bounds memory per engine.
pub const MAX_UTTERANCE_LIMIT: Duration = Duration::from_secs(600);

const DEFAULT_MAX_UTTERANCE: Duration = Duration::from_secs(30);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// A secret resolved from the caller's handle. `Debug` never prints it, so a
/// config logged or dumped in a panic cannot leak it.
#[derive(Clone)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

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
