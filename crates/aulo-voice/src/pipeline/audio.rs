//! The two audio seams of the pipeline. The daemon implements them over
//! `aulo-audio` (`Capture` and `Playback`); tests implement them over scripted
//! frames and a recording sink. They live in the domain crate so the pipeline
//! never reaches an adapter (spec §4.2), and so `aulo-audio` stays the one
//! owner of the platform audio stack.

use aulo_speech::{AudioFrame, SpeechError};

/// Where microphone frames come from: whole 20 ms frames at 16 kHz mono, in
/// stream order, or `None` when nothing is ready yet. Implementations never
/// block: the pipeline decides when to look, so the platform audio thread
/// stays free.
pub trait FrameSource: Send {
    fn next_frame(&mut self) -> Option<AudioFrame<'_>>;
}

/// Where synthesized audio goes: the local speaker, or the channel of a
/// remote `VoiceService.Talk` client.
///
/// [`write`](Self::write) takes as much of the frame as it can hold and
/// returns how many per-channel samples that was; the caller offers the rest
/// again, because the buffer is bounded and waiting loses less than dropping
/// half a sentence.
pub trait AudioSink: Send {
    fn write(&mut self, frame: AudioFrame<'_>) -> usize;

    /// Marks the end of a reply: from here a dry buffer is silence between
    /// replies, not a failure.
    fn finish(&mut self);

    /// Barge-in: silence at once and throw away what is queued.
    fn stop(&mut self);

    /// Gain from `0.0` (mute) to `1.0` (as written).
    fn set_volume(&mut self, volume: f32) -> Result<(), SpeechError>;
}
