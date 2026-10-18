//! Assembles [`llm_wire::Provider`] clients from `[providers.*]` (spec §10, T4.1).
//!
//! One entry under `providers.endpoints` becomes one client:
//! - `openai` speaks the Responses API (`POST /v1/responses`), which is what
//!   OpenAI's own models use;
//! - `anthropic` speaks the Messages API;
//! - every other kind speaks Chat Completions (`POST /v1/chat/completions`).
//!   That one wire covers Ollama, LM Studio, llama.cpp, vLLM, OpenRouter,
//!   DeepSeek, Gemini's compatibility endpoint, xAI, runa and a custom
//!   `openai-compatible` base URL. There is no per-vendor client.
//!
//! A section that cannot be built is skipped and reported in
//! [`ProviderSet::warnings`]. The remaining endpoints are still returned.
//! Credentials are not resolved here: every client is built without a key
//! (T4.2 attaches the keychain and env-var lookup). A missing key is not a
//! broken section.

mod build;
mod preset;

pub use build::{ProviderSet, ProviderWarning, SkipReason, build_providers};
pub use llm_wire::{Provider, ProviderId};
pub use preset::{
    ANTHROPIC_CONTEXT_TOKENS, DEFAULT_CONTEXT_TOKENS, DEFAULT_MAX_RETRIES, DEFAULT_TIMEOUT_SECS,
    Wire, default_base_url, wire_for,
};
