//! Detects a local model server and lists the models it serves (T4.3, spec §10).
//!
//! [`detect_local`] probes [`LOCAL_PROBE_ORIGINS`] at startup. A server that is
//! down, or that does not answer as one of the known kinds, is skipped so a
//! missing local server never fails startup.
//!
//! Fingerprints, taken from each server's own model list:
//! - runa `GET /v1/models` sets `owned_by` to `"runa"` (`pyrlyn/runa`
//!   `crates/runa/src/serve/mod.rs`, `list_models`).
//! - llama.cpp's server sets `owned_by` to `"llamacpp"` on that same path, so
//!   port 8080 can be either program.
//! - Ollama's `/v1/models` copies the model namespace into `owned_by`, and a
//!   bare name has an empty namespace (`ollama/ollama` `openai/openai.go`,
//!   `ToListCompletion`). The stable mark is `GET /api/version`
//!   (`{"version":"<semver>"}`). Older servers wrote `owned_by: "ollama"`.
//! - LM Studio's native list is `GET /api/v0/models` (rows carry `state`,
//!   `quantization` or `max_context_length`) and `GET /api/v1/models` (`key`,
//!   `loaded_instances`). Those fields are not on the OpenAI list, and the
//!   native rows are the ones reported.
//!   <https://lmstudio.ai/docs/developer/rest/endpoints>
//!
//! The reply is untrusted. Bodies over [`MAX_BODY_BYTES`] are refused, ids are
//! capped and control characters are dropped, and nothing from the body is
//! copied into an error. No API key is sent. Redirects are not followed.

use std::time::Duration;

use aulo_config::ProviderKind;
use reqwest::redirect::Policy;
use reqwest::{Client, Response, StatusCode};
use serde_json::Value;
use url::Url;

use crate::preset::LOCAL_PROBE_ORIGINS;

/// Bytes read from one model-list response. A local catalog fits; anything
/// larger is refused rather than parsed in part.
pub const MAX_BODY_BYTES: usize = 256 * 1024;

/// Model ids kept from one server. The rest of a longer list is dropped.
pub const MAX_MODELS: usize = 256;

/// Bytes in one model id. Longer ids are dropped, not truncated, so a cut id
/// is never sent back to the server as if it were real.
pub const MAX_ID_BYTES: usize = 256;

const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);

/// Which local program answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerKind {
    /// runa on its OpenAI-compatible server.
    Runa,
    /// Ollama.
    Ollama,
    /// LM Studio, recognized from the native model list.
    LmStudio,
    /// llama.cpp's `llama-server`.
    LlamaCpp,
}

impl ServerKind {
    /// The config spelling, matching [`ProviderKind::as_str`](ProviderKind::as_str).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runa => "runa",
            Self::Ollama => "ollama",
            Self::LmStudio => "lm-studio",
            Self::LlamaCpp => "llama-cpp",
        }
    }

    /// The provider kind to record if this server is added to the config.
    #[must_use]
    pub const fn provider_kind(self) -> ProviderKind {
        match self {
            Self::Runa => ProviderKind::Runa,
            Self::Ollama => ProviderKind::Ollama,
            Self::LmStudio => ProviderKind::LmStudio,
            Self::LlamaCpp => ProviderKind::LlamaCpp,
        }
    }
}

/// Whether LM Studio currently has the model loaded. Absent when the list did
/// not say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadState {
    /// The native list says the model is loaded.
    Loaded,
    /// The native list says the model is downloaded but not loaded.
    NotLoaded,
}

/// One model a detected server says it can serve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedModel {
    /// The id to send as the chat `model`. Control characters never appear.
    pub id: String,
    /// Set from LM Studio's native list only.
    pub state: Option<LoadState>,
}

/// A server that answered as one known kind, and the models it listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedServer {
    pub kind: ServerKind,
    /// OpenAI-compatible base, including `/v1` and no trailing slash.
    pub base_url: String,
    pub models: Vec<ServedModel>,
}

/// Why one origin was not detected. The text never includes a response body
/// or a credential.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProbeError {
    /// Not an http(s) URL with a host, or it embeds a user or password, or it
    /// has a path other than `/` or `/v1`.
    #[error("origin must be an http(s) URL with a host and no credentials")]
    BadOrigin,
    /// `reqwest` refused to build the client. The underlying text is logged
    /// at debug, not shown: it can carry local paths.
    #[error("cannot build the probe client")]
    Client,
    /// Nothing accepted a connection. Normal for a preset that is not running.
    #[error("the server at {host} is not reachable")]
    Unreachable {
        /// Host and port, with no userinfo.
        host: String,
    },
    /// One reply was longer than [`MAX_BODY_BYTES`].
    #[error("the model list is larger than {MAX_BODY_BYTES} bytes")]
    TooLarge,
    /// The server spoke HTTP but not as runa, Ollama, LM Studio or llama.cpp.
    #[error("the server is not a known local model server")]
    Unrecognized,
}

