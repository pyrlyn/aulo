//! What picks an engine: one validated layer of choice (`[voice.stt]` or
//! `[voice.tts]`, a bot override, a chat override) and the resolved spec a
//! factory builds from. They live in the contract so config, storage, the
//! gRPC layer and engine crates share them without depending on the registry.

use crate::{EngineId, SpeechRate};

/// Everything a factory needs to build one engine. Rate is ignored by STT.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineSpec {
    pub engine: EngineId,
    /// Engine-specific model name; `None` takes the engine's default.
    pub model: Option<String>,
    /// A [`crate::Voice::id`]; `None` takes the engine's default.
    pub voice: Option<String>,
    pub rate: SpeechRate,
}

/// One layer of engine choice. Unset fields defer to the layer below, so an
/// override only names what it changes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EngineChoice {
    pub engine: Option<EngineId>,
    pub model: Option<String>,
    pub voice: Option<String>,
    pub rate: Option<SpeechRate>,
    /// Engines tried in order when the chosen ones fail.
    pub fallback: Vec<EngineId>,
}
