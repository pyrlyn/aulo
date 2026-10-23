//! The agent seam of the voice pipeline (T4.8).
//!
//! The pipeline submits final transcripts and streams the turn's events back.
//! The production implementation is aulo-agent's chat session behind
//! `SessionService.Converse` (T4.8, wb/core); the pipeline only knows this
//! trait, so the voice work does not wait on the agent core. Tests drive it
//! with a scripted session.
//!
//! Everything the session reports is untrusted model output: the pipeline
//! caps the text (the speakable-text normalizer's input cap) and never treats
//! a notice as more than text to show.

use aulo_speech::ErrorDetail;
use aulo_types::{ChatId, NoticeLevel, TurnId};
use tokio::sync::mpsc;

/// One user turn, as handed to the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    pub chat_id: ChatId,
    pub turn_id: TurnId,
    /// The final transcript: untrusted STT output, capped by the pipeline.
    pub text: String,
    /// BCP 47 tag when the STT engine detected or was told the language.
    pub language: Option<String>,
}

/// What a running turn reports back.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SessionEvent {
    /// A piece of the reply text. Untrusted model output; the pipeline runs
    /// it through the speakable-text normalizer before any engine sees it.
    Text { delta: String },
    /// A tool call started. Its description feeds the progress fillers
    /// (T8.8); until then the pipeline only forwards what it must.
    ToolStarted { name: String, summary: String },
    /// A notice about the turn, forwarded to clients as it stands.
    Notice { level: NoticeLevel, message: String },
    /// The turn is over; the pipeline speaks what it has.
    Finished,
    /// The turn was cancelled; the pipeline drops pending speech.
    Cancelled,
}

/// Why a session would not take a turn.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SessionError {
    /// The session cannot take the turn: the chat is gone, the agent is down.
    #[error("session unavailable: {0}")]
    Unavailable(ErrorDetail),
    /// The session broke while the turn was running.
    #[error("session failed: {0}")]
    Failed(ErrorDetail),
}

impl SessionError {
    pub fn unavailable(detail: &str) -> Self {
        Self::Unavailable(ErrorDetail::new(detail))
    }

    pub fn failed(detail: &str) -> Self {
        Self::Failed(ErrorDetail::new(detail))
    }
}

/// The agent side of a voice conversation: submissions in, events out.
///
/// `submit` starts one turn and returns the receiver its events arrive on.
/// Dropping that receiver is the cancel signal for a turn the pipeline no
/// longer cares about (barge-in, T8.5), which is why the events arrive on a
/// bounded channel: a session can never outrun the pipeline.
pub trait VoiceSession: Send + Sync {
    fn submit(&self, submission: Submission) -> Result<mpsc::Receiver<SessionEvent>, SessionError>;
}
