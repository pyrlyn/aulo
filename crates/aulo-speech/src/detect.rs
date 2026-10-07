//! Detectors that watch the capture stream frame by frame: voice activity,
//! wake words and end of turn. They are cheap enough to answer on every push,
//! so unlike STT and TTS they return their result directly instead of being
//! polled.

use crate::{AudioFrame, EngineInfo, SpeechError};

/// A change in voice activity, stamped with the stream position (see
/// [`AudioFrame::position`]) where it happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadEvent {
    SpeechStarted { position: u64 },
    SpeechEnded { position: u64 },
}

/// Voice activity detection.
///
/// At most one event per pushed frame. Engines with minimum speech and
/// silence durations (hangover) never flip twice inside one 20 ms frame, so
/// one is enough, and it keeps the return value allocation-free.
pub trait Vad: Send {
    fn info(&self) -> &EngineInfo;

    /// Feeds one frame; frames must be contiguous. Engines that work on a
    /// different window size buffer internally in fixed-size storage.
    fn push(&mut self, frame: AudioFrame<'_>) -> Result<Option<VadEvent>, SpeechError>;

    /// Forgets all state, for example after the microphone owner changed.
    fn reset(&mut self);
}

/// A wake word was heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeywordHit {
    /// Index into the list given to [`KeywordSpotter::set_keywords`]. An index
    /// rather than the phrase keeps the hit allocation-free; the pipeline maps
    /// it to the bot or chat the keyword selects.
    pub keyword: usize,
    /// Stream position where the keyword ended.
    pub position: u64,
}

/// Wake-word detection over an open vocabulary.
pub trait KeywordSpotter: Send {
    fn info(&self) -> &EngineInfo;

    /// Replaces the keywords. Called on config changes, never on the audio
    /// path, so engines may allocate and rebuild models here.
    fn set_keywords(&mut self, phrases: &[&str]) -> Result<(), SpeechError>;

    fn push(&mut self, frame: AudioFrame<'_>) -> Result<Option<KeywordHit>, SpeechError>;

    fn reset(&mut self);
}

/// The detector's verdict after a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnDecision {
    /// The user may still be talking.
    Continue,
    /// The user's turn ended at this stream position; the pipeline calls
    /// [`crate::SttEngine::finish`].
    EndOfTurn { position: u64 },
}

/// End-of-turn detection: a silence timeout, optionally refined by a
/// semantic model on the audio (Smart Turn) or on the transcript.
pub trait TurnDetector: Send {
    fn info(&self) -> &EngineInfo;

    /// Feeds one frame with the VAD's current verdict, so detectors build on
    /// the selected VAD instead of each running their own.
    fn push(&mut self, frame: AudioFrame<'_>, in_speech: bool)
    -> Result<TurnDecision, SpeechError>;

    /// The latest partial transcript, for detectors that judge text. Audio-only
    /// detectors ignore it, hence the default.
    fn observe_transcript(&mut self, _text: &str) {}

    /// Starts over for the next turn.
    fn reset(&mut self);
}