/// A client that does not follow redirects and gives up quickly, so a wedged
/// port cannot stall startup.
///
/// # Errors
///
/// [`ProbeError::Client`] when the client cannot be built.
pub fn probe_client() -> Result<Client, ProbeError> {
    Client::builder()
        .user_agent(concat!("aulo-providers/", env!("CARGO_PKG_VERSION")))
        .redirect(Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| {
            tracing::debug!(
                error = %error.without_url(),
                "cannot build the local-server probe"
            );
            ProbeError::Client
        })
}

/// Probes `origin` (or an OpenAI base ending in `/v1`) and names the server.
///
/// # Errors
///
/// [`ProbeError::BadOrigin`] for a URL this probe will not call.
/// [`ProbeError::Unreachable`] when every request fails to connect.
/// [`ProbeError::TooLarge`] when a reply exceeds [`MAX_BODY_BYTES`].
/// [`ProbeError::Unrecognized`] when the server is up but not a known kind.
pub async fn probe_origin(client: &Client, origin: &str) -> Result<DetectedServer, ProbeError> {
    let origin = normalize_origin(origin)?;
    let host = host_label(&origin);
    let urls = [
        endpoint(&origin, "/v1/models"),
        endpoint(&origin, "/api/v0/models"),
        endpoint(&origin, "/api/v1/models"),
        endpoint(&origin, "/api/version"),
    ];
    let (v1, native_v0, native_v1, version) = tokio::join!(
        fetch(client, &urls[0]),
        fetch(client, &urls[1]),
        fetch(client, &urls[2]),
        fetch(client, &urls[3]),
    );
    let docs = match documents([v1, native_v0, native_v1, version]) {
        DocSet::Down => return Err(ProbeError::Unreachable { host }),
        DocSet::TooLarge => return Err(ProbeError::TooLarge),
        DocSet::Ready(docs) => docs,
    };
    let evidence = Evidence {
        v1: docs[0].as_ref(),
        native_v0: docs[1].as_ref(),
        native_v1: docs[2].as_ref(),
        version: docs[3].as_ref(),
    };
    let Some((kind, models)) = classify(evidence) else {
        return Err(ProbeError::Unrecognized);
    };
    Ok(DetectedServer {
        kind,
        base_url: openai_base(&origin),
        models,
    })
}

/// Probes each origin and keeps the ones that answered as a known server.
/// Unreachable and unrecognized origins are skipped. Order follows `origins`.
#[must_use = "the detected servers are the list to keep"]
pub async fn probe_origins(client: &Client, origins: &[&str]) -> Vec<DetectedServer> {
    let mut found = Vec::new();
    for origin in origins {
        match probe_origin(client, origin).await {
            Ok(server) => {
                tracing::info!(
                    kind = server.kind.as_str(),
                    models = server.models.len(),
                    base = %server.base_url,
                    "detected a local model server"
                );
                found.push(server);
            }
            Err(ProbeError::Unreachable { .. }) => {
                tracing::debug!("a local model server is not running");
            }
            Err(error) => {
                tracing::warn!(%error, "skipping a local model server probe");
            }
        }
    }
    found
}

/// Probes [`LOCAL_PROBE_ORIGINS`]. This is the startup check: call it when
/// the process is ready to talk to local servers, and keep whatever came back.
#[must_use = "the detected servers are the list to keep"]
pub async fn detect_local(client: &Client) -> Vec<DetectedServer> {
    probe_origins(client, &LOCAL_PROBE_ORIGINS).await
}

enum Fetch {
    Down,
    Missing,
    TooLarge,
    Doc(Value),
}

enum DocSet {
    Down,
    TooLarge,
    Ready(Box<[Option<Value>; 4]>),
}

struct Evidence<'a> {
    v1: Option<&'a Value>,
    native_v0: Option<&'a Value>,
    native_v1: Option<&'a Value>,
    version: Option<&'a Value>,
}

struct Row<'a> {
    owned_by: Option<&'a str>,
    state: Option<&'a str>,
    quantization: bool,
    max_context: bool,
}

fn documents(fetches: [Fetch; 4]) -> DocSet {
    if fetches.iter().all(|fetch| matches!(fetch, Fetch::Down)) {
        return DocSet::Down;
    }
    if fetches.iter().any(|fetch| matches!(fetch, Fetch::TooLarge)) {
        return DocSet::TooLarge;
    }
    DocSet::Ready(Box::new(fetches.map(|fetch| match fetch {
        Fetch::Doc(value) => Some(value),
        Fetch::Down | Fetch::Missing | Fetch::TooLarge => None,
    })))
}

