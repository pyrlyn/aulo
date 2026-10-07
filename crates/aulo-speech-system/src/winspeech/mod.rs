//! Windows system voices over WinRT `SpeechSynthesizer`.
//!
//! - `engine` is the [`aulo_speech::TtsEngine`] the pipeline drives; it only
//!   talks to the worker through a bounded channel and a fixed-size ring.
//! - `worker` owns the synthesis backend on its own thread and streams each
//!   sentence's audio into the ring.
//! - `voices` and `convert` hold the voice choice and rate mapping.
//! - `winrt` is the only module that touches Windows APIs.
//!
//! Everything but `winrt` compiles on every host and is tested with a fake
//! backend, because the WinRT calls cannot run anywhere else.

// Off Windows only the tests build the engine.
#![cfg_attr(not(windows), allow(dead_code))]

mod convert;
mod engine;
mod voices;
#[cfg(windows)]
mod winrt;
mod worker;

#[cfg(test)]
mod tests;

/// The id config names (`engine = "system"`); every platform registers its
/// system voices under it, so one config works on every desktop.
pub const ENGINE_ID: &str = "system";

#[cfg(windows)]
pub use windows_api::{SystemTts, factory};

#[cfg(windows)]
mod windows_api {
    use aulo_speech::{EngineSpec, SpeechError, TtsEngine};

    use super::engine::WindowsTts;
    use super::winrt::WinRtBackend;

    /// Text-to-speech over the voices installed in Windows.
    pub type SystemTts = WindowsTts;

    impl WindowsTts {
        /// Starts the worker, which opens `SpeechSynthesizer` and lists the
        /// voices. `default_voice` is a [`aulo_speech::Voice::id`] used when a
        /// request names none; an unknown one makes `begin` report it
        /// unsupported, so the registry falls back.
        pub fn new(default_voice: Option<String>) -> Result<Self, SpeechError> {
            Self::start(WinRtBackend::open, default_voice)
        }
    }

    /// The registry factory: `registry.register(EngineId::new(ENGINE_ID)?, factory)`.
    /// `spec.voice` becomes the default voice; model and rate are per reply.
    pub fn factory(spec: &EngineSpec) -> Result<Box<dyn TtsEngine>, SpeechError> {
        Ok(Box::new(WindowsTts::new(spec.voice.clone())?))
    }
}
