//! Chat sessions on the neutral agent loop (spec §4, T4.6).
//!
//! One [`Session`] per chat: submissions in, [`SessionEvent`]s out, with
//! cancellation, max steps and parallel or exclusive tool dispatch owned
//! by `agent-loop`. The daemon owns the channel a session emits on and
//! fans the events out to every attached client.
//!
//! Policy is not here. Every tool call passes the `Approvals` gate the
//! daemon installs (`aulo-policy`, T5.1), outside the model, and a broken
//! engine, MCP server or plugin is a notice, never a failed turn.

mod context;
mod events;
mod session;
mod sink;

pub use context::{ToolSpecSource, TurnContext};
pub use events::{
    ARGUMENTS_MAX_BYTES, ApprovalBy, NOTICE_MAX_BYTES, SessionEvent, SessionState,
    TEXT_DELTA_MAX_BYTES, TOOL_NAME_MAX_BYTES, TOOL_OUTPUT_MAX_BYTES, TurnStop, approval_summary,
};
pub use session::{
    ApprovalChoice, EVENT_CHANNEL, MAX_USER_TEXT_BYTES, Session, SessionConfig, SessionError,
    SessionParts, Submission,
};
