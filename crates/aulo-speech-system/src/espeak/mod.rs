//! espeak-ng as a text-to-speech engine, run as a separate process.
//!
//! This is the zero-download fallback voice on Linux (registered as the
//! `system` engine there), and it compiles on every platform so the process
//! handling is tested wherever the workspace builds.
//!
//! - `engine` is the [`aulo_speech::TtsEngine`] the pipeline drives.
//! - `worker` runs one `espeak-ng --stdout` per sentence and feeds a ring.
//! - `process` starts, decodes and reaps the child.
//! - `voices` parses `espeak-ng --voices` and maps rate and language to flags.
//!
//! # Why not speech-dispatcher
//!
//! speech-dispatcher is left out. Its SSIP protocol hands text to a module
//! that plays through the sound server itself, and a client cannot ask for the
//! audio back. Those samples would never pass aulo-audio, so barge-in and
//! echo cancellation (`docs/spikes/aec.md`) would not hear the assistant.
//! Its `sd_espeak-ng` module is the same synthesizer this engine already
//! drives, with audio that stays in aulo.
//!
//! # Licence
//!
//! espeak-ng is GPL-3.0. It runs as its own process and nothing of it is
//! linked, which is what the project's licence rule allows for GPL engines.

mod engine;
mod process;
mod voices;
mod worker;

// The fake `espeak-ng` is a shell script.
#[cfg(test)]
#[cfg(unix)]
mod tests;

pub use engine::{ENGINE_ID, EspeakTts, factory, factory_with};
