//! Text-to-speech: text in (sentence by sentence from the normalizer), audio
//! chunks out into a caller slice. Audio goes to aulo-audio rather than the
//! speaker, so barge-in and echo cancellation see every sample.

use aulo_types::TurnId;

use crate::{AudioFormat, EngineInfo, SpeechError};

/// A voice an engine offers, as listed in the desktop picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voice {
    /// What `[voice.tts] voice` and `aulo voice use` name.
    pub id: String,
    pub name: String,
    /// BCP 47 tag of the voice's language, when the engine knows it.
    pub language: Option<String>,
}

/// Speaking rate as a multiple of the voice's normal pace.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SpeechRate(f32);

impl SpeechRate {
    pub const NORMAL: Self = Self(1.0);
    /// Bounds most engines accept; beyond them speech is hard to follow.
    pub const MIN: f32 = 0.5;
    pub const MAX: f32 = 2.0;

    pub fn new(rate: f32) -> Result<Self, SpeechError> {
        // The range check also rejects NaN, which compares false both ways.
        if (Self::MIN..=Self::MAX).contains(&rate) {
            Ok(Self(rate))
        } else {
            Err(SpeechError::invalid(
                "speech rate",
                "must be within 0.5..=2.0",
            ))
        }
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

impl Default for SpeechRate {
    fn default() -> Self {
        Self::NORMAL
    }
}

/// How to speak one reply. Borrowed so starting a reply copies nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TtsRequest<'a> {
    pub turn_id: TurnId,
    /// A [`Voice::id`]; `None` takes the engine's default for `language`.
    pub voice: Option<&'a str>,
    /// BCP 47 tag of the text, from the normalizer's language detection.
    pub language: Option<&'a str>,
    pub rate: SpeechRate,
}

/// What a poll produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtsPoll {
    /// Nothing ready yet; the slice is untouched.
    Pending,
    /// The first `samples` interleaved samples of the slice hold new audio.
    Audio { samples: usize },
    /// All audio of the reply was delivered, or the reply was cancelled.
    Done,
}

/// A text-to-speech engine. See the crate docs for why it is push and poll.
///
/// One reply at a time: `begin`, any number of `push_text` and `poll`, then
/// `finish`, then `poll` until [`TtsPoll::Done`]. Text pushed before `finish`
/// may already be speaking, which is how the first sentence plays while the
/// rest is generated. `begin` may be called again at any point and implies
/// `cancel`.
pub trait TtsEngine: Send {
    fn info(&self) -> &EngineInfo;

    /// Voices known when the engine was loaded. Cloud engines fetch their list
    /// up front so this never blocks.
    fn voices(&self) -> &[Voice];

    /// Starts a reply and returns the format `poll` will write in. An unknown
    /// voice or language returns [`SpeechError::Unsupported`] so the registry
    /// can fall back.
    fn begin(&mut self, request: &TtsRequest<'_>) -> Result<AudioFormat, SpeechError>;

    /// Queues more text: usually one sentence. Must not block.
    fn push_text(&mut self, text: &str) -> Result<(), SpeechError>;

    /// No more text for this reply.
    fn finish(&mut self) -> Result<(), SpeechError>;

    /// Writes ready audio into `out`, at most `out.len()` samples and always a
    /// whole number of channel groups. Must not block.
    fn poll(&mut self, out: &mut [f32]) -> Result<TtsPoll, SpeechError>;

    /// Drops queued text and audio; the next poll returns [`TtsPoll::Done`].
    /// Must return at once: barge-in calls it under a 150 ms budget.
    fn cancel(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_outside_the_range_or_nan_is_rejected() {
        assert_eq!(SpeechRate::new(1.5).unwrap().get(), 1.5);
        for rate in [0.0, 0.49, 2.01, f32::NAN, f32::INFINITY] {
            assert!(SpeechRate::new(rate).is_err(), "{rate} was accepted");
        }
        assert_eq!(SpeechRate::default(), SpeechRate::NORMAL);
    }
}