fn classify(evidence: Evidence<'_>) -> Option<(ServerKind, Vec<ServedModel>)> {
    if let Some(doc) = evidence.v1 {
        let rows = data_rows(doc);
        if unanimous(&rows, "runa") {
            return Some((ServerKind::Runa, models_from_data(doc)));
        }
        if unanimous(&rows, "llamacpp") {
            return Some((ServerKind::LlamaCpp, models_from_data(doc)));
        }
        if unanimous(&rows, "ollama") {
            return Some((ServerKind::Ollama, models_from_data(doc)));
        }
    }
    if let Some(doc) = evidence.native_v0 {
        let rows = data_rows(doc);
        if rows.iter().any(is_native) {
            return Some((ServerKind::LmStudio, models_from_data(doc)));
        }
    }
    if let Some(doc) = evidence.native_v1
        && let Some(models) = lmstudio_v1(doc)
    {
        return Some((ServerKind::LmStudio, models));
    }
    if evidence.version.is_some_and(ollama_version)
        && let Some(doc) = evidence
            .v1
            .filter(|doc| doc.get("data").is_some_and(Value::is_array))
    {
        return Some((ServerKind::Ollama, models_from_data(doc)));
    }
    None
}

fn unanimous(rows: &[Row<'_>], owner: &str) -> bool {
    let mut saw = false;
    for row in rows {
        match row.owned_by {
            Some(value) if value == owner => saw = true,
            Some(value) if !value.is_empty() => return false,
            _ => {}
        }
    }
    saw
}

fn is_native(row: &Row<'_>) -> bool {
    row.state.is_some() || row.quantization || row.max_context
}

fn data_rows(doc: &Value) -> Vec<Row<'_>> {
    doc.get("data")
        .and_then(Value::as_array)
        .map(|data| data.iter().map(row_of).collect())
        .unwrap_or_default()
}

fn row_of(value: &Value) -> Row<'_> {
    Row {
        owned_by: value.get("owned_by").and_then(Value::as_str),
        state: value.get("state").and_then(Value::as_str),
        quantization: present(value, "quantization"),
        max_context: present(value, "max_context_length"),
    }
}

fn present(value: &Value, key: &str) -> bool {
    value.get(key).is_some_and(|item| !item.is_null())
}

fn models_from_data(doc: &Value) -> Vec<ServedModel> {
    let Some(data) = doc.get("data").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for value in data {
        let Some(id) = value.get("id").and_then(Value::as_str).and_then(clean_id) else {
            continue;
        };
        push_model(
            &mut out,
            ServedModel {
                id,
                state: load_state(value.get("state").and_then(Value::as_str)),
            },
        );
    }
    out
}

/// LM Studio's v1 list uses `models[].key`, not the OpenAI `data[].id`.
fn lmstudio_v1(doc: &Value) -> Option<Vec<ServedModel>> {
    let models = doc.get("models")?.as_array()?;
    let mut out = Vec::new();
    let mut native = false;
    for model in models {
        if present(model, "quantization")
            || present(model, "loaded_instances")
            || present(model, "max_context_length")
        {
            native = true;
        }
        let Some(id) = model.get("key").and_then(Value::as_str).and_then(clean_id) else {
            continue;
        };
        let state = model
            .get("loaded_instances")
            .and_then(Value::as_array)
            .map(|loaded| {
                if loaded.is_empty() {
                    LoadState::NotLoaded
                } else {
                    LoadState::Loaded
                }
            });
        push_model(&mut out, ServedModel { id, state });
    }
    native.then_some(out)
}

fn ollama_version(doc: &Value) -> bool {
    if doc.get("data").is_some() || doc.get("models").is_some() {
        return false;
    }
    let Some(version) = doc.get("version").and_then(Value::as_str) else {
        return false;
    };
    // `0.11.4` and `0.5.1`. A leading `v` is not what Ollama writes.
    let mut parts = version.split('.');
    let major = parts
        .next()
        .is_some_and(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    let minor = parts
        .next()
        .is_some_and(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    major
        && minor
        && version.len() <= 32
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}

fn load_state(raw: Option<&str>) -> Option<LoadState> {
    match raw? {
        "loaded" => Some(LoadState::Loaded),
        "not-loaded" => Some(LoadState::NotLoaded),
        _ => None,
    }
}

fn clean_id(raw: &str) -> Option<String> {
    let id = raw.trim();
    if id.is_empty() || id.len() > MAX_ID_BYTES {
        return None;
    }
    if id
        .chars()
        .any(|c| c.is_control() || c == '\u{7f}' || c.is_whitespace())
    {
        return None;
    }
    Some(id.to_owned())
}

fn push_model(out: &mut Vec<ServedModel>, model: ServedModel) {
    if out.len() >= MAX_MODELS || out.iter().any(|have| have.id == model.id) {
        return;
    }
    out.push(model);
}

async fn fetch(client: &Client, url: &str) -> Fetch {
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::debug!(error = %error.without_url(), "local model server probe failed");
            return Fetch::Down;
        }
    };
    if response.status() == StatusCode::NOT_FOUND || !response.status().is_success() {
        return Fetch::Missing;
    }
    match read_capped(response).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map_or(Fetch::Missing, Fetch::Doc),
        Err(ReadFail::TooLarge) => Fetch::TooLarge,
        Err(ReadFail::Io) => Fetch::Missing,
    }
}

