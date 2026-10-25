//! The events a session emits: the agent loop's events mapped onto the
//! aulo-types events plus the agent-loop block the `aulo.v1.Event` proto
//! mirrors (proto/aulo/v1/event.proto).
//!
//! Everything that crosses this boundary is untrusted: model output, tool
//! output, plugin and MCP names. Every text field is capped here at the
//! proto's per-event limit, so a surface never has to re-check.

use std::str::FromStr;

use aulo_types::{AuloEvent, CallId, NoticeLevel, TurnId};
use llm_wire::{StopReason, Usage};

use agent_loop::types::{DecidedBy, Event as LoopEvent, Level, LoopError};

/// Streamed assistant text per delta, as the proto caps it.
pub const TEXT_DELTA_MAX_BYTES: usize = 16 * 1024;
/// Streamed tool output per event, as the proto caps it.
pub const TOOL_OUTPUT_MAX_BYTES: usize = 16 * 1024;
/// Model-chosen tool arguments, as the proto caps the display copy.
pub const ARGUMENTS_MAX_BYTES: usize = 16 * 1024;
/// Notice text and approval summaries, as the proto caps them.
pub const NOTICE_MAX_BYTES: usize = 2 * 1024;
/// A tool name, as the proto caps it.
pub const TOOL_NAME_MAX_BYTES: usize = 128;

/// One event a session emits, in loop order. Nothing is emitted after the
/// turn's [`SessionEvent::TurnFinished`].
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    /// An aulo-types event: a notice, an approval prompt or a takeover
    /// request. These pass through unchanged in meaning.
    Shared(AuloEvent),
    /// A user turn was accepted and the agent started on it.
    TurnStarted {
        /// The turn's id, in the aulo-types id space.
        turn: TurnId,
        /// The turn's ordinal, 1-based.
        seq: u32,
    },
    /// A streamed piece of the assistant reply.
    TextDelta {
        /// The turn the delta belongs to.
        turn: TurnId,
        /// The next chunk of text, capped at [`TEXT_DELTA_MAX_BYTES`].
        text: String,
    },
    /// The model requested a tool call.
    ToolCallStarted {
        /// The turn the call belongs to.
        turn: TurnId,
        /// The call's id, in the aulo-types id space.
        call: CallId,
        /// The tool's registered name, capped at [`TOOL_NAME_MAX_BYTES`].
        tool: String,
        /// Model-chosen arguments as JSON, capped at [`ARGUMENTS_MAX_BYTES`].
        arguments_json: String,
    },
    /// A streamed piece of tool output, for display only.
    ToolCallOutput {
        /// The call producing output.
        call: CallId,
        /// The next chunk, capped at [`TOOL_OUTPUT_MAX_BYTES`].
        text: String,
    },
    /// A tool call finished.
    ToolCallFinished {
        /// The call that finished.
        call: CallId,
        /// Whether it succeeded.
        ok: bool,
        /// What the model saw, capped at [`TOOL_OUTPUT_MAX_BYTES`].
        output: Option<String>,
        /// Wall-clock time the call took.
        duration_ms: u64,
    },
    /// An approval was answered, so every other attached client can
    /// dismiss its prompt.
    ApprovalResolved {
        /// The call that was decided.
        call: CallId,
        /// False only for a denial.
        allowed: bool,
        /// Who decided it.
        by: ApprovalBy,
        /// True when nobody answered in time and the server denied.
        timed_out: bool,
    },
    /// A provider call's usage, for the ledger (T4.12).
    Usage {
        /// The turn this usage belongs to.
        turn: TurnId,
        /// The usage the provider reported or the loop estimated.
        usage: Usage,
    },
    /// The turn finished; nothing follows for it.
    TurnFinished {
        /// The turn that finished.
        turn: TurnId,
        /// Why it stopped.
        stop: TurnStop,
    },
}

/// Why a turn ended. Mirrors the proto's `TurnEndReason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnStop {
    /// The model finished normally.
    Completed,
    /// An interrupt cancelled the turn.
    Interrupted,
    /// The step limit was hit before the model finished.
    MaxSteps,
    /// The turn ended in an error or a refusal.
    Failed,
}

