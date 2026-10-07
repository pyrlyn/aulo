use std::fmt;
use std::net::IpAddr;
use std::time::Duration;

use serde_json::Value;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use url::Url;

use crate::RealtimeError;

pub const DEFAULT_BASE_URL: &str = "wss://api.openai.com/v1";
pub const DEFAULT_REST_URL: &str = "https://api.openai.com/v1";
/// The only PCM rate the Realtime API takes; GPT-Live also takes 16 kHz.
pub const SAMPLE_RATE_HZ: u32 = 24_000;
const MAX_NAME_CHARS: usize = 128;

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

/// The two OpenAI voice protocols. They share the socket and the audio
/// encoding but not the event names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    /// `wss://.../realtime?model=...`, session configured with `session.update`.
    #[default]
    Realtime,
    /// `wss://.../live/sessions`, session configured with `session.start`.
    Live,
}

/// A function the model may call. Realtime only: GPT-Live delegates tool use
/// to a backend instead.
#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// JSON Schema of the arguments.
    pub parameters: Value,
}

#[derive(Debug, Clone)]
pub struct RealtimeConfig {
    pub dialect: Dialect,
    /// Up to and including the version segment; `wss`, or `ws` only to this machine.
    pub base_url: String,
    pub model: String,
    pub api_key: ApiKey,
    pub voice: Option<String>,
    pub instructions: Option<String>,
    /// Realtime only. Without a transcription model the server sends no
    /// transcript of what the user said.
    pub transcription_model: Option<String>,
    pub tools: Vec<Tool>,
    /// Bounds the handshake, the wait for the session to start and the close.
    pub timeout: Duration,
}

impl RealtimeConfig {
    pub fn new(dialect: Dialect, model: impl Into<String>, api_key: ApiKey) -> Self {
        Self {
            dialect,
            base_url: DEFAULT_BASE_URL.to_owned(),
            model: model.into(),
            api_key,
            voice: None,
            instructions: None,
            transcription_model: None,
            tools: Vec::new(),
            timeout: Duration::from_secs(15),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), RealtimeError> {
        let named = |s: &str| !s.is_empty() && s.chars().count() <= MAX_NAME_CHARS;
        if !named(&self.model) || self.voice.as_deref().is_some_and(|v| !named(v)) {
            return Err(RealtimeError::invalid(
                "model or voice",
                "1 to 128 characters",
            ));
        }
        if self.timeout.is_zero() {
            return Err(RealtimeError::invalid("timeout", "must not be zero"));
        }
        if self.dialect == Dialect::Live
            && (!self.tools.is_empty() || self.transcription_model.is_some())
        {
            return Err(RealtimeError::invalid(
                "tools or transcription model",
                "GPT-Live takes neither; it delegates tools to a backend",
            ));
        }
        if self.api_key.expose().is_empty() {
            return Err(RealtimeError::invalid("api key", "must not be empty"));
        }
        Ok(())
    }

    pub(crate) fn socket_url(&self) -> Result<Url, RealtimeError> {
        let mut url = endpoint(
            &self.base_url,
            match self.dialect {
                Dialect::Realtime => "/realtime",
                Dialect::Live => "/live/sessions",
            },
            ("wss", "ws"),
        )?;
        if self.dialect == Dialect::Realtime {
            url.query_pairs_mut().append_pair("model", &self.model);
        }
        Ok(url)
    }
}

/// `base_url` plus `path`, on the secure scheme or the plain one to this machine
/// only: a key over an unencrypted connection to another host is readable on the path.
pub(crate) fn endpoint(
    base_url: &str,
    path: &str,
    (secure, plain): (&str, &str),
) -> Result<Url, RealtimeError> {
    let bad = || RealtimeError::invalid("base url", "https/wss, or http/ws only to localhost");
    let url =
        Url::parse(&format!("{}{path}", base_url.trim_end_matches('/'))).map_err(|_| bad())?;
    let local = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .trim_matches(['[', ']'])
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if url.scheme() == secure || (url.scheme() == plain && local) {
        Ok(url)
    } else {
        Err(bad())
    }
}

pub(crate) fn bearer(key: &ApiKey) -> Result<HeaderValue, RealtimeError> {
    let mut value = HeaderValue::from_str(&format!("Bearer {}", key.expose()))
        .map_err(|_| RealtimeError::invalid("api key", "not a valid header value"))?;
    // Keeps the token out of the HTTP libraries' own Debug output.
    value.set_sensitive(true);
    Ok(value)
}
