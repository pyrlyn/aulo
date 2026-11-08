//! A session's provider-form history to store rows and back.
//!
//! Each message keeps its `llm_wire::Message` JSON in `wire`, so tool use and
//! tool results survive a restart exactly. `content` holds only the text a
//! person reads, which `ListMessages` and search serve.

use aulo_store::{HistoryMessage, NewMessage, NewToolCall};
use llm_wire::{Content, Message, Role};

const USER: &str = "user";
const ASSISTANT: &str = "assistant";
/// A user-role message that carries tool results, not anything a person typed.
const TOOL: &str = "tool";

fn has_tool_use(message: &Message) -> bool {
    message
        .content
        .iter()
        .any(|c| matches!(c, Content::ToolUse { .. }))
}

fn role(message: &Message) -> &'static str {
    match message.role {
        Role::Assistant => ASSISTANT,
        Role::User
            if message
                .content
                .iter()
                .any(|c| matches!(c, Content::ToolResult { .. })) =>
        {
            TOOL
        }
        Role::User => USER,
    }
}

fn text(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// How much of `history` is safe to persist: everything except a trailing
/// tool use whose results are not in yet (a turn queued behind the one that
/// just finished may be mid-tools). Persisting it alone would resume into a
/// request the provider rejects.
pub(crate) fn complete_len(history: &[Message]) -> usize {
    match history.last() {
        Some(last) if last.role == Role::Assistant && has_tool_use(last) => history.len() - 1,
        _ => history.len(),
    }
}

/// Store rows for `messages`, which must start and end on a complete
/// boundary ([`complete_len`]). Each tool use takes its output and status
/// from the result in the message after it.
pub(crate) fn to_rows(messages: &[Message]) -> Vec<NewMessage> {
    messages
        .iter()
        .enumerate()
        .map(|(i, message)| {
            let results = messages
                .get(i + 1)
                .map_or(&[][..], |m| m.content.as_slice());
            NewMessage {
                role: role(message).to_owned(),
                content: text(message),
                wire: match serde_json::to_string(message) {
                    Ok(json) => Some(json),
                    Err(error) => {
                        tracing::error!(%error, "cannot encode a history message");
                        None
                    }
                },
                tool_calls: message
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        Content::ToolUse { id, name, input } => {
                            Some(tool_call(id, name, input, results))
                        }
                        _ => None,
                    })
                    .collect(),
            }
        })
        .collect()
}

fn tool_call(
    id: &llm_wire::CallId,
    name: &str,
    input: &serde_json::Value,
    results: &[Content],
) -> NewToolCall {
    let result = results.iter().find_map(|c| match c {
        Content::ToolResult {
            call_id,
            content,
            is_error,
        } if call_id == id => Some((content, *is_error)),
        _ => None,
    });
    let (output, status) = match result {
        Some((content, false)) => (Some(content.clone()), "succeeded"),
        Some((content, true)) => (Some(content.clone()), "failed"),
        None => (None, "cancelled"),
    };
    NewToolCall {
        name: name.to_owned(),
        arguments: input.to_string(),
        output,
        status: status.to_owned(),
    }
}

/// The history a resumed session continues. A row without a readable
/// `wire` falls back to its text; a tool row without one is skipped, since
/// its results cannot be rebuilt from text.
pub(crate) fn from_rows(rows: Vec<HistoryMessage>) -> Vec<Message> {
    rows.into_iter()
        .filter_map(|row| {
            if let Some(wire) = &row.wire {
                match serde_json::from_str::<Message>(wire) {
                    Ok(message) => return Some(message),
                    Err(error) => {
                        tracing::warn!(id = %row.id, %error, "unreadable stored message; using its text");
                    }
                }
            }
            let role = match row.role.as_str() {
                USER => Role::User,
                ASSISTANT => Role::Assistant,
                _ => return None,
            };
            (!row.content.is_empty()).then(|| Message {
                role,
                content: vec![Content::Text { text: row.content }],
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_wire::CallId;

    fn said(role: Role, text: &str) -> Message {
        Message {
            role,
            content: vec![Content::Text { text: text.into() }],
        }
    }

    fn tool_turn(id: CallId) -> Vec<Message> {
        vec![
            said(Role::User, "list files"),
            Message {
                role: Role::Assistant,
                content: vec![
                    Content::Text {
                        text: "looking".into(),
                    },
                    Content::ToolUse {
                        id,
                        name: "shell".into(),
                        input: serde_json::json!({"command": "ls"}),
                    },
                ],
            },
            Message {
                role: Role::User,
                content: vec![Content::ToolResult {
                    call_id: id,
                    content: "a.txt".into(),
                    is_error: false,
                }],
            },
            said(Role::Assistant, "one file"),
        ]
    }

    fn stored(rows: Vec<NewMessage>) -> Vec<HistoryMessage> {
        rows.into_iter()
            .enumerate()
            .map(|(i, r)| HistoryMessage {
                id: i.to_string(),
                role: r.role,
                content: r.content,
                wire: r.wire,
            })
            .collect()
    }

    #[test]
    fn a_tool_turn_round_trips_with_its_call_row() {
        let history = tool_turn(CallId::new());
        let rows = to_rows(&history);
        let roles: Vec<&str> = rows.iter().map(|r| r.role.as_str()).collect();
        assert_eq!(roles, [USER, ASSISTANT, TOOL, ASSISTANT]);
        assert_eq!(rows[1].content, "looking");
        assert_eq!(
            rows[1].tool_calls,
            [NewToolCall {
                name: "shell".into(),
                arguments: r#"{"command":"ls"}"#.into(),
                output: Some("a.txt".into()),
                status: "succeeded".into(),
            }]
        );
        assert_eq!(rows[2].content, "");
        assert_eq!(from_rows(stored(rows)), history);
    }

    #[test]
    fn a_trailing_tool_use_waits_for_its_results() {
        let history = tool_turn(CallId::new());
        assert_eq!(complete_len(&history[..2]), 1);
        assert_eq!(complete_len(&history[..3]), 3);
        assert_eq!(complete_len(&[]), 0);
    }

    #[test]
    fn a_use_without_a_result_is_recorded_as_cancelled() {
        let history = tool_turn(CallId::new());
        let rows = to_rows(&history[..2]);
        assert_eq!(rows[1].tool_calls[0].status, "cancelled");
        assert_eq!(rows[1].tool_calls[0].output, None);
    }

    #[test]
    fn rows_without_wire_fall_back_to_text_and_skip_tool_rows() {
        let rows = vec![
            HistoryMessage {
                id: "1".into(),
                role: USER.into(),
                content: "hi".into(),
                wire: None,
            },
            HistoryMessage {
                id: "2".into(),
                role: TOOL.into(),
                content: String::new(),
                wire: None,
            },
            HistoryMessage {
                id: "3".into(),
                role: ASSISTANT.into(),
                content: "hello".into(),
                wire: Some("not json".into()),
            },
        ];
        assert_eq!(
            from_rows(rows),
            [said(Role::User, "hi"), said(Role::Assistant, "hello")]
        );
    }
}
