//! Turn a `ProvidersConfig` into clients, skipping sections that cannot be built.

use std::collections::BTreeMap;
use std::fmt;

use aulo_config::{ProviderConfig, ProviderKind, ProvidersConfig, TiersConfig};
use llm_http::Transport;
use llm_wire::Provider;
use llm_wire_anthropic::{AnthropicProvider, CacheTtl};
use llm_wire_openai::chat::OpenAiChatProvider;
use llm_wire_openai::responses::OpenAiResponsesProvider;
use url::Url;

use crate::preset::{
    self, ANTHROPIC_CONTEXT_TOKENS, DEFAULT_CONTEXT_TOKENS, DEFAULT_MAX_RETRIES,
    DEFAULT_TIMEOUT_SECS, Wire,
};

/// Why one `[providers.*]` section was left out of the set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SkipReason {
    /// `kind = "openai-compatible"` with no `base_url`.
    #[error("openai-compatible endpoint needs a base_url")]
    MissingBaseUrl,
    /// `base_url` is not an http(s) URL with a host.
    #[error("base_url must be an http(s) URL with a host")]
    BadBaseUrl,
    /// `base_url` carries a username or password. Keys are not configuration.
    #[error("base_url must not embed credentials")]
    EmbeddedCredentials,
    /// The wire constructor failed. The text is the wire error's display,
    /// which does not include a key.
    #[error("provider wire did not build: {0}")]
    Wire(String),
    /// A tier names an endpoint that is absent or was skipped.
    #[error("tier names endpoint `{endpoint}`, which was not built")]
    UnknownEndpoint {
        /// The `provider` value on the tier.
        endpoint: String,
    },
}

/// One skipped section. `section` is a dotted config path. The rendered
/// warning never includes the rejected URL, so an embedded secret stays out
/// of logs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{section}: {reason}")]
pub struct ProviderWarning {
    /// For example `providers.endpoints.custom` or `providers.tiers.main`.
    pub section: String,
    pub reason: SkipReason,
}

impl ProviderWarning {
    fn endpoint(name: &str, reason: SkipReason) -> Self {
        Self {
            section: format!("providers.endpoints.{name}"),
            reason,
        }
    }
}

/// The client built for one endpoint. The Chat wire reports
/// [`ProviderId::Local`](llm_wire::ProviderId::Local) for every compatible
/// server, including remote ones; [`kind`](EndpointProvider::kind) is the
/// configured kind.
enum WireClient {
    Chat(OpenAiChatProvider),
    Responses(OpenAiResponsesProvider),
    Anthropic(AnthropicProvider),
}

/// One endpoint that built.
pub struct EndpointProvider {
    /// Key under `providers.endpoints`.
    pub name: String,
    pub kind: ProviderKind,
    pub wire: Wire,
    /// Base URL the client will call, with no trailing slash.
    pub base_url: String,
    client: WireClient,
}

impl EndpointProvider {
    /// The shared provider trait the agent loop calls.
    #[must_use]
    pub fn as_provider(&self) -> &dyn Provider {
        match &self.client {
            WireClient::Chat(client) => client,
            WireClient::Responses(client) => client,
            WireClient::Anthropic(client) => client,
        }
    }
}

impl fmt::Debug for EndpointProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EndpointProvider")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("wire", &self.wire)
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

/// Endpoints that built, plus every section that did not.
#[derive(Debug)]
pub struct ProviderSet {
    pub endpoints: BTreeMap<String, EndpointProvider>,
    pub warnings: Vec<ProviderWarning>,
}

