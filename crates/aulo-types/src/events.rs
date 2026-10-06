//! Events that go beyond the agent loop: voice state, speech, transcripts, TTS
//! metadata, approvals, takeover and notices. They are one tagged enum so the
//! gRPC layer and the FFI layer map a single type.

use serde::{Deserialize, Serialize};

use crate::{CallId, ChatId, TurnId};

/// Where the voice pipeline is for the current client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VoiceState {
    Idle,
    Listening,
    Thinking,
    Speaking,
}

/// Whether a transcript may still change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TranscriptKind {
    /// A streaming hypothesis the engine may revise.
    Partial,
    /// The engine's committed text for the utterance.
    Final,
}

/// Metadata of one synthesized audio chunk. The samples travel on the audio
/// path, not in events, so events stay small and the audio path stays bounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TtsChunk {
    pub turn_id: TurnId,
    /// Position in the reply, so a client can detect a dropped chunk.
    pub seq: u32,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub duration_ms: u32,
    /// True on the last chunk of the reply.
    pub last: bool,
}

/// What a human has to do themselves because the agent must not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TakeoverReason {
    Password,
    TwoFactor,
    Captcha,
    Other,
}

/// Severity of a [`AuloEvent::Notice`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum NoticeLevel {
    Info,
    Warn,
    Error,
}

/// An event pushed to clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AuloEvent {
    VoiceState {
        state: VoiceState,
    },
    SpeechStarted {
        /// Offset into the capture stream, so clients can align it with audio.
        at_ms: u64,
    },
    SpeechEnded {
        at_ms: u64,
    },
    Transcript {
        turn_id: TurnId,
        kind: TranscriptKind,
        text: String,
        /// BCP 47 tag when the engine detected or was told the language.
        language: Option<String>,
    },
    TtsChunk(TtsChunk),
    ApprovalRequired {
        chat_id: ChatId,
        turn_id: TurnId,
        call_id: CallId,
        tool: String,
        summary: String,
        /// Voice never widens access: when true, only a click or key approves.
        needs_click: bool,
    },
    TakeoverRequested {
        chat_id: ChatId,
        call_id: CallId,
        reason: TakeoverReason,
    },
    Notice {
        level: NoticeLevel,
        message: String,
        /// The engine, server or plugin the notice is about, if any.
        source: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn round_trip(event: &AuloEvent) -> AuloEvent {
        serde_json::from_str(&serde_json::to_string(event).unwrap()).unwrap()
    }

    #[test]
    fn every_event_survives_a_json_round_trip() {
        let events = [
            AuloEvent::VoiceState {
                state: VoiceState::Speaking,
            },
            AuloEvent::SpeechStarted { at_ms: 120 },
            AuloEvent::SpeechEnded { at_ms: 980 },
            AuloEvent::Transcript {
                turn_id: TurnId::new(),
                kind: TranscriptKind::Partial,
                text: "open the".into(),
                language: Some("en".into()),
            },
            AuloEvent::TtsChunk(TtsChunk {
                turn_id: TurnId::new(),
                seq: 3,
                sample_rate_hz: 24_000,
                channels: 1,
                duration_ms: 200,
                last: false,
            }),
            AuloEvent::ApprovalRequired {
                chat_id: ChatId::new(),
                turn_id: TurnId::new(),
                call_id: CallId::new(),
                tool: "shell".into(),
                summary: "rm -rf build".into(),
                needs_click: true,
            },
            AuloEvent::TakeoverRequested {
                chat_id: ChatId::new(),
                call_id: CallId::new(),
                reason: TakeoverReason::Captcha,
            },
            AuloEvent::Notice {
                level: NoticeLevel::Warn,
                message: "stt engine failed, falling back".into(),
                source: Some("whisper".into()),
            },
        ];
        for event in &events {
            assert_eq!(&round_trip(event), event);
        }
    }

    #[test]
    fn events_use_a_snake_case_type_tag() {
        let json = serde_json::to_value(AuloEvent::VoiceState {
            state: VoiceState::Idle,
        })
        .unwrap();
        assert_eq!(json, json!({"type": "voice_state", "state": "idle"}));
    }

    #[test]
    fn unknown_event_type_is_rejected() {
        assert!(serde_json::from_value::<AuloEvent>(json!({"type": "teleport"})).is_err());
    }

    #[test]
    fn notice_levels_order_by_severity() {
        assert!(NoticeLevel::Info < NoticeLevel::Warn && NoticeLevel::Warn < NoticeLevel::Error);
    }
}
