//! The one owner of aulo configuration I/O: typed sections, layered loading
//! with per-key provenance, and the committed JSON Schema.
//!
//! Nothing else in the workspace imports `figment` or `toml`.

mod choice;
mod error;
mod load;
mod model;
mod render;
mod schema;
mod secrets;

pub use error::ConfigError;
pub use load::{LoadOptions, Loaded, Origin, Provenance, aulo_home};
pub use model::{
    BargeIn, Config, DaemonConfig, EngineConfig, ListenMode, McpConfig, McpServer, PluginsConfig,
    PolicyConfig, ProviderConfig, ProviderKind, ProvidersConfig, TierConfig, TiersConfig,
    VoiceConfig,
};
pub use render::mask;
pub use schema::schema_json;
pub use secrets::{
    CredentialsError, KeyState, MemoryStore, ProviderKeyInfo, SecretStore, configured_keys,
    delete_key, key_state, resolve_provider_key, store_key, system_store, validate_provider_id,
};

/// The commented-out settings file `aulo config default` prints. Tests keep it
/// in step with [`Config`], so it cannot drift from the real defaults.
pub const DEFAULT_TOML: &str = include_str!("../config/default.toml");
