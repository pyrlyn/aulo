//! The seam between the loop and the session's event channel: it maps each
//! `agent_loop::Event` onto a [`SessionEvent`]] and tracks the turn in
//! flight, because the loop's streamed events name their item, not their
//! turn.

use std::sync::{Mutex, PoisonError};

use aulo_types::{ChatId, TurnId};
use tokio::sync::mpsc;

use agent_loop::traits::EventSink;
use agent_loop::types::{Event as LoopEvent, LoopError};

use crate::events::{SessionEvent, map_event};

/// Where a session's loop events go. One per session; the outbox is the
/// channel the daemon fans out to attached clients and persists from.
pub(crate) struct SessionSink {
    chat_id: ChatId,
    outbox: mpsc::Sender<SessionEvent>,
    /// The turn in flight, from `TurnStarted` to `TurnDone`. Events are
    /// ordered through one sink, so a plain mutex is enough.
    turn: Mutex<Option<TurnId>>,
}

impl SessionSink {
    pub(crate) fn new(chat_id: ChatId, outbox: mpsc::Sender<SessionEvent>) -> Self {
        Self {
            chat_id,
            outbox,
            turn: Mutex::new(None),
        }
    }
}

impl std::fmt::Debug for SessionSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionSink")
            .field("chat_id", &self.chat_id)
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl EventSink for SessionSink {
    async fn emit(&self, event: LoopEvent) -> Result<(), LoopError> {
        let turn = *self.turn.lock().unwrap_or_else(PoisonError::into_inner);
        let (mapped, turn) = map_event(self.chat_id, turn, event)?;
        if let Some(turn) = turn {
            *self.turn.lock().unwrap_or_else(PoisonError::into_inner) = Some(turn);
        }
        if let Some(event) = mapped {
            // A reader that went away is not a reason to keep a turn
            // running: the session exists for its clients, and a full or
            // closed channel fails the turn closed.
            self.outbox.send(event).await.map_err(|_| LoopError::Sink {
                message: "the session's event channel closed".into(),
            })?;
        }
        Ok(())
    }
}
