//! macOS system voices over `AVSpeechSynthesizer.writeUtterance:toBufferCallback:`.
//!
//! - `engine` is the [`aulo_speech::TtsEngine`] the pipeline drives.
//! - `worker` owns the synthesizer on its own thread and turns buffers from
//!   the main-queue callback into samples in a fixed-size ring.
//! - `voices` reads the installed voices and picks one for a language.
//! - `stt` is the [`aulo_speech::SttEngine`] over `SFSpeechRecognizer`, and
//!   `recognizer` its probe and worker thread.
//! - `convert` holds the pure sample math, so it is tested without
//!   AVFoundation.

// AVFoundation is reached only through objc2's generated bindings, and every
// one of them is `unsafe`. The lint is relaxed for this module alone; each
// block carries a SAFETY comment and the rest of the workspace stays denied.
#![allow(unsafe_code)]

mod convert;
mod engine;
mod recognizer;
mod stt;
mod voices;
mod worker;

pub use engine::{ENGINE_ID, SystemTts, factory};
pub use stt::{MAX_TRANSCRIPT_BYTES, SystemStt, stt_factory};

/// Tests that reach AVSpeechSynthesisVoice or AVSpeechSynthesizer take this first, so they run
/// one at a time. Three of them starting the speech framework at once hung a cold macOS CI
/// runner until the job timed out.
#[cfg(test)]
fn avspeech_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
