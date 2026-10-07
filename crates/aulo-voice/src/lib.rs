//! The conversation pipeline's text side. Today this is the speakable-text
//! normalizer (T7.15): it turns model output into what a TTS engine should
//! say, and cuts it into sentences so playback can start on the first one.
//!
//! Everything here is pure and does no I/O. The reply text is untrusted model
//! output, so every entry point caps its input.

mod expand;
mod sentences;
mod speakable;
mod stream;

pub use sentences::SentenceSplitter;
pub use speakable::{
    Language, MAX_INPUT_BYTES, MAX_SPOKEN_CHARS, SpeakOptions, detect_language, speakable,
};
pub use stream::SpeakableStream;
