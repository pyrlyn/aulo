//! What every cloud engine shares: endpoint validation, the bearer header, the
//! HTTP client rules and the mapping from failures to fallback categories.

use std::time::Duration;

use aulo_speech::SpeechError;
use reqwest::header::HeaderValue;
use reqwest::{Client, StatusCode, redirect};

use aulo_types::{ApiKey, checked_endpoint};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) use aulo_types::Endpoint;

/// `base_url` goes up to and including the version segment.
pub(crate) fn endpoint(base_url: &str, path: &str, has_key: bool) -> Result<Endpoint, SpeechError> {
    checked_endpoint(base_url, path, has_key, ("https", "http"))
        .ok_or_else(|| SpeechError::invalid("base url", "https, or http only to localhost"))
}

/// The WebSocket form of [`endpoint`]: `wss`, or `ws` only to this machine.
pub(crate) fn ws_endpoint(
    base_url: &str,
    path: &str,
    has_key: bool,
) -> Result<Endpoint, SpeechError> {
    checked_endpoint(base_url, path, has_key, ("wss", "ws"))
        .ok_or_else(|| SpeechError::invalid("base url", "wss, or ws only to localhost"))
}

pub(crate) fn bearer(key: Option<&ApiKey>) -> Result<Option<HeaderValue>, SpeechError> {
    secret_header(key, "Bearer ")
}

/// The key as a header value, after `prefix` (empty for a bare key header).
pub(crate) fn secret_header(
    key: Option<&ApiKey>,
    prefix: &str,
) -> Result<Option<HeaderValue>, SpeechError> {
    key.map(|key| HeaderValue::from_str(&format!("{prefix}{}", key.expose())))
        .transpose()
        .map_err(|_| SpeechError::invalid("api key", "not a valid header value"))
        .map(|value| {
            value.map(|mut value| {
                // Keeps the token out of reqwest's own Debug output.
                value.set_sensitive(true);
                value
            })
        })
}

/// The caller adds its own timeout: STT bounds the whole request, TTS only
/// the silence between chunks, because audio is read at playback speed.
pub(crate) fn client(timeout: Option<Duration>) -> Result<Client, SpeechError> {
    let mut builder = Client::builder()
        .user_agent(concat!("aulo-speech-cloud/", env!("CARGO_PKG_VERSION")))
        // A redirect would resend the payload, and the key, somewhere the user never chose.
        .redirect(redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT);
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    builder
        .build()
        .map_err(|e| SpeechError::unavailable(&e.without_url().to_string()))
}

/// `noun` names the service in messages, for example "transcription".
pub(crate) fn net_error(noun: &str, error: reqwest::Error) -> SpeechError {
    // The URL is the user's configured endpoint, but it may carry credentials.
    let error = error.without_url();
    if error.is_connect() {
        SpeechError::unavailable(&format!("cannot reach the {noun} server"))
    } else if error.is_timeout() {
        timed_out(noun)
    } else {
        SpeechError::failed(&format!("{noun} request failed: {error}"))
    }
}

pub(crate) fn timed_out(noun: &str) -> SpeechError {
    SpeechError::failed(&format!("the {noun} request timed out"))
}

/// Only the status is reported: the reply body is untrusted and 401s quote the key.
pub(crate) fn status_error(status: StatusCode) -> SpeechError {
    let code = status.as_u16();
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => SpeechError::unavailable(&format!(
            "the server rejected the API key (HTTP {code}); check the key"
        )),
        StatusCode::TOO_MANY_REQUESTS => SpeechError::unavailable("rate limited (HTTP 429)"),
        _ => SpeechError::failed(&format!("the server refused the request (HTTP {code})")),
    }
}
