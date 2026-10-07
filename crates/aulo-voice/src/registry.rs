use std::collections::BTreeMap;
use std::fmt;

use aulo_speech::{EngineId, EngineSpec, SpeechError, SttEngine, TtsEngine};

/// Builds one engine from a resolved spec. Shared and `Sync` so one registry
/// serves every bot and chat; a plugin factory talks to its plugin process.
pub type Factory<E> = Box<dyn Fn(&EngineSpec) -> Result<Box<E>, SpeechError> + Send + Sync>;

/// The engine traits a registry can hold. The kind name keeps notices and
/// errors readable without building an engine to ask it.
pub trait Engine {
    const KIND: &'static str;
}

impl Engine for dyn SttEngine {
    const KIND: &'static str = "stt";
}

impl Engine for dyn TtsEngine {
    const KIND: &'static str = "tts";
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RegistryError {
    /// Refused so a plugin cannot silently take over a built-in engine's id.
    #[error("{kind} engine `{id}` is already registered")]
    Duplicate { kind: &'static str, id: EngineId },
    /// Every candidate failed to build or start, or none was configured.
    #[error("no {kind} engine could start")]
    NoEngine { kind: &'static str },
    /// An error fallback would not fix (a caller bug, a full queue). The
    /// engine stays active.
    #[error(transparent)]
    Speech(#[from] SpeechError),
}

/// Engine factories of one kind, by id.
pub struct EngineRegistry<E: ?Sized> {
    factories: BTreeMap<EngineId, Factory<E>>,
}

pub type SttRegistry = EngineRegistry<dyn SttEngine>;
pub type TtsRegistry = EngineRegistry<dyn TtsEngine>;

impl<E: ?Sized + Engine> EngineRegistry<E> {
    pub fn new() -> Self {
        Self {
            factories: BTreeMap::new(),
        }
    }

    pub fn register(
        &mut self,
        id: EngineId,
        factory: impl Fn(&EngineSpec) -> Result<Box<E>, SpeechError> + Send + Sync + 'static,
    ) -> Result<(), RegistryError> {
        if self.factories.contains_key(&id) {
            return Err(RegistryError::Duplicate { kind: E::KIND, id });
        }
        self.factories.insert(id, Box::new(factory));
        Ok(())
    }

    /// Registered ids in a stable order, for pickers and `aulo voice list`.
    pub fn ids(&self) -> impl Iterator<Item = &EngineId> {
        self.factories.keys()
    }

    pub fn contains(&self, id: &EngineId) -> bool {
        self.factories.contains_key(id)
    }

    /// An id nobody registered is reported as unavailable rather than as a
    /// caller error: a config naming an uninstalled plugin must fall back.
    pub fn build(&self, spec: &EngineSpec) -> Result<Box<E>, SpeechError> {
        match self.factories.get(&spec.engine) {
            Some(factory) => factory(spec),
            None => Err(SpeechError::unavailable("engine is not registered")),
        }
    }
}

impl<E: ?Sized + Engine> Default for EngineRegistry<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: ?Sized + Engine> fmt::Debug for EngineRegistry<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineRegistry")
            .field("kind", &E::KIND)
            .field("ids", &self.factories.keys().collect::<Vec<_>>())
            .finish()
    }
}
