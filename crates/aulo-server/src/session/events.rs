//! Session events to `aulo.v1.Event`. `aulo-agent` already caps every text
//! field to the proto's limits, so this only renames.

use aulo_agent::{SessionEvent, TurnStop};
use aulo_proto::aulo::v1::event::Kind;
use aulo_proto::aulo::v1::{
    ApprovalDecision, ApprovalRequired, ApprovalResolved, Event, Notice, NoticeLevel,
    TakeoverReason, TakeoverRequested, TextDelta, ToolCallFinished, ToolCallOutput,
    ToolCallStarted, ToolCallStatus, TurnEndReason, TurnFinished, TurnStarted,
};
use aulo_types::{AuloEvent, NoticeLevel as Level, TakeoverReason as Reason};

/// `None` for what clients do not get: usage rows (they go to the store) and
/// voice events, which no session emits.
pub(crate) fn event(e: SessionEvent) -> Option<Event> {
    let kind = match e {
        SessionEvent::Shared(shared) => shared_kind(shared)?,
        SessionEvent::TurnStarted { turn, .. } => Kind::TurnStarted(TurnStarted {
            turn_id: turn.to_string(),
        }),
        SessionEvent::TextDelta { turn, text } => Kind::TextDelta(TextDelta {
            turn_id: turn.to_string(),
            text,
        }),
        SessionEvent::ToolCallStarted {
            turn,
            call,
            tool,
            arguments_json,
        } => Kind::ToolCallStarted(ToolCallStarted {
            turn_id: turn.to_string(),
            call_id: call.to_string(),
            tool,
            arguments_json,
        }),
        SessionEvent::ToolCallOutput { call, text } => Kind::ToolCallOutput(ToolCallOutput {
            call_id: call.to_string(),
            text,
        }),
        SessionEvent::ToolCallFinished { call, ok, .. } => {
            Kind::ToolCallFinished(ToolCallFinished {
                call_id: call.to_string(),
                status: if ok {
                    ToolCallStatus::Succeeded
                } else {
                    ToolCallStatus::Failed
                }
                .into(),
            })
        }
        SessionEvent::ApprovalResolved {
            call,
            allowed,
            timed_out,
            ..
        } => Kind::ApprovalResolved(ApprovalResolved {
            call_id: call.to_string(),
            decision: if allowed {
                ApprovalDecision::AllowOnce
            } else {
                ApprovalDecision::DenyOnce
            }
            .into(),
            timed_out,
        }),
        SessionEvent::Usage { .. } => return None,
        SessionEvent::TurnFinished { turn, stop } => {
            let (reason, error) = match stop {
                TurnStop::Completed => (TurnEndReason::Completed, None),
                TurnStop::Interrupted => (TurnEndReason::Interrupted, None),
                TurnStop::MaxSteps => (TurnEndReason::MaxSteps, None),
                // The loop's error may quote a provider response, which can
                // echo request details; the log keeps it, the client gets this.
                TurnStop::Failed => (
                    TurnEndReason::Failed,
                    Some("the model call failed; see the daemon log".to_owned()),
                ),
            };
            Kind::TurnFinished(TurnFinished {
                turn_id: turn.to_string(),
                reason: reason.into(),
                error,
            })
        }
    };
    Some(Event { kind: Some(kind) })
}

fn shared_kind(e: AuloEvent) -> Option<Kind> {
    Some(match e {
        AuloEvent::ApprovalRequired {
            chat_id,
            turn_id,
            call_id,
            tool,
            summary,
            needs_click,
        } => Kind::ApprovalRequired(ApprovalRequired {
            chat_id: chat_id.to_string(),
            turn_id: turn_id.to_string(),
            call_id: call_id.to_string(),
            tool,
            summary,
            needs_click,
        }),
        AuloEvent::TakeoverRequested {
            chat_id,
            call_id,
            reason,
        } => Kind::TakeoverRequested(TakeoverRequested {
            chat_id: chat_id.to_string(),
            call_id: call_id.to_string(),
            reason: match reason {
                Reason::Password => TakeoverReason::Password,
                Reason::TwoFactor => TakeoverReason::TwoFactor,
                Reason::Captcha => TakeoverReason::Captcha,
                _ => TakeoverReason::Other,
            }
            .into(),
        }),
        AuloEvent::Notice {
            level,
            message,
            source,
        } => notice(level, message, source),
        _ => return None,
    })
}

pub(crate) fn notice(level: Level, message: String, source: Option<String>) -> Kind {
    Kind::Notice(Notice {
        level: match level {
            Level::Info => NoticeLevel::Info,
            Level::Warn => NoticeLevel::Warn,
            _ => NoticeLevel::Error,
        }
        .into(),
        message,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aulo_types::TurnId;

    #[test]
    fn usage_stays_on_the_server_and_a_failure_hides_its_cause() {
        let turn = TurnId::new();
        assert_eq!(
            event(SessionEvent::Usage {
                turn,
                usage: llm_wire::Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                    cache_read_tokens: 0,
                    cache_write_tokens: 0,
                    estimated: true,
                    cost_usd: 0.0,
                    latency_ms: 0,
                },
            }),
            None
        );
        let Some(Event {
            kind: Some(Kind::TurnFinished(finished)),
        }) = event(SessionEvent::TurnFinished {
            turn,
            stop: TurnStop::Failed,
        })
        else {
            panic!("a turn end maps to TurnFinished");
        };
        assert_eq!(finished.turn_id, turn.to_string());
        assert_eq!(finished.reason, i32::from(TurnEndReason::Failed));
        assert!(finished.error.is_some_and(|e| e.len() <= 2048));
    }
}
