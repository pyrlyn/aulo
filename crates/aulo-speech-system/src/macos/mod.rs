//! macOS system voices over `AVSpeechSynthesizer.writeUtterance:toBufferCallback:`.
//!
//! - `engine` is the [`aulo_speech::TtsEngine`] the pipeline drives.
//! - `worker` owns the synthesizer on its own thread and turns buffers from
//!   the main-queue callback into samples in a fixed-size ring.
//! - `voices` reads the installed voices and picks one for a language.
//! - `convert` holds the pure sample math, so it is tested without
//!   AVFoundation.

// AVFoundation is reached only through objc2's generated bindings, and every
// one of them is `unsafe`. The lint is relaxed for this module alone; each
// block carries a SAFETY comment and the rest of the workspace stays denied.
#![allow(unsafe_code)]

mod convert;
mod engine;
mod voices;
mod worker;

pub use engine::{ENGINE_ID, SystemTts, factory};
