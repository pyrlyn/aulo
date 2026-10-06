//! The contract shared by every aulo crate: typed ids and the events that go
//! beyond the agent loop. It has no I/O dependencies so the daemon, the
//! clients and the FFI layer can all depend on it without pulling in a runtime.

mod events;
mod ids;

pub use events::{AuloEvent, NoticeLevel, TakeoverReason, TranscriptKind, TtsChunk, VoiceState};
pub use ids::{BotId, CallId, ChatId, ClientId, IdParseError, TurnId};