/// Who answered an approval. Mirrors `agent_loop::types::DecidedBy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalBy {
    /// A client answered the prompt.
    User,
    /// A rule matched.
    Rule,
    /// A session grant matched.
    Session,
    /// A mode or policy decided without a rule, or an approval timed out.
    Policy,
    /// A hook decided.
    Hook,
}

/// Where a session is, for a status line or presence record. Mirrors
/// `agent_loop::types::State`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// No turn in flight.
    Idle,
    /// Building the next provider request.
    Assembling,
    /// A provider stream is open.
    Streaming,
    /// Tools from the last assistant message are running.
    RunningTools,
    /// Waiting on an approval answer.
    AwaitingApproval,
    /// Emitting the turn's finish.
    Finishing,
    /// An interrupt is ending the turn.
    Interrupted,
}

impl From<agent_loop::types::State> for SessionState {
    fn from(state: agent_loop::types::State) -> Self {
        use agent_loop::types::State;
        match state {
            State::Idle => Self::Idle,
            State::Assembling => Self::Assembling,
            State::Streaming => Self::Streaming,
            State::RunningTools => Self::RunningTools,
            State::AwaitingApproval => Self::AwaitingApproval,
            State::Finishing => Self::Finishing,
            State::Interrupted => Self::Interrupted,
        }
    }
}

impl From<DecidedBy> for ApprovalBy {
    fn from(by: DecidedBy) -> Self {
        match by {
            DecidedBy::User => Self::User,
            DecidedBy::Rule => Self::Rule,
            DecidedBy::Session => Self::Session,
            DecidedBy::Policy => Self::Policy,
            DecidedBy::Hook => Self::Hook,
        }
    }
}

fn map_stop(stop: StopReason) -> TurnStop {
    match stop {
        StopReason::EndTurn => TurnStop::Completed,
        StopReason::Interrupted => TurnStop::Interrupted,
        StopReason::MaxTurns => TurnStop::MaxSteps,
        StopReason::Budget | StopReason::Refusal { .. } | StopReason::Error => TurnStop::Failed,
    }
}

fn map_level(level: Level) -> NoticeLevel {
    match level {
        Level::Info => NoticeLevel::Info,
        Level::Warn => NoticeLevel::Warn,
    }
}

