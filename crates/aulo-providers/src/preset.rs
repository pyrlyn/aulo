//! Which wire a provider kind speaks, and the base URL used when the config
//! leaves `base_url` unset.

use aulo_config::ProviderKind;

/// Which shared wire crate implements this endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    /// `POST {base}/chat/completions` (OpenAI-compatible).
    Chat,
    /// `POST {base}/responses` (OpenAI's own API).
    Responses,
    /// `POST {base}/v1/messages`.
    Anthropic,
}

/// Context window passed to the Chat and Responses wires when the catalog
/// (T4.4) has not named a model yet. The wires use it as `Caps.max_context`
/// while `models` is empty.
pub const DEFAULT_CONTEXT_TOKENS: u32 = 128_000;

/// The Messages-wire fallback when the catalog has no row for the model.
/// Matches the figure `llm-wire-anthropic` documents for that case.
pub const ANTHROPIC_CONTEXT_TOKENS: u32 = 200_000;

/// Idle time between stream chunks, in seconds. Not a whole-call cap: a long
/// answer may stream for minutes as long as chunks keep arriving.
pub const DEFAULT_TIMEOUT_SECS: u32 = 120;

/// Retries after the first attempt for a retryable error before the first
/// byte. Matches `llm-http`'s `Policy` default.
pub const DEFAULT_MAX_RETRIES: u32 = 4;

/// Loopback origins probed at startup. runa and llama.cpp both use port 8080,
/// so that origin is listed once; the model list says which one answered.
/// Ollama is 11434. LM Studio is 1234, and is recognized from its native model
/// list rather than from `/v1/models`.
pub const LOCAL_PROBE_ORIGINS: [&str; 3] = [
    "http://127.0.0.1:8080",
    "http://127.0.0.1:11434",
    "http://127.0.0.1:1234",
];

/// The wire for `kind`. `openai` is Responses; `anthropic` is Messages;
/// everything else, including `openai-compatible`, is Chat Completions.
#[must_use]
pub fn wire_for(kind: ProviderKind) -> Wire {
    match kind {
        ProviderKind::Openai => Wire::Responses,
        ProviderKind::Anthropic => Wire::Anthropic,
        ProviderKind::Runa
        | ProviderKind::Ollama
        | ProviderKind::LmStudio
        | ProviderKind::LlamaCpp
        | ProviderKind::Gemini
        | ProviderKind::Xai
        | ProviderKind::Openrouter
        | ProviderKind::Deepseek
        | ProviderKind::OpenaiCompatible => Wire::Chat,
    }
}

/// Default base URL for `kind`, already including the path the wire appends
/// to (`/v1` for Chat and Responses, the origin only for Messages).
/// `openai-compatible` has none: the config must set `base_url`.
///
/// runa and llama.cpp both default to `127.0.0.1:8080`, because that is each
/// upstream's own default. A machine running both sets `base_url` on one of
/// them. vLLM is not a kind; point an `openai-compatible` endpoint at it
/// (its OpenAI server defaults to port 8000).
#[must_use]
pub fn default_base_url(kind: ProviderKind) -> Option<&'static str> {
    match kind {
        ProviderKind::Runa | ProviderKind::LlamaCpp => Some("http://127.0.0.1:8080/v1"),
        ProviderKind::Ollama => Some("http://127.0.0.1:11434/v1"),
        ProviderKind::LmStudio => Some("http://127.0.0.1:1234/v1"),
        ProviderKind::Openai => Some("https://api.openai.com/v1"),
        ProviderKind::Anthropic => Some("https://api.anthropic.com"),
        ProviderKind::Gemini => Some("https://generativelanguage.googleapis.com/v1beta/openai"),
        ProviderKind::Xai => Some("https://api.x.ai/v1"),
        ProviderKind::Openrouter => Some("https://openrouter.ai/api/v1"),
        ProviderKind::Deepseek => Some("https://api.deepseek.com/v1"),
        ProviderKind::OpenaiCompatible => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [ProviderKind; 11] = [
        ProviderKind::Runa,
        ProviderKind::Ollama,
        ProviderKind::LmStudio,
        ProviderKind::LlamaCpp,
        ProviderKind::Openai,
        ProviderKind::Anthropic,
        ProviderKind::Gemini,
        ProviderKind::Xai,
        ProviderKind::Openrouter,
        ProviderKind::Deepseek,
        ProviderKind::OpenaiCompatible,
    ];

    #[test]
    fn only_the_custom_kind_lacks_a_default_base() {
        for kind in KINDS {
            assert_eq!(
                default_base_url(kind).is_some(),
                kind != ProviderKind::OpenaiCompatible,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn local_probe_origins_are_the_preset_bases() {
        let bases = [
            default_base_url(ProviderKind::Runa),
            default_base_url(ProviderKind::LlamaCpp),
            default_base_url(ProviderKind::Ollama),
            default_base_url(ProviderKind::LmStudio),
        ];
        for base in bases {
            let base = base.unwrap_or("");
            let origin = base.strip_suffix("/v1").unwrap_or(base);
            assert!(
                LOCAL_PROBE_ORIGINS.contains(&origin),
                "{origin} is not probed"
            );
        }
        assert_eq!(LOCAL_PROBE_ORIGINS.len(), 3);
    }

    #[test]
    fn wires_match_the_three_shared_clients() {
        assert_eq!(wire_for(ProviderKind::Openai), Wire::Responses);
        assert_eq!(wire_for(ProviderKind::Anthropic), Wire::Anthropic);
        for kind in KINDS {
            if matches!(kind, ProviderKind::Openai | ProviderKind::Anthropic) {
                continue;
            }
            assert_eq!(wire_for(kind), Wire::Chat, "{kind:?}");
        }
    }
}
