//! Server events to aulo events. Every field is untrusted: values are read
//! leniently from the JSON, sizes are capped, unknown event types are ignored
//! so a newer server does not break an older client, and no server text is
//! passed on except transcripts and tool calls, which are the payload.

use std::collections::VecDeque;

use aulo_types::{AuloEvent, NoticeLevel, TranscriptKind, TurnId, VoiceState};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

use crate::{Dialect, RealtimeError};

const SOURCE: &str = "openai-realtime";
/// Utterances in flight at once; a server that never completes them cannot grow the list.
const MAX_TURNS: usize = 16;
pub(crate) const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_ID_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    /// The model's JSON arguments, still untrusted text.
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ServerEvent {
    /// The session is configured and takes audio. Always the first event.
    Ready,
    /// Voice state, speech boundaries, user transcripts and notices.
    Aulo(AuloEvent),
    /// Raw mono PCM16 (or G.711) bytes of the reply, in playback order.
    Audio(Vec<u8>),
    /// What the assistant says: a fragment, or with `last` the whole final text
    /// (GPT-Live sends fragments only).
    AssistantText { text: String, last: bool },
    /// Realtime only. Policy has to pass before anything runs (spec §4.4).
    ToolCall(ToolCall),
    /// GPT-Live only: the session ended, with the voice seconds it used.
    Closed { seconds: Option<u64> },
}

struct Turn {
    key: String,
    id: TurnId,
    text: String,
}

pub(crate) struct Decoder {
    dialect: Dialect,
    speaking: bool,
    turns: VecDeque<Turn>,
}

