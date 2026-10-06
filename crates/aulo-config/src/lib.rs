//! The one owner of aulo configuration I/O: typed sections, layered loading
//! with per-key provenance, and the committed JSON Schema.
//!
//! Nothing else in the workspace imports `figment` or `toml`.

mod error;
mod load;
mod model;
mod schema;

pub use error::ConfigError;
pub use load::{LoadOptions, Loaded, Origin, Provenance, aulo_home};
pub use model::{
    BargeIn, Config, DaemonConfig, EngineConfig, ListenMode, McpConfig, McpServer, PluginsConfig,
    PolicyConfig, ProviderConfig, ProviderKind, ProvidersConfig, TierConfig, TiersConfig,
    VoiceConfig,
};
pub use schema::schema_json;