/// Builds a client per endpoint. A bad section appends a warning and is
/// omitted; it does not fail the build. Also warns when a tier names an
/// endpoint that is not in the result.
///
/// Clients are keyless. `api_key_env` on the transport is empty because this
/// function does not read the environment or the keychain.
#[must_use]
pub fn build_providers(config: &ProvidersConfig) -> ProviderSet {
    let mut endpoints = BTreeMap::new();
    let mut warnings = Vec::new();

    for (name, section) in &config.endpoints {
        match build_one(name, section) {
            Ok(provider) => {
                endpoints.insert(name.clone(), provider);
            }
            Err(reason) => {
                let warning = ProviderWarning::endpoint(name, reason);
                tracing::warn!(section = %warning.section, reason = %warning.reason, "skipping provider section");
                warnings.push(warning);
            }
        }
    }

    warn_tiers(&config.tiers, &endpoints, &mut warnings);
    ProviderSet {
        endpoints,
        warnings,
    }
}

fn warn_tiers(
    tiers: &TiersConfig,
    endpoints: &BTreeMap<String, EndpointProvider>,
    warnings: &mut Vec<ProviderWarning>,
) {
    for (tier, spec) in [
        ("voice", &tiers.voice),
        ("main", &tiers.main),
        ("sentinel", &tiers.sentinel),
        ("cheap", &tiers.cheap),
    ] {
        let Some(spec) = spec else { continue };
        if endpoints.contains_key(&spec.provider) {
            continue;
        }
        let warning = ProviderWarning {
            section: format!("providers.tiers.{tier}"),
            reason: SkipReason::UnknownEndpoint {
                endpoint: spec.provider.clone(),
            },
        };
        tracing::warn!(section = %warning.section, reason = %warning.reason, "skipping provider section");
        warnings.push(warning);
    }
}

fn build_one(name: &str, section: &ProviderConfig) -> Result<EndpointProvider, SkipReason> {
    let wire = preset::wire_for(section.kind);
    let base_url = resolve_base(section.kind, section.base_url.as_deref())?;
    let client =
        build_client(wire, &base_url).map_err(|error| SkipReason::Wire(error.to_string()))?;
    Ok(EndpointProvider {
        name: name.to_owned(),
        kind: section.kind,
        wire,
        base_url,
        client,
    })
}

/// Base URL the endpoint will be called at, or `None` when the section is
/// not buildable (missing custom URL, bad URL, embedded credentials).
#[must_use]
pub fn resolved_base_url(kind: ProviderKind, configured: Option<&str>) -> Option<String> {
    resolve_base(kind, configured).ok()
}

fn resolve_base(kind: ProviderKind, configured: Option<&str>) -> Result<String, SkipReason> {
    let Some(raw) = configured.map(str::trim).filter(|s| !s.is_empty()) else {
        return preset::default_base_url(kind)
            .map(str::to_owned)
            .ok_or(SkipReason::MissingBaseUrl);
    };
    let candidate = raw.trim_end_matches('/');
    let url = Url::parse(candidate).map_err(|_| SkipReason::BadBaseUrl)?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(SkipReason::BadBaseUrl);
    }
    if url.host_str().is_none() {
        return Err(SkipReason::BadBaseUrl);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(SkipReason::EmbeddedCredentials);
    }
    Ok(candidate.to_owned())
}

fn build_client(wire: Wire, base_url: &str) -> Result<WireClient, llm_wire::ProviderError> {
    let transport = Transport {
        base_url: base_url.to_owned(),
        api_key_env: String::new(),
        timeout_s: DEFAULT_TIMEOUT_SECS,
        max_retries: DEFAULT_MAX_RETRIES,
    };
    match wire {
        Wire::Chat => Ok(WireClient::Chat(OpenAiChatProvider::new(
            &transport,
            None,
            Vec::new(),
            DEFAULT_CONTEXT_TOKENS,
        )?)),
        Wire::Responses => Ok(WireClient::Responses(OpenAiResponsesProvider::new(
            &transport,
            None,
            Vec::new(),
            DEFAULT_CONTEXT_TOKENS,
        )?)),
        Wire::Anthropic => Ok(WireClient::Anthropic(AnthropicProvider::with_key(
            &transport,
            None,
            CacheTtl::default(),
            false,
            ANTHROPIC_CONTEXT_TOKENS,
        )?)),
    }
}