impl Decoder {
    pub(crate) fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            speaking: false,
            turns: VecDeque::new(),
        }
    }

    pub(crate) fn decode(&mut self, text: &str) -> Result<Vec<ServerEvent>, RealtimeError> {
        let frame: Value = serde_json::from_str(text)
            .map_err(|_| RealtimeError::Protocol("unreadable message"))?;
        let kind =
            str_of(&frame, "type").ok_or(RealtimeError::Protocol("message without a type"))?;
        let mut out = Vec::new();
        match (self.dialect, kind) {
            (_, "error") => out.push(notice(&frame)),
            (Dialect::Realtime, kind) => self.realtime(kind, &frame, &mut out)?,
            (Dialect::Live, kind) => self.live(kind, &frame, &mut out)?,
        }
        Ok(out)
    }

    fn realtime(
        &mut self,
        kind: &str,
        frame: &Value,
        out: &mut Vec<ServerEvent>,
    ) -> Result<(), RealtimeError> {
        match kind {
            "session.created" => out.push(ServerEvent::Ready),
            "input_audio_buffer.speech_started" => {
                // Barge-in: the server already cancelled the reply being spoken.
                self.speaking = false;
                self.turn(str_of(frame, "item_id").unwrap_or_default());
                out.push(aulo(AuloEvent::SpeechStarted {
                    at_ms: number(frame, "audio_start_ms"),
                }));
                out.push(aulo(AuloEvent::VoiceState {
                    state: VoiceState::Listening,
                }));
            }
            "input_audio_buffer.speech_stopped" => {
                out.push(aulo(AuloEvent::SpeechEnded {
                    at_ms: number(frame, "audio_end_ms"),
                }));
                out.push(aulo(AuloEvent::VoiceState {
                    state: VoiceState::Thinking,
                }));
            }
            "conversation.item.input_audio_transcription.delta" => {
                let key = str_of(frame, "item_id").unwrap_or_default();
                self.partial(key, str_of(frame, "delta").unwrap_or_default(), out);
            }
            "conversation.item.input_audio_transcription.completed" => {
                let key = str_of(frame, "item_id").unwrap_or_default();
                let index = self.turn(key);
                let Some(turn) = self.turns.remove(index) else {
                    return Ok(());
                };
                let language = frame
                    .pointer("/languages/0/code")
                    .and_then(Value::as_str)
                    .filter(|code| code.len() <= 35)
                    .map(str::to_owned);
                out.push(final_transcript(
                    turn.id,
                    str_of(frame, "transcript").unwrap_or_default(),
                    language,
                ));
            }
            "response.output_audio.delta" => self.audio(frame, out)?,
            "response.output_audio_transcript.delta" => out.push(ServerEvent::AssistantText {
                text: clip(str_of(frame, "delta").unwrap_or_default()),
                last: false,
            }),
            "response.output_audio_transcript.done" => out.push(ServerEvent::AssistantText {
                text: clip(str_of(frame, "transcript").unwrap_or_default()),
                last: true,
            }),
            "response.function_call_arguments.done" => out.push(tool_call(frame)),
            "response.done" => {
                self.speaking = false;
                out.push(aulo(AuloEvent::VoiceState {
                    state: VoiceState::Listening,
                }));
            }
            _ => {}
        }
        Ok(())
    }

    fn live(
        &mut self,
        kind: &str,
        frame: &Value,
        out: &mut Vec<ServerEvent>,
    ) -> Result<(), RealtimeError> {
        match kind {
            "session.started" => out.push(ServerEvent::Ready),
            "session.input_transcript.delta" => {
                self.partial("", str_of(frame, "delta").unwrap_or_default(), out);
            }
            // The frames carry no turn marker, so the assistant starting to answer ends the user's turn.
            "session.output_transcript.delta" => {
                self.end_user_turn(out);
                out.push(ServerEvent::AssistantText {
                    text: clip(str_of(frame, "delta").unwrap_or_default()),
                    last: false,
                });
            }
            "session.output_audio.delta" => {
                self.end_user_turn(out);
                self.audio(frame, out)?;
            }
            "session.closed" => {
                self.end_user_turn(out);
                out.push(ServerEvent::Closed {
                    seconds: frame.pointer("/usage/seconds").and_then(Value::as_u64),
                });
            }
            _ => {}
        }
        Ok(())
    }

    fn audio(&mut self, frame: &Value, out: &mut Vec<ServerEvent>) -> Result<(), RealtimeError> {
        let bytes = STANDARD
            .decode(str_of(frame, "delta").unwrap_or_default())
            .map_err(|_| RealtimeError::Protocol("undecodable audio"))?;
        if self.dialect == Dialect::Realtime && !self.speaking {
            self.speaking = true;
            out.push(aulo(AuloEvent::VoiceState {
                state: VoiceState::Speaking,
            }));
        }
        out.push(ServerEvent::Audio(bytes));
        Ok(())
    }

    /// Index of the turn for `key`, opened if new; the oldest one is dropped at the cap.
    fn turn(&mut self, key: &str) -> usize {
        let key: String = key.chars().take(MAX_ID_BYTES).collect();
        if let Some(index) = self.turns.iter().position(|t| t.key == key) {
            return index;
        }
        if self.turns.len() == MAX_TURNS {
            self.turns.pop_front();
        }
        self.turns.push_back(Turn {
            key,
            id: TurnId::new(),
            text: String::new(),
        });
        self.turns.len() - 1
    }

    /// The transcript so far, as a hypothesis the server may still revise.
    fn partial(&mut self, key: &str, delta: &str, out: &mut Vec<ServerEvent>) {
        let index = self.turn(key);
        let Some(turn) = self.turns.get_mut(index) else {
            return;
        };
        // Past the cap the text stops growing instead of the memory.
        if turn.text.len() + delta.len() <= MAX_TEXT_BYTES {
            turn.text.push_str(delta);
        }
        out.push(aulo(AuloEvent::Transcript {
            turn_id: turn.id,
            kind: TranscriptKind::Partial,
            text: turn.text.clone(),
            language: None,
        }));
    }

    fn end_user_turn(&mut self, out: &mut Vec<ServerEvent>) {
        if let Some(turn) = self.turns.pop_front().filter(|t| !t.text.is_empty()) {
            out.push(final_transcript(turn.id, &turn.text, None));
        }
    }
}

fn aulo(event: AuloEvent) -> ServerEvent {
    ServerEvent::Aulo(event)
}

fn final_transcript(turn_id: TurnId, text: &str, language: Option<String>) -> ServerEvent {
    aulo(AuloEvent::Transcript {
        turn_id,
        kind: TranscriptKind::Final,
        text: clip(text),
        language,
    })
}

fn str_of<'a>(frame: &'a Value, key: &str) -> Option<&'a str> {
    frame.get(key).and_then(Value::as_str)
}

