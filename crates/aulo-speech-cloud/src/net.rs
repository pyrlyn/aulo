//! What every cloud engine shares: endpoint validation, the bearer header, the
//! HTTP client rules and the mapping from failures to fallback categories.

use std::net::IpAddr;
use std::time::Duration;

use aulo_speech::SpeechError;
use reqwest::header::HeaderValue;
use reqwest::{Client, StatusCode, Url, redirect};

use crate::config::ApiKey;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// A validated endpoint and whether it is on this machine.
pub(crate) struct Endpoint {
    pub(crate) url: Url,
    pub(crate) local: bool,
}

/// `base_url` goes up to and including the version segment.
pub(crate) fn endpoint(base_url: &str, path: &str, has_key: bool) -> Result<Endpoint, SpeechError> {
    let url = Url::parse(&format!("{}{path}", base_url.trim_end_matches('/')))
        .map_err(|_| SpeechError::invalid("base url", "not a valid URL"))?;
    let local = is_loopback(&url);
    // A bearer token over plain http to another host is readable on the path.
    let secure = match url.scheme() {
        "https" => true,
        "http" => local || !has_key,
        _ => false,
    };
    if !secure {
        return Err(SpeechError::invalid(
            "base url",
            "https, or http only to localhost",
        ));
    }
    Ok(Endpoint { url, local })
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

pub(crate) fn bearer(key: Option<&ApiKey>) -> Result<Option<HeaderValue>, SpeechError> {
    key.map(|key| HeaderValue::from_str(&format!("Bearer {}", key.expose())))
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
