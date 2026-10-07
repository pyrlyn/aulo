//! Speech-to-text: audio frames in, partial and final transcripts out. The
//! transcript is a caller-owned buffer the engine overwrites, so streaming
//! partials reuses one allocation for the whole utterance.

use aulo_types::{TranscriptKind, TurnId};

use crate::{AudioFrame, EngineInfo, SpeechError};

/// The latest transcript of an utterance. Kept by the caller across polls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    /// The turn passed to [`SttEngine::begin`]. A network engine can deliver a
    /// result after the pipeline moved on (barge-in, cancel); the stamp lets
    /// the pipeline drop such stale text instead of attributing it to the new
    /// turn.
    pub turn_id: TurnId,
    pub kind: TranscriptKind,
    pub text: String,
    /// BCP 47 tag when the engine detected or was told the language.
    pub language: Option<String>,
}

impl Transcript {
    /// An empty partial with room for `capacity` bytes of text, so the caller
    /// allocates once up front instead of while partials stream in.
    pub fn with_capacity(turn_id: TurnId, capacity: usize) -> Self {
        Self {
            turn_id,
            kind: TranscriptKind::Partial,
            text: String::with_capacity(capacity),
            language: None,
        }
    }

    /// Overwrites the transcript in place, reusing the text and language
    /// buffers. Engines should update through this rather than assigning new
    /// strings.
    pub fn set(
        &mut self,
        turn_id: TurnId,
        kind: TranscriptKind,
        text: &str,
        language: Option<&str>,
    ) {
        self.turn_id = turn_id;
        self.kind = kind;
        self.text.clear();
        self.text.push_str(text);
        match (language, self.language.as_mut()) {
            (Some(new), Some(old)) => {
                old.clear();
                old.push_str(new);
            }
            (Some(new), None) => self.language = Some(new.to_owned()),
            (None, _) => self.language = None,
        }
    }
}

/// What a poll produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttPoll {
    /// Nothing new; `out` is untouched.
    Pending,
    /// `out` now holds a newer transcript; check its `kind`.
    Updated,
    /// The final transcript was already delivered; the utterance is over.
    Done,
}

/// A speech-to-text engine. See the crate docs for why it is push and poll.
///
/// One utterance at a time: `begin`, any number of `push` and `poll`, then
/// `finish`, then `poll` until [`SttPoll::Done`]. Exactly one
/// [`TranscriptKind::Final`] is delivered per finished utterance (its text may
/// be empty), and only after `finish`; end of turn is decided by the
/// [`crate::TurnDetector`], not by the STT engine. `begin` may be called again
/// at any point and implies `cancel`.
pub trait SttEngine: Send {
    fn info(&self) -> &EngineInfo;

    /// Starts an utterance. `language` is a BCP 47 hint; `None` asks the
    /// engine to detect it. Unknown languages return
    /// [`SpeechError::Unsupported`] so the registry can fall back.
    fn begin(&mut self, turn_id: TurnId, language: Option<&str>) -> Result<(), SpeechError>;

    /// Hands the engine one frame. Must not block: a full engine queue drops
    /// the frame and returns [`SpeechError::Overflow`].
    fn push(&mut self, frame: AudioFrame<'_>) -> Result<(), SpeechError>;

    /// No more audio for this utterance; the final transcript follows.
    fn finish(&mut self) -> Result<(), SpeechError>;

    /// Writes the newest transcript into `out` if there is one. Must not block.
    fn poll(&mut self, out: &mut Transcript) -> Result<SttPoll, SpeechError>;

    /// Drops the utterance and anything still queued. Must return at once:
    /// barge-in calls it under a 150 ms budget.
    fn cancel(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT_CAPACITY: usize = 64;

    #[test]
    fn set_reuses_the_text_buffer() {
        let turn = TurnId::new();
        let mut out = Transcript::with_capacity(turn, TEXT_CAPACITY);
        let buffer = out.text.as_ptr();
        out.set(turn, TranscriptKind::Partial, "open the", Some("en"));
        out.set(turn, TranscriptKind::Final, "open the door", Some("en-US"));
        assert_eq!(out.text.as_ptr(), buffer);
        assert_eq!(out.text, "open the door");
        assert_eq!(out.language.as_deref(), Some("en-US"));
        out.set(turn, TranscriptKind::Final, "", None);
        assert_eq!(out.language, None);
    }
}