fn number(frame: &Value, key: &str) -> u64 {
    frame.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// At most [`MAX_TEXT_BYTES`], cut on a character boundary.
fn clip(text: &str) -> String {
    let mut end = text.len().min(MAX_TEXT_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

fn tool_call(frame: &Value) -> ServerEvent {
    let id = |key| str_of(frame, key).filter(|s| !s.is_empty() && s.len() <= MAX_ID_BYTES);
    let arguments = str_of(frame, "arguments").filter(|a| a.len() <= MAX_TEXT_BYTES);
    match (id("call_id"), id("name"), arguments) {
        (Some(call_id), Some(name), Some(arguments)) => ServerEvent::ToolCall(ToolCall {
            call_id: call_id.to_owned(),
            name: name.to_owned(),
            arguments: arguments.to_owned(),
        }),
        // A call the client cannot answer is dropped loudly instead of left hanging.
        _ => notice_text(
            NoticeLevel::Warn,
            "the realtime server sent a malformed tool call".into(),
        ),
    }
}

fn notice(frame: &Value) -> ServerEvent {
    // Only a short identifier-like code is kept: the message itself may quote the key.
    let code = frame
        .pointer("/error/code")
        .and_then(Value::as_str)
        .filter(|c| {
            c.len() <= 64
                && c.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        });
    let message = match code {
        Some(code) => format!("the realtime server reported an error ({code})"),
        None => "the realtime server reported an error".to_owned(),
    };
    notice_text(NoticeLevel::Error, message)
}

fn notice_text(level: NoticeLevel, message: String) -> ServerEvent {
    aulo(AuloEvent::Notice {
        level,
        message,
        source: Some(SOURCE.to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn delta(item: &str, text: &str) -> String {
        json!({"type": "conversation.item.input_audio_transcription.delta",
            "item_id": item, "delta": text})
        .to_string()
    }

    fn partial_text(events: &[ServerEvent]) -> &str {
        let [ServerEvent::Aulo(AuloEvent::Transcript { text, .. })] = events else {
            panic!("expected one transcript, got {events:?}");
        };
        text
    }

    #[test]
    fn a_transcript_stops_growing_at_the_cap() {
        let mut decoder = Decoder::new(Dialect::Realtime);
        let chunk = "x".repeat(MAX_TEXT_BYTES / 2 + 1);
        decoder.decode(&delta("a", &chunk)).unwrap();
        let events = decoder.decode(&delta("a", &chunk)).unwrap();
        assert_eq!(partial_text(&events).len(), chunk.len());
    }

    #[test]
    fn a_final_transcript_is_cut_on_a_character_boundary() {
        let mut decoder = Decoder::new(Dialect::Realtime);
        let frame = json!({"type": "conversation.item.input_audio_transcription.completed",
            "item_id": "a", "transcript": "é".repeat(MAX_TEXT_BYTES)});
        let events = decoder.decode(&frame.to_string()).unwrap();
        let ServerEvent::Aulo(AuloEvent::Transcript { text, .. }) = &events[0] else {
            panic!("expected a transcript, got {events:?}");
        };
        assert_eq!(text.len(), MAX_TEXT_BYTES);
    }

    #[test]
    fn open_turns_are_bounded_and_the_oldest_is_forgotten() {
        let mut decoder = Decoder::new(Dialect::Realtime);
        let first = decoder.decode(&delta("item_0", "a")).unwrap();
        for n in 1..=MAX_TURNS {
            decoder.decode(&delta(&format!("item_{n}"), "a")).unwrap();
        }
        assert_eq!(decoder.turns.len(), MAX_TURNS);
        // The evicted turn starts over under a new id instead of growing the list.
        let again = decoder.decode(&delta("item_0", "b")).unwrap();
        assert_eq!(partial_text(&again), "b");
        assert_ne!(first, again);
        assert_eq!(decoder.turns.len(), MAX_TURNS);
    }

    #[test]
    fn overlong_ids_are_cut_and_tool_calls_need_every_field() {
        let mut decoder = Decoder::new(Dialect::Realtime);
        decoder.decode(&delta(&"i".repeat(10_000), "a")).unwrap();
        assert!(decoder.turns.iter().all(|t| t.key.len() <= MAX_ID_BYTES));
        for frame in [
            json!({"type": "response.function_call_arguments.done", "name": "n", "arguments": "{}"}),
            json!({"type": "response.function_call_arguments.done", "call_id": "c", "name": "", "arguments": "{}"}),
            json!({"type": "response.function_call_arguments.done", "call_id": "c", "name": "n"}),
        ] {
            let events = decoder.decode(&frame.to_string()).unwrap();
            assert!(matches!(
                events.as_slice(),
                [ServerEvent::Aulo(AuloEvent::Notice {
                    level: NoticeLevel::Warn,
                    ..
                })]
            ));
        }
    }
}
