//! The voice domain (spec §4.2): the conversation pipeline's engine handling
//! and text side. Everything here is synchronous and does no I/O; the
//! pipeline (T8.1) drives it from the speech worker, never the audio path.
//!
//! Engines (spec §5.2):
//! - [`EngineRegistry`] maps an [`aulo_speech::EngineId`] to a factory.
//!   Built-in engines and plugin proxies register the same way, so a plugin
//!   engine is chosen, switched and fallen back from like a built-in one.
//! - [`candidates`] resolves config, bot and chat choices into the ordered
//!   list of engines to try.
//! - [`ActiveEngine`] owns the engine in use. A switch is applied only when
//!   the next utterance or reply begins, and an engine that fails is skipped
//!   with an [`aulo_types::AuloEvent::Notice`]: voice fails open.
//!
//! Speakable text (T7.15): [`speakable`] turns model output into what a TTS
//! engine should say, and [`SpeakableStream`] cuts it into sentences so
//! playback can start on the first one. The reply text is untrusted model
//! output, so every entry point caps its input.

mod active;
mod expand;
mod registry;
mod resolve;
mod sentences;
mod speakable;
mod stream;

pub use active::{ActiveEngine, ActiveStt, ActiveTts};
pub use registry::{Engine, EngineRegistry, Factory, RegistryError, SttRegistry, TtsRegistry};
pub use resolve::{Overrides, candidates};
pub use sentences::SentenceSplitter;
pub use speakable::{
    Language, MAX_INPUT_BYTES, MAX_SPOKEN_CHARS, SpeakOptions, detect_language, speakable,
};
pub use stream::SpeakableStream;
