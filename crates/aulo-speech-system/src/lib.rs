//! Speech engines over the operating system's own voices (spec §4.2, §5.1).
//! They need no model download, so they are the fallback that always exists
//! on a desktop. Each platform lives in its own module behind a `cfg`, and a
//! build for any other target compiles this crate to nothing.
//!
//! Linux has no system synthesizer API, so its `system` engine is an
//! `espeak-ng` subprocess ([`espeak`]). That module compiles on every
//! platform, so its process handling is tested on macOS too.
//!
//! Audio never goes to the speaker from here: engines hand PCM to the
//! pipeline through [`aulo_speech::TtsEngine::poll`], so playback runs through
//! aulo-audio, where barge-in and echo cancellation see every sample
//! (`docs/spikes/aec.md`).
//!
//! # macOS threading
//!
//! [`SystemTts`] drives `AVSpeechSynthesizer` from its own worker thread, but
//! AVFoundation delivers the synthesized buffers on the **main** dispatch
//! queue. The host process must therefore keep the main thread's run loop
//! running (an `NSApplication`, `CFRunLoopRun`, or `dispatch_main`) and run
//! everything else, such as the tokio runtime, on other threads. Without it no
//! audio arrives, and the first `poll` after a few seconds reports
//! [`aulo_speech::SpeechError::Unavailable`] so the registry falls back.

pub mod espeak;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{ENGINE_ID, SystemTts, factory};

#[cfg(target_os = "linux")]
pub use espeak::{ENGINE_ID, EspeakTts as SystemTts, factory};
