//! Catalog rows plus the served overlay, one list per configured endpoint.
//!
//! `llm-catalog` stores a model id and not the provider that serves it: the
//! rows were flattened out of the coding agent's per-provider lists. The id itself puts a
//! built-in row back under a provider kind (`claude-*` is Anthropic, a
//! slash id is a gateway id). A row with no such hint, such as `qwen3-coder`,
//! stays hidden until a tier names it or a server reports it.

use std::collections::BTreeMap;

use aulo_config::{ProviderKind, ProvidersConfig, TierConfig};
use llm_catalog::{Catalog, ModelRow};

use crate::served::ServedModels;

/// One model `ConfigService.ListModels` can return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedModel {
    /// Endpoint name under `providers.endpoints`.
    pub provider_id: String,
    /// Model id sent on the wire.
    pub model_id: String,
    /// Catalog name, or the id when the catalog has no row. At most 128 bytes.
    pub display_name: String,
    /// The endpoint's server reported this id on its model list.
    pub served: bool,
}

/// Built-in catalog, loaded once. A parse failure becomes an empty catalog
/// so listing still returns tier and served models.
#[derive(Debug, Clone)]
pub struct ModelCatalog {
    inner: Catalog,
}

impl ModelCatalog {
    /// The embedded catalog. Logs and continues with no built-in rows when
    /// the embedded table cannot be parsed.
    #[must_use]
    pub fn load() -> Self {
        match Catalog::builtin() {
            Ok(inner) => Self { inner },
            Err(error) => {
                tracing::error!(error = %error, "built-in model catalog failed to parse");
                Self {
                    inner: Catalog::default(),
                }
            }
        }
    }

    /// Models for every configured endpoint: catalog rows for that kind,
    /// models named by a tier, then the served overlay (which wins the
    /// `served` flag and adds ids the catalog does not know).
    #[must_use]
    pub fn list(&self, providers: &ProvidersConfig, served: &ServedModels) -> Vec<ListedModel> {
        let mut listed = Vec::new();
        for (id, section) in &providers.endpoints {
            listed.extend(self.endpoint(
                id,
                section.kind,
                tier_models(&providers.tiers, id),
                served.get(id),
            ));
        }
        listed.sort_by(|a, b| (&a.provider_id, &a.model_id).cmp(&(&b.provider_id, &b.model_id)));
        listed
    }

    fn endpoint(
        &self,
        provider_id: &str,
        kind: ProviderKind,
        tiers: Vec<&str>,
        served: &[String],
    ) -> Vec<ListedModel> {
        // `true` means the server reported the id. Catalog and tier rows
        // start false; the overlay overwrites that.
        let mut ids: BTreeMap<String, bool> = BTreeMap::new();
        for row in self.inner.rows() {
            if catalog_kinds(&row.id).contains(&kind) && acceptable_id(&row.id) {
                ids.entry(row.id.clone()).or_insert(false);
            }
        }
        for model in tiers {
            if acceptable_id(model) {
                ids.entry(model.to_owned()).or_insert(false);
            }
        }
        for id in served {
            if acceptable_id(id) {
                ids.insert(id.clone(), true);
            }
        }
        ids.into_iter()
            .map(|(model_id, served)| ListedModel {
                provider_id: provider_id.to_owned(),
                display_name: display_name(self.inner.get(&model_id), &model_id),
                model_id,
                served,
            })
            .collect()
    }
}

fn tier_models<'a>(tiers: &'a aulo_config::TiersConfig, provider_id: &str) -> Vec<&'a str> {
    [&tiers.voice, &tiers.main, &tiers.sentinel, &tiers.cheap]
        .into_iter()
        .filter_map(|tier| tier.as_ref())
        .filter(|tier| tier.provider == provider_id)
        .map(model_of)
        .collect()
}

fn model_of(tier: &TierConfig) -> &str {
    tier.model.as_str()
}

/// Proto `Model.display_name` is at most 128 bytes. The catalog names are
/// trusted; a served id used as a name is not, so both go through the cap.
const MAX_DISPLAY_BYTES: usize = 128;
const MAX_MODEL_BYTES: usize = 128;

fn acceptable_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_MODEL_BYTES && !id.chars().any(char::is_control)
}

fn display_name(row: Option<&ModelRow>, id: &str) -> String {
    let name = row
        .and_then(|row| row.display_name.as_deref())
        .unwrap_or(id);
    cap_bytes(name, MAX_DISPLAY_BYTES)
}

