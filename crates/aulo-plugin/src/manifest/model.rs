//! Manifest types. They fix the file's shape (unknown keys are rejected, absent
//! lists are empty); the rules serde and JSON Schema cannot express live in
//! `validate`, so the types stay readable as the format's documentation.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::limits::{MAX_FS_ROOTS, MAX_HOSTS, MAX_ID_LEN};

/// Root of `aulo-plugin.toml`. Written by third parties, so every value is
/// untrusted until [`PluginManifest::from_toml`](super::PluginManifest::from_toml)
/// has accepted it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    /// Lowercase letters, digits and single hyphens, starting with a letter. No
    /// underscore: tools surface as `plugin__<id>__<tool>`, and `__` is the separator.
    #[schemars(
        length(min = 1, max = 32),
        regex(pattern = r"^[a-z][a-z0-9]*(-[a-z0-9]+)*$")
    )]
    pub id: String,
    /// Semantic version of the plugin (semver.org), for example `1.4.0`.
    #[schemars(with = "String")]
    pub version: semver::Version,
    /// What the plugin provides. At least one.
    #[schemars(length(min = 1))]
    pub kinds: BTreeSet<Kind>,
    /// How the host runs the plugin. Required for `tools` and the streaming
    /// kinds; skills and shell hooks need no process.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Entry")]
    pub entry: Option<Entry>,
    /// What the plugin asks for. Everything is denied unless listed here and
    /// then approved by the user.
    #[serde(default)]
    pub capabilities: Capabilities,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Tools,
    Stt,
    Tts,
    LlmProvider,
    Skills,
    Hooks,
}

impl Kind {
    /// Streaming kinds are served over gRPC (`aulo.plugin.v1`), never MCP.
    pub fn is_streaming(self) -> bool {
        matches!(self, Self::Stt | Self::Tts | Self::LlmProvider)
    }
}

/// How the host launches the plugin. Paths are relative to the plugin's own
/// directory so a package cannot point outside itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Entry {
    /// MCP server over stdio: the default ABI for tools.
    McpProcess(Process),
    /// gRPC server on a Unix socket: STT, TTS and LLM providers.
    GrpcProcess(Process),
    /// In-process WASM module for small tools and hooks.
    Wasm {
        /// Relative path of the `.wasm` file.
        module: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Process {
    /// A bare program name resolved on `PATH`, or a path relative to the plugin directory.
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// Requested capabilities. Missing means none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Capabilities {
    /// Hosts the plugin may reach: lowercase DNS names, IP addresses, or
    /// `*.example.com` for subdomains. No scheme, port or path.
    #[schemars(length(max = 64))]
    pub net: Vec<String>,
    /// Absolute directories the plugin may read.
    #[schemars(length(max = 64))]
    pub fs_read: Vec<String>,
    /// Absolute directories the plugin may write.
    #[schemars(length(max = 64))]
    pub fs_write: Vec<String>,
    /// Whether the plugin may start other programs.
    pub exec: bool,
    /// Microphone and speaker access.
    pub audio: BTreeSet<Audio>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Audio {
    Capture,
    Playback,
}

// The schema attributes above repeat these numbers as literals because the
// derive needs literals; keep them equal.
const _: () = assert!(MAX_ID_LEN == 32 && MAX_HOSTS == 64 && MAX_FS_ROOTS == 64);