/// Truncates at a char boundary, silently: the cap is a display limit, and
/// the full text stays in the persisted row (T4.8).
fn capped(text: String, max: usize) -> String {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

/// Re-keys a loop id onto the matching aulo-types id. Both are ULIDs, so
/// the display form parses back; a failure is impossible in practice and
/// fails the turn closed rather than emitting an uncorrelated event.
fn rekey_turn(turn: agent_loop::TurnId) -> Result<TurnId, LoopError> {
    TurnId::from_str(&turn.to_string()).map_err(|_| LoopError::Sink {
        message: "turn id did not round-trip".into(),
    })
}

fn rekey_call(call: llm_wire::CallId) -> Result<CallId, LoopError> {
    CallId::from_str(&call.to_string()).map_err(|_| LoopError::Sink {
        message: "call id did not round-trip".into(),
    })
}

/// The human-readable line an approval prompt shows. Built from
/// model-chosen arguments, so it is untrusted and capped like a notice.
pub fn approval_summary(tool: &str, subject: &str) -> String {
    capped(format!("{tool}({subject})"), NOTICE_MAX_BYTES)
}

/// What one loop event maps to; `None` for the events that stay inside
/// the session (item and thinking bookkeeping, which the proto does not
/// carry).
pub(crate) fn map_event(
    chat_id: aulo_types::ChatId,
    turn: Option<TurnId>,
    event: LoopEvent,
) -> Result<(Option<SessionEvent>, Option<TurnId>), LoopError> {
    let current = || {
        turn.ok_or_else(|| LoopError::Sink {
            message: "an event arrived outside a turn".into(),
        })
    };
    let mapped = match event {
        LoopEvent::TurnStarted { turn: id, seq } => {
            let turn = rekey_turn(id)?;
            (Some(SessionEvent::TurnStarted { turn, seq }), Some(turn))
        }
        LoopEvent::ItemStarted { .. } | LoopEvent::ItemDone { .. } => (None, turn),
        LoopEvent::TextDelta { text, .. } => (
            Some(SessionEvent::TextDelta {
                turn: current()?,
                text: capped(text, TEXT_DELTA_MAX_BYTES),
            }),
            turn,
        ),
        // Thinking is not part of the aulo event surface: it never reaches
        // a client, a transcript or the model's own replay.
        LoopEvent::ThinkingDelta { .. } | LoopEvent::ThinkingDone { .. } => (None, turn),
        LoopEvent::ToolCallRequested { call } => (
            Some(SessionEvent::ToolCallStarted {
                turn: current()?,
                call: rekey_call(call.id)?,
                tool: capped(call.name, TOOL_NAME_MAX_BYTES),
                arguments_json: capped(call.input.to_string(), ARGUMENTS_MAX_BYTES),
            }),
            turn,
        ),
        LoopEvent::ApprovalRequired { call, why: _ } => (
            Some(SessionEvent::Shared(AuloEvent::ApprovalRequired {
                chat_id,
                turn_id: current()?,
                call_id: rekey_call(call.id)?,
                // Voice never widens access: a destructive call can only be
                // approved by a click or a key, never by a spoken yes.
                tool: capped(call.name.clone(), TOOL_NAME_MAX_BYTES),
                summary: approval_summary(&call.name, &call.subject),
                needs_click: call.risk == llm_wire::Risk::Destructive,
            })),
            turn,
        ),
        LoopEvent::ApprovalDecided {
            call_id,
            decision,
            by,
        } => (
            Some(SessionEvent::ApprovalResolved {
                call: rekey_call(call_id)?,
                allowed: !matches!(decision, agent_loop::types::Decision::Deny { .. }),
                by: by.into(),
                // The loop's timeout decides as `Policy`; the daemon's
                // approval queue (T5.3) tells the two apart.
                timed_out: false,
            }),
            turn,
        ),
        LoopEvent::ToolCallOutput { call_id, delta } => (
            Some(SessionEvent::ToolCallOutput {
                call: rekey_call(call_id)?,
                text: capped(delta, TOOL_OUTPUT_MAX_BYTES),
            }),
            turn,
        ),
        LoopEvent::ToolCallDone { call_id, result } => (
            Some(SessionEvent::ToolCallFinished {
                call: rekey_call(call_id)?,
                ok: result.ok,
                output: Some(capped(result.visible, TOOL_OUTPUT_MAX_BYTES)),
                duration_ms: result.duration_ms,
            }),
            turn,
        ),
        LoopEvent::Usage { turn: id, usage } => (
            Some(SessionEvent::Usage {
                turn: rekey_turn(id)?,
                usage,
            }),
            turn,
        ),
        LoopEvent::Notice { level, text } => (
            Some(SessionEvent::Shared(AuloEvent::Notice {
                level: map_level(level),
                message: capped(text, NOTICE_MAX_BYTES),
                source: None,
            })),
            turn,
        ),
        LoopEvent::TurnDone { turn: id, stop } => {
            let turn = rekey_turn(id)?;
            (
                Some(SessionEvent::TurnFinished {
                    turn,
                    stop: map_stop(stop),
                }),
                None,
            )
        }
        // The loop emits `Error` before its `TurnDone(Error)`, so a notice
        // here and the finish event carry the failure. A fatal error also
        // ends the session; the daemon (T4.8) watches for it.
        LoopEvent::Error { error, fatal: _ } => (
            Some(SessionEvent::Shared(AuloEvent::Notice {
                level: NoticeLevel::Error,
                message: capped(error.to_string(), NOTICE_MAX_BYTES),
                source: None,
            })),
            turn,
        ),
    };
    Ok(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aulo_types::ChatId;
    use llm_wire::{CallId as WireCallId, Risk};

    fn wire_call(name: &str, risk: Risk) -> agent_loop::types::ToolCall {
        agent_loop::types::ToolCall {
            id: WireCallId::new(),
            name: name.into(),
            input: serde_json::json!({"path": "notes.txt"}),
            risk,
            subject: "notes.txt".into(),
            segments: None,
        }
    }

    #[test]
    fn item_and_thinking_events_stay_inside_the_session() {
        let event = LoopEvent::ItemStarted {
            item: agent_loop::ItemId::new(),
            kind: agent_loop::types::ItemKind::UserMessage {
                text: "hi".into(),
                attachments: vec![],
            },
        };
        let (mapped, turn) = map_event(ChatId::new(), None, event).expect("map");
        assert!(mapped.is_none());
        assert!(turn.is_none());
    }

    #[test]
    fn an_approval_prompt_is_an_aulo_event_with_the_chat_and_turn() {
        let chat = ChatId::new();
        let turn = TurnId::new();
        let call = wire_call("shell", Risk::Destructive);
        let event = LoopEvent::ApprovalRequired {
            call: call.clone(),
            why: agent_loop::types::Why::Risk {
                risk: Risk::Destructive,
            },
        };
        let (mapped, _) = map_event(chat, Some(turn), event).expect("map");
        assert_eq!(
            mapped,
            Some(SessionEvent::Shared(AuloEvent::ApprovalRequired {
                chat_id: chat,
                turn_id: turn,
                call_id: CallId::from_str(&call.id.to_string()).expect("rekey"),
                tool: "shell".into(),
                summary: "shell(notes.txt)".into(),
                needs_click: true,
            }))
        );
    }

    #[test]
    fn a_write_approval_may_be_answered_by_voice() {
        let call = wire_call("edit", Risk::Write);
        let event = LoopEvent::ApprovalRequired {
            call,
            why: agent_loop::types::Why::Risk { risk: Risk::Write },
        };
        let (mapped, _) = map_event(ChatId::new(), Some(TurnId::new()), event).expect("map");
        assert!(
            matches!(&mapped, Some(SessionEvent::Shared(AuloEvent::ApprovalRequired { needs_click, .. })) if !needs_click)
        );
    }

    #[test]
    fn every_stop_reason_maps_to_a_turn_end() {
        for (stop, want) in [
            (StopReason::EndTurn, TurnStop::Completed),
            (StopReason::Interrupted, TurnStop::Interrupted),
            (StopReason::MaxTurns, TurnStop::MaxSteps),
            (StopReason::Budget, TurnStop::Failed),
            (StopReason::Error, TurnStop::Failed),
        ] {
            let (mapped, turn) = map_event(
                ChatId::new(),
                Some(TurnId::new()),
                LoopEvent::TurnDone {
                    turn: agent_loop::TurnId::new(),
                    stop,
                },
            )
            .expect("map");
            assert!(
                matches!(mapped, Some(SessionEvent::TurnFinished { stop, .. }) if stop == want)
            );
            assert!(turn.is_none(), "the turn ends");
        }
    }

    #[test]
    fn a_text_delta_is_capped_and_a_notice_keeps_its_level() {
        let (mapped, _) = map_event(
            ChatId::new(),
            Some(TurnId::new()),
            LoopEvent::TextDelta {
                item: agent_loop::ItemId::new(),
                text: "x".repeat(TEXT_DELTA_MAX_BYTES + 10),
            },
        )
        .expect("map");
        assert!(
            matches!(&mapped, Some(SessionEvent::TextDelta { text, .. }) if text.len() == TEXT_DELTA_MAX_BYTES)
        );

        let (mapped, _) = map_event(
            ChatId::new(),
            Some(TurnId::new()),
            LoopEvent::Notice {
                level: Level::Warn,
                text: "engine fell back".into(),
            },
        )
        .expect("map");
        assert_eq!(
            mapped,
            Some(SessionEvent::Shared(AuloEvent::Notice {
                level: NoticeLevel::Warn,
                message: "engine fell back".into(),
                source: None,
            }))
        );
    }

    #[test]
    fn a_denial_resolves_the_approval_as_not_allowed() {
        let call = WireCallId::new();
        let (mapped, _) = map_event(
            ChatId::new(),
            Some(TurnId::new()),
            LoopEvent::ApprovalDecided {
                call_id: call,
                decision: agent_loop::types::Decision::Deny {
                    reason: "no".into(),
                },
                by: DecidedBy::User,
            },
        )
        .expect("map");
        assert_eq!(
            mapped,
            Some(SessionEvent::ApprovalResolved {
                call: CallId::from_str(&call.to_string()).expect("rekey"),
                allowed: false,
                by: ApprovalBy::User,
                timed_out: false,
            })
        );
    }
}
