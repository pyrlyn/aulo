//! What a local server says it is serving, from `GET {base}/v1/models`.
//!
//! Hosted providers are not probed: their list needs a key, and the catalog
//! already names their models. Kind detection (runa versus Ollama versus
//! LM Studio's native list) is T4.3; every local preset speaks this
//! OpenAI-compatible list.

use std::collections::BTreeMap;
use std::time::Duration;

use aulo_config::{ProviderKind, ProvidersConfig};
use serde::Deserialize;
use url::Host;

use crate::resolved_base_url;

/// A probe that found nothing, or was never asked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServedModels {
    by_provider: BTreeMap<String, Vec<String>>,
}

impl ServedModels {
    /// Ids the probe kept for `provider`, in the order the server sent them.
    #[must_use]
    pub fn get(&self, provider: &str) -> &[String] {
        self.by_provider
            .get(provider)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Records a probe result. An empty list is kept so a later read can
    /// tell "probed, nothing served" from "not probed", though listing
    /// treats both as no overlay.
    pub fn insert(&mut self, provider: impl Into<String>, models: Vec<String>) {
        self.by_provider.insert(provider.into(), models);
    }
}

const CONNECT_TIMEOUT: Duration = Duration::from_millis(300);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
/// A model list is a short JSON document. Anything larger is dropped whole
/// rather than parsed, so a hostile server cannot fill memory.
const MAX_BODY_BYTES: usize = 256 * 1024;

/// Probes every local endpoint. A down server, a bad body or a client that
/// will not build is an empty list for that endpoint, not a startup failure.
pub async fn probe_served(config: &ProvidersConfig) -> ServedModels {
    let targets = probe_targets(config);
    if targets.is_empty() {
        return ServedModels::default();
    }
    let Some(client) = probe_client() else {
        return ServedModels::default();
    };
    let futs = targets.into_iter().map(|(id, base)| {
        let client = client.clone();
        async move {
            let models = probe_models(&client, &base).await;
            (id, models)
        }
    });
    let pairs = futures_util::future::join_all(futs).await;
    let mut served = ServedModels::default();
    for (id, models) in pairs {
        served.insert(id, models);
    }
    served
}

fn probe_targets(config: &ProvidersConfig) -> Vec<(String, String)> {
    config
        .endpoints
        .iter()
        .filter_map(|(id, section)| {
            let base = resolved_base_url(section.kind, section.base_url.as_deref())?;
            if should_probe(section.kind, &base) {
                Some((id.clone(), base))
            } else {
                None
            }
        })
        .collect()
}

fn should_probe(kind: ProviderKind, base: &str) -> bool {
    match kind {
        ProviderKind::Runa
        | ProviderKind::Ollama
        | ProviderKind::LmStudio
        | ProviderKind::LlamaCpp => true,
        // A custom cloud URL is not asked for its models without a key.
        // Loopback is the local-server case (vLLM on port 8000).
        ProviderKind::OpenaiCompatible => host_is_local(base),
        ProviderKind::Openai
        | ProviderKind::Anthropic
        | ProviderKind::Gemini
        | ProviderKind::Xai
        | ProviderKind::Openrouter
        | ProviderKind::Deepseek => false,
    }
}

fn host_is_local(base: &str) -> bool {
    let Ok(url) = url::Url::parse(base) else {
        return false;
    };
    match url.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        _ => false,
    }
}

fn probe_client() -> Option<reqwest::Client> {
    match reqwest::Client::builder()
        .user_agent(concat!("aulo/", env!("CARGO_PKG_VERSION")))
        // The probe builds the URL itself and must not be sent elsewhere.
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
    {
        Ok(client) => Some(client),
        Err(error) => {
            tracing::warn!(error = %error, "cannot build the model-list client");
            None
        }
    }
}

#[derive(Debug, Deserialize)]
struct ModelsBody {
    #[serde(default)]
    data: Vec<ModelItem>,
}

#[derive(Debug, Deserialize)]
struct ModelItem {
    #[serde(default)]
    id: String,
}

async fn probe_models(client: &reqwest::Client, base: &str) -> Vec<String> {
    let Some(url) = models_url(base) else {
        return Vec::new();
    };
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(error) => {
            // `without_url` drops the endpoint, which is the user's config.
            tracing::warn!(error = %error.without_url(), "model list probe failed");
            return Vec::new();
        }
    };
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "model list probe returned an error status");
        return Vec::new();
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_BODY_BYTES as u64)
    {
        tracing::warn!("model list probe body is too large");
        return Vec::new();
    }
    let body = match response.bytes().await {
        Ok(body) => body,
        Err(error) => {
            tracing::warn!(error = %error.without_url(), "model list probe body failed");
            return Vec::new();
        }
    };
    if body.len() > MAX_BODY_BYTES {
        tracing::warn!(bytes = body.len(), "model list probe body is too large");
        return Vec::new();
    }
    let parsed: ModelsBody = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(error) => {
            tracing::warn!(error = %error, "model list probe body is not a model list");
            return Vec::new();
        }
    };
    let mut ids = Vec::new();
    for item in parsed.data {
        if item.id.is_empty()
            || item.id.len() > 128
            || item.id.chars().any(char::is_control)
            || ids.iter().any(|seen: &String| seen == &item.id)
        {
            continue;
        }
        ids.push(item.id);
    }
    ids
}

/// `{base}` already ends at `/v1`. Joining `"models"` as a relative URL
/// would replace that segment, so the path is extended instead.
fn models_url(base: &str) -> Option<String> {
    let mut url = url::Url::parse(base).ok()?;
    url.path_segments_mut().ok()?.push("models");
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aulo_config::{ProviderConfig, ProviderKind, ProvidersConfig};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn the_models_path_stays_under_v1() {
        assert_eq!(
            models_url("http://127.0.0.1:11434/v1").as_deref(),
            Some("http://127.0.0.1:11434/v1/models")
        );
    }

    #[tokio::test]
    async fn a_local_server_is_probed_and_a_hosted_one_is_not() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "object": "list",
            "data": [
                {"id": "llama3.2", "object": "model"},
                {"id": ""},
                {"id": "a".repeat(200)},
                {"id": "bad\nid"},
                {"id": "llama3.2"},
                {"id": "extra"}
            ]
        });
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;

        let base = format!("{}/v1", server.uri());
        let mut providers = ProvidersConfig::default();
        providers.endpoints.insert(
            "ollama".into(),
            ProviderConfig {
                kind: ProviderKind::Ollama,
                base_url: Some(base.clone()),
            },
        );
        providers.endpoints.insert(
            "openai".into(),
            ProviderConfig {
                kind: ProviderKind::Openai,
                base_url: Some(base),
            },
        );

        let served = probe_served(&providers).await;
        assert_eq!(
            served.get("ollama"),
            ["llama3.2".to_owned(), "extra".to_owned()]
        );
        assert!(served.get("openai").is_empty());
        server.verify().await;
    }

    #[tokio::test]
    async fn a_failed_probe_is_an_empty_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let mut providers = ProvidersConfig::default();
        providers.endpoints.insert(
            "ollama".into(),
            ProviderConfig {
                kind: ProviderKind::Ollama,
                base_url: Some(format!("{}/v1", server.uri())),
            },
        );
        let served = probe_served(&providers).await;
        assert!(served.get("ollama").is_empty());
    }
}
