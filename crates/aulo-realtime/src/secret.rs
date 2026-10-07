use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, redirect};
use serde_json::{Value, json};
use url::Url;

use crate::RealtimeError;
use crate::config::{ApiKey, bearer, endpoint};

/// Replies are a token and a session echo; more is a hostile server.
const MAX_REPLY_BYTES: usize = 64 * 1024;
const MAX_SECRET_BYTES: usize = 4096;
/// What the API accepts for the lifetime of a secret, in seconds.
const TTL_SECONDS: std::ops::RangeInclusive<u64> = 10..=7200;

/// A short-lived token a remote client uses in place of the API key. It only
/// opens sessions until it expires; the real key never leaves the daemon.
/// `Debug` hides the value.
#[derive(Debug, Clone)]
pub struct ClientSecret {
    value: ApiKey,
    /// Seconds since the Unix epoch.
    pub expires_at: u64,
}

impl ClientSecret {
    /// The token, to hand to the one remote client it was minted for.
    pub fn expose(&self) -> &str {
        self.value.expose()
    }
}

/// What a minted secret may do. The remote client can still override the
/// session when it connects, so a daemon that relies on the instructions or
/// tools set here must not let a remote client talk to the model directly.
#[derive(Debug, Clone)]
pub struct ClientSecretRequest {
    pub model: String,
    pub voice: Option<String>,
    pub instructions: Option<String>,
    /// 10 to 7200 seconds; the API's own default is 10 minutes.
    pub ttl: Duration,
}

/// Mints client secrets with the real API key (`POST /realtime/client_secrets`).
/// Realtime only: GPT-Live documents no client secrets, its browsers connect
/// over WebRTC through a server.
#[derive(Debug, Clone)]
pub struct ClientSecretMinter {
    client: Client,
    url: Url,
    key: ApiKey,
}

impl ClientSecretMinter {
    /// `base_url` is the REST one, for example [`DEFAULT_REST_URL`](crate::DEFAULT_REST_URL).
    pub fn new(key: ApiKey, base_url: &str, timeout: Duration) -> Result<Self, RealtimeError> {
        let url = endpoint(base_url, "/realtime/client_secrets", ("https", "http"))?;
        bearer(&key)?;
        let client = Client::builder()
            .user_agent(concat!("aulo-realtime/", env!("CARGO_PKG_VERSION")))
            // A redirect would resend the key somewhere the user never chose.
            .redirect(redirect::Policy::none())
            .timeout(timeout)
            .build()
            .map_err(|_| RealtimeError::Unreachable)?;
        Ok(Self { client, url, key })
    }

    pub async fn mint(&self, request: &ClientSecretRequest) -> Result<ClientSecret, RealtimeError> {
        if !TTL_SECONDS.contains(&request.ttl.as_secs()) || request.model.is_empty() {
            return Err(RealtimeError::invalid(
                "client secret",
                "ttl 10 to 7200 s and a model",
            ));
        }
        let mut session = json!({"type": "realtime", "model": request.model});
        if let Some(voice) = &request.voice {
            session["audio"] = json!({"output": {"voice": voice}});
        }
        if let Some(instructions) = &request.instructions {
            session["instructions"] = json!(instructions);
        }
        let body = json!({
            "expires_after": {"anchor": "created_at", "seconds": request.ttl.as_secs()},
            "session": session,
        });
        let mut response = self
            .client
            .post(self.url.clone())
            .header(AUTHORIZATION, bearer(&self.key)?)
            .header(CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(net_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(RealtimeError::from_status(status.as_u16()));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(net_error)? {
            if bytes.len() + chunk.len() > MAX_REPLY_BYTES {
                return Err(RealtimeError::Protocol("reply too large"));
            }
            bytes.extend_from_slice(&chunk);
        }
        parse(&bytes)
    }
}

fn parse(bytes: &[u8]) -> Result<ClientSecret, RealtimeError> {
    let reply: Value = serde_json::from_slice(bytes)
        .map_err(|_| RealtimeError::Protocol("unreadable client secret reply"))?;
    let value = reply
        .get("value")
        .and_then(Value::as_str)
        .filter(|v| {
            !v.is_empty() && v.len() <= MAX_SECRET_BYTES && v.bytes().all(|b| b.is_ascii_graphic())
        })
        .ok_or(RealtimeError::Protocol("no client secret in the reply"))?;
    let expires_at = reply
        .get("expires_at")
        .and_then(Value::as_u64)
        .ok_or(RealtimeError::Protocol("no expiry in the reply"))?;
    Ok(ClientSecret {
        value: ApiKey::new(value),
        expires_at,
    })
}

fn net_error(error: reqwest::Error) -> RealtimeError {
    if error.is_timeout() {
        RealtimeError::TimedOut
    } else {
        RealtimeError::Unreachable
    }
}
