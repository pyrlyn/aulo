//! Store rows to `aulo.v1` messages.

use std::collections::HashMap;

use aulo_proto::Timestamp;
use aulo_proto::aulo::v1::{self, MessageRole, ModelRef, ToolCallStatus};
use aulo_store as store;

use super::input::MODEL_SEPARATOR;

/// Tool output is untrusted and can be huge; the proto promises 64 KiB here and
/// the full text stays in storage.
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

fn timestamp(ms: i64) -> Timestamp {
    Timestamp {
        seconds: ms.div_euclid(1000),
        // rem_euclid(1000) < 1000, so the nanoseconds always fit an i32.
        nanos: i32::try_from(ms.rem_euclid(1000) * 1_000_000).unwrap_or_default(),
    }
}

/// A chat created without a model stores an empty string, which reads back as an
/// unset `ModelRef` so the daemon can apply the bot's configured model later.
fn model_ref(stored: &str) -> ModelRef {
    match stored.split_once(MODEL_SEPARATOR) {
        Some((provider, model)) => ModelRef {
            provider: provider.to_owned(),
            model: model.to_owned(),
        },
        None => ModelRef {
            provider: String::new(),
            model: stored.to_owned(),
        },
    }
}

pub(super) fn chat(c: store::Chat) -> v1::Chat {
    v1::Chat {
        id: c.id,
        bot_id: c.bot_id,
        title: c.title,
        model: Some(model_ref(&c.model)),
        created_at: Some(timestamp(c.created_at)),
        updated_at: Some(timestamp(c.updated_at)),
    }
}

fn role(stored: &str) -> MessageRole {
    match stored {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "tool" => MessageRole::Tool,
        _ => MessageRole::Unspecified,
    }
}

fn status(stored: &str) -> ToolCallStatus {
    match stored {
        "running" => ToolCallStatus::Running,
        "succeeded" | "ok" => ToolCallStatus::Succeeded,
        "failed" => ToolCallStatus::Failed,
        "denied" => ToolCallStatus::Denied,
        "cancelled" => ToolCallStatus::Cancelled,
        _ => ToolCallStatus::Unspecified,
    }
}

fn cap(mut text: String) -> String {
    if text.len() > MAX_OUTPUT_BYTES {
        let end = (0..=MAX_OUTPUT_BYTES)
            .rev()
            .find(|&i| text.is_char_boundary(i))
            .unwrap_or(0);
        text.truncate(end);
    }
    text
}

fn tool_call(t: store::ToolCall) -> v1::ToolCall {
    v1::ToolCall {
        id: t.id,
        name: t.name,
        arguments_json: t.arguments,
        output: t.output.map(cap),
        status: status(&t.status).into(),
        created_at: Some(timestamp(t.created_at)),
    }
}

/// Attaches each call to its message, keeping the calls' oldest-first order.
pub(super) fn messages(rows: Vec<store::Message>, calls: Vec<store::ToolCall>) -> Vec<v1::Message> {
    let mut by_message: HashMap<String, Vec<v1::ToolCall>> = HashMap::new();
    for call in calls {
        by_message
            .entry(call.message_id.clone())
            .or_default()
            .push(tool_call(call));
    }
    rows.into_iter()
        .map(|m| v1::Message {
            tool_calls: by_message.remove(&m.id).unwrap_or_default(),
            role: role(&m.role).into(),
            id: m.id,
            chat_id: m.chat_id,
            content: m.content,
            created_at: Some(timestamp(m.created_at)),
        })
        .collect()
}