fn cap_bytes(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// Provider kinds a built-in id belongs to. Empty means the id has no kind
/// of its own and appears only when a tier or a server names it.
fn catalog_kinds(id: &str) -> &'static [ProviderKind] {
    const ANTHROPIC: &[ProviderKind] = &[ProviderKind::Anthropic];
    const OPENAI: &[ProviderKind] = &[ProviderKind::Openai];
    const GEMINI: &[ProviderKind] = &[ProviderKind::Gemini];
    const DEEPSEEK: &[ProviderKind] = &[ProviderKind::Deepseek];
    const XAI: &[ProviderKind] = &[ProviderKind::Xai];
    const OPENROUTER: &[ProviderKind] = &[ProviderKind::Openrouter];
    // `x-ai/grok-4.3` is the gateway id and the only grok row in the catalog,
    // so the xAI endpoint lists it too.
    const XAI_AND_ROUTER: &[ProviderKind] = &[ProviderKind::Xai, ProviderKind::Openrouter];

    if let Some((vendor, _)) = id.split_once('/') {
        return match vendor {
            "x-ai" => XAI_AND_ROUTER,
            _ => OPENROUTER,
        };
    }
    if id.starts_with("claude-") {
        ANTHROPIC
    } else if id.starts_with("gpt-") {
        OPENAI
    } else if id.starts_with("gemini-") {
        GEMINI
    } else if id.starts_with("deepseek-") {
        DEEPSEEK
    } else if id.starts_with("grok-") {
        XAI
    } else {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aulo_config::{ProviderConfig, ProviderKind, ProvidersConfig, TierConfig};

    fn config() -> ProvidersConfig {
        let mut providers = ProvidersConfig::default();
        providers.endpoints.insert(
            "anthropic".into(),
            ProviderConfig {
                kind: ProviderKind::Anthropic,
                base_url: None,
            },
        );
        providers.endpoints.insert(
            "ollama".into(),
            ProviderConfig {
                kind: ProviderKind::Ollama,
                base_url: None,
            },
        );
        providers.endpoints.insert(
            "openrouter".into(),
            ProviderConfig {
                kind: ProviderKind::Openrouter,
                base_url: None,
            },
        );
        providers.tiers.main = Some(TierConfig {
            provider: "ollama".into(),
            model: "llama3.2".into(),
        });
        providers
    }

    fn served() -> ServedModels {
        let mut served = ServedModels::default();
        served.insert(
            "ollama",
            vec![
                "llama3.2".into(),
                "claude-sonnet-5".into(),
                String::new(),
                "bad\nid".into(),
                "x".repeat(200),
            ],
        );
        served
    }

    #[test]
    fn catalog_rows_merge_with_the_served_overlay() {
        let listed = ModelCatalog::load().list(&config(), &served());

        let sonnet = listed
            .iter()
            .find(|row| row.provider_id == "anthropic" && row.model_id == "claude-sonnet-5")
            .expect("catalog lists claude-sonnet-5 under anthropic");
        assert_eq!(sonnet.display_name, "Claude Sonnet 5");
        assert!(!sonnet.served);
        assert!(
            listed
                .iter()
                .filter(|row| row.provider_id == "anthropic")
                .all(|row| row.model_id.starts_with("claude-") && !row.served)
        );

        let local = listed
            .iter()
            .find(|row| row.provider_id == "ollama" && row.model_id == "llama3.2")
            .expect("the tier model is listed");
        assert!(local.served);
        assert_eq!(local.display_name, "llama3.2");

        let overlay = listed
            .iter()
            .find(|row| row.provider_id == "ollama" && row.model_id == "claude-sonnet-5")
            .expect("a served id keeps the catalog display name");
        assert!(overlay.served);
        assert_eq!(overlay.display_name, "Claude Sonnet 5");

        assert!(
            listed
                .iter()
                .all(|row| !row.model_id.is_empty() && !row.model_id.chars().any(char::is_control))
        );
        assert!(listed.iter().all(|row| row.model_id.len() <= 128));
        assert!(
            !listed.iter().any(|row| row.model_id == "qwen3-coder"),
            "an unattributed catalog row stays hidden until a tier or a server names it"
        );
    }

    #[test]
    fn a_gateway_id_is_listed_for_openrouter_and_xai() {
        let mut providers = ProvidersConfig::default();
        for (id, kind) in [
            ("openrouter", ProviderKind::Openrouter),
            ("xai", ProviderKind::Xai),
            ("openai", ProviderKind::Openai),
        ] {
            providers.endpoints.insert(
                id.into(),
                ProviderConfig {
                    kind,
                    base_url: None,
                },
            );
        }
        let listed = ModelCatalog::load().list(&providers, &ServedModels::default());
        assert!(
            listed
                .iter()
                .any(|row| row.provider_id == "openrouter" && row.model_id == "x-ai/grok-4.3")
        );
        assert!(
            listed
                .iter()
                .any(|row| row.provider_id == "xai" && row.model_id == "x-ai/grok-4.3")
        );
        assert!(
            !listed
                .iter()
                .any(|row| row.provider_id == "openai" && row.model_id.contains('/'))
        );
    }
}
