//! Config types. Every struct rejects unknown keys and fills missing ones from
//! its defaults, so a layer only has to name what it changes.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::num::NonZeroU64;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Root of `config.toml` and `.aulo.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub daemon: DaemonConfig,
    pub providers: ProvidersConfig,
    pub voice: VoiceConfig,
    pub mcp: McpConfig,
    pub plugins: PluginsConfig,
    pub policy: PolicyConfig,
}

/// `aulod` transport.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DaemonConfig {
    /// TCP address for remote clients. Unset: the owner-only local socket only.
    pub listen: Option<SocketAddr>,
    /// Certificate for `listen`. TLS is required whenever `listen` is set.
    pub tls_cert: Option<PathBuf>,
    /// Private key for `tls_cert`.
    pub tls_key: Option<PathBuf>,
}

/// Model providers and the tiers that route to them. API keys are not
/// configuration: they live in the OS keychain.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ProvidersConfig {
    /// Named provider endpoints that tiers refer to.
    pub endpoints: BTreeMap<String, ProviderConfig>,
    pub tiers: TiersConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    /// Overrides the provider's default endpoint; required for `openai-compatible`.
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    Runa,
    Ollama,
    LmStudio,
    LlamaCpp,
    Openai,
    Anthropic,
    Gemini,
    Xai,
    Openrouter,
    Deepseek,
    OpenaiCompatible,
}

/// One provider and model per tier; an unset tier is not configured.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TiersConfig {
    /// Fast short answers, or the realtime front.
    pub voice: Option<TierConfig>,
    /// Tool use.
    pub main: Option<TierConfig>,
    /// The reviewer of consequential actions.
    pub sentinel: Option<TierConfig>,
    /// Titles and summaries.
    pub cheap: Option<TierConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TierConfig {
    /// Name of an entry in `providers.endpoints`.
    pub provider: String,
    pub model: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct VoiceConfig {
    pub listen: ListenMode,
    pub barge_in: BargeIn,
    pub stt: EngineConfig,
    pub tts: EngineConfig,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ListenMode {
    #[default]
    PushToTalk,
    WakeWord,
}

/// What speaking over aulo does to the turn in progress.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BargeIn {
    /// Stop playback and cancel the turn.
    #[default]
    Interrupt,
    /// Stop playback; the turn keeps running.
    Continue,
}

/// Shape shared by `[voice.stt]` and `[voice.tts]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct EngineConfig {
    pub engine: Option<String>,
    pub model: Option<String>,
    pub voice: Option<String>,
    pub rate: Option<f32>,
    /// Engines tried in order when the chosen one fails.
    pub fallback: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct McpConfig {
    /// MCP servers aulo connects to, by name.
    pub servers: BTreeMap<String, McpServer>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "transport", rename_all = "kebab-case", deny_unknown_fields)]
pub enum McpServer {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Http {
        url: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PluginsConfig {
    /// Extra directories scanned for plugins.
    pub dirs: Vec<PathBuf>,
    /// Plugin ids that are never loaded.
    pub disabled: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PolicyConfig {
    /// Rules in the perm-rules grammar. Evaluation order is deny, allow, ask.
    pub deny: Vec<String>,
    pub allow: Vec<String>,
    pub ask: Vec<String>,
    /// An approval request nobody answers within this time is denied.
    pub approval_timeout_secs: NonZeroU64,
}

// `NonZeroU64::new` is not const-unwrappable under the no-unwrap lint, so the
// impossible zero arm falls back to the smallest valid value.
const DEFAULT_APPROVAL_TIMEOUT_SECS: NonZeroU64 = match NonZeroU64::new(60) {
    Some(n) => n,
    None => NonZeroU64::MIN,
};

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            deny: Vec::new(),
            allow: Vec::new(),
            ask: Vec::new(),
            approval_timeout_secs: DEFAULT_APPROVAL_TIMEOUT_SECS,
        }
    }
}
