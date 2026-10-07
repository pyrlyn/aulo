//! Client messages, as JSON text.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::{Dialect, RealtimeConfig, SAMPLE_RATE_HZ};

/// The message that configures the session: Realtime sends it after
/// `session.created`, GPT-Live as the very first message.
pub(crate) fn start(config: &RealtimeConfig) -> String {
    let mut output = json!({});
    if let Some(voice) = &config.voice {
        output["voice"] = json!(voice);
    }
    let mut session;
    let kind = match config.dialect {
        Dialect::Realtime => {
            let mut input = json!({"turn_detection": {"type": "server_vad"}});
            if let Some(model) = &config.transcription_model {
                input["transcription"] = json!({"model": model});
            }
            session = json!({"type": "realtime", "output_modalities": ["audio"],
                "audio": {"input": input, "output": output}});
            if !config.tools.is_empty() {
                session["tools"] = config.tools.iter().map(tool).collect();
                session["tool_choice"] = json!("auto");
            }
            // The model is in the URL and cannot change after the first update.
            "session.update"
        }
        Dialect::Live => {
            // One format for both directions, fixed for the whole session.
            let format = json!({"type": "audio/pcm", "rate": SAMPLE_RATE_HZ});
            session = json!({"model": config.model, "audio": {"format": format, "output": output}});
            "session.start"
        }
    };
    if let Some(instructions) = &config.instructions {
        session["instructions"] = json!(instructions);
    }
    json!({"type": kind, "session": session}).to_string()
}

fn tool(tool: &crate::Tool) -> Value {
    json!({"type": "function", "name": tool.name, "description": tool.description,
        "parameters": tool.parameters})
}

pub(crate) fn audio(dialect: Dialect, pcm: &[u8]) -> String {
    let kind = match dialect {
        Dialect::Realtime => "input_audio_buffer.append",
        Dialect::Live => "session.input_audio.append",
    };
    json!({"type": kind, "audio": STANDARD.encode(pcm)}).to_string()
}

pub(crate) fn cancel() -> String {
    json!({"type": "response.cancel"}).to_string()
}

/// The result of a tool call, and the request that lets the model speak about it.
pub(crate) fn tool_output(call_id: &str, output: &str) -> [String; 2] {
    [
        json!({"type": "conversation.item.create",
            "item": {"type": "function_call_output", "call_id": call_id, "output": output}})
        .to_string(),
        json!({"type": "response.create"}).to_string(),
    ]
}

pub(crate) fn live_close() -> String {
    json!({"type": "session.close"}).to_string()
}