enum ReadFail {
    TooLarge,
    Io,
}

async fn read_capped(mut response: Response) -> Result<Vec<u8>, ReadFail> {
    let mut body = Vec::new();
    loop {
        let chunk = match response.chunk().await {
            Ok(chunk) => chunk,
            Err(error) => {
                tracing::debug!(error = %error.without_url(), "model list ended early");
                return Err(ReadFail::Io);
            }
        };
        let Some(chunk) = chunk else {
            return Ok(body);
        };
        if body.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
            return Err(ReadFail::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
}

fn normalize_origin(raw: &str) -> Result<Url, ProbeError> {
    let raw = raw.trim().trim_end_matches('/');
    let mut url = Url::parse(raw).map_err(|_| ProbeError::BadOrigin)?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(ProbeError::BadOrigin);
    }
    if url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return Err(ProbeError::BadOrigin);
    }
    let path = url.path().trim_end_matches('/');
    if !path.is_empty() && path != "/" && path != "/v1" {
        return Err(ProbeError::BadOrigin);
    }
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn host_label(url: &Url) -> String {
    let Some(host) = url.host_str() else {
        return String::new();
    };
    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    match url.port_or_known_default() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    }
}

fn endpoint(origin: &Url, path: &str) -> String {
    let mut url = origin.clone();
    url.set_path(path);
    url.to_string()
}

fn openai_base(origin: &Url) -> String {
    let mut url = origin.clone();
    url.set_path("/v1");
    let text = url.to_string();
    text.trim_end_matches('/').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn evidence<'a>(
        v1: Option<&'a Value>,
        native_v0: Option<&'a Value>,
        native_v1: Option<&'a Value>,
        version: Option<&'a Value>,
    ) -> Evidence<'a> {
        Evidence {
            v1,
            native_v0,
            native_v1,
            version,
        }
    }

    #[test]
    fn ids_reject_control_characters_and_overlong_text() {
        assert_eq!(
            clean_id("  llama3.2:latest ").as_deref(),
            Some("llama3.2:latest")
        );
        assert!(clean_id("bad\nid").is_none());
        assert!(clean_id("has space").is_none());
        assert!(clean_id(&"m".repeat(MAX_ID_BYTES + 1)).is_none());
        assert_eq!(
            clean_id(&"m".repeat(MAX_ID_BYTES)).unwrap().len(),
            MAX_ID_BYTES
        );
    }

    #[test]
    fn a_mixed_owner_list_is_not_guessed() {
        let doc = json!({"object": "list", "data": [
            {"id": "a", "object": "model", "owned_by": "runa"},
            {"id": "b", "object": "model", "owned_by": "llamacpp"}
        ]});
        assert!(classify(evidence(Some(&doc), None, None, None)).is_none());
    }

    #[test]
    fn the_native_list_wins_over_a_generic_openai_list() {
        let v1 = json!({"object": "list", "data": [
            {"id": "openai-shaped", "object": "model", "owned_by": "organization"}
        ]});
        let native = json!({"object": "list", "data": [
            {"id": "native-model", "object": "model", "state": "loaded", "quantization": "Q4_K_M", "max_context_length": 8192}
        ]});
        let (kind, models) = classify(evidence(Some(&v1), Some(&native), None, None)).unwrap();
        assert_eq!(kind, ServerKind::LmStudio);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "native-model");
        assert_eq!(models[0].state, Some(LoadState::Loaded));
    }

    #[test]
    fn an_openai_list_with_native_fields_is_not_lm_studio() {
        let v1 = json!({"object": "list", "data": [
            {"id": "m", "owned_by": "someone", "state": "loaded", "quantization": "Q4"}
        ]});
        assert!(classify(evidence(Some(&v1), None, None, None)).is_none());
    }
}
