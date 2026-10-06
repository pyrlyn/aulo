//! Typed ULID ids. Separate newtypes keep a `ChatId` from being passed where a
//! `TurnId` is expected, and ULIDs sort by creation time, which the store and
//! event log rely on for ordering.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// An id string that is not a valid ULID.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {kind} id {input:?}: {reason}")]
pub struct IdParseError {
    kind: &'static str,
    // Ids come from remote clients, so the offending text is kept for the log
    // but only ever shown inside this error.
    input: String,
    reason: ulid::DecodeError,
}

macro_rules! ulid_id {
    ($name:ident, $kind:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Ulid);

        impl $name {
            /// Generates a fresh id.
            #[must_use]
            pub fn new() -> Self {
                Self(Ulid::generate())
            }

            /// The underlying ULID, for timestamps and storage.
            #[must_use]
            pub const fn as_ulid(&self) -> Ulid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<Ulid> for $name {
            fn from(ulid: Ulid) -> Self {
                Self(ulid)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = IdParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ulid::from_string(s)
                    .map(Self)
                    .map_err(|reason| IdParseError {
                        kind: $kind,
                        input: s.chars().take(64).collect(),
                        reason,
                    })
            }
        }
    };
}

ulid_id!(
    BotId,
    "bot",
    "Identifies a bot (an agent persona with its own chats)."
);
ulid_id!(ChatId, "chat", "Identifies one conversation.");
ulid_id!(
    TurnId,
    "turn",
    "Identifies one user turn and the agent reply to it."
);
ulid_id!(CallId, "call", "Identifies one tool call within a turn.");
ulid_id!(
    ClientId,
    "client",
    "Identifies a connected client: a desktop app, the CLI or a remote device."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_display_and_parse_round_trip() {
        let id = ChatId::new();
        assert_eq!(id.to_string().len(), 26);
        assert_eq!(id.to_string().parse::<ChatId>(), Ok(id));
    }

    #[test]
    fn id_serializes_as_a_plain_string() {
        let id = TurnId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<TurnId>(&json).unwrap(), id);
    }

    #[test]
    fn parsing_garbage_names_the_id_kind() {
        let err = "not-a-ulid".parse::<CallId>().unwrap_err();
        assert!(err.to_string().starts_with("invalid call id"), "{err}");
    }

    #[test]
    fn parse_error_caps_the_echoed_input() {
        let long = "x".repeat(10_000);
        let err = long.parse::<BotId>().unwrap_err();
        assert!(err.to_string().len() < 200);
    }

    #[test]
    fn deserializing_garbage_fails() {
        assert!(serde_json::from_str::<ClientId>("\"nope\"").is_err());
    }

    #[test]
    fn later_ids_sort_after_earlier_ones() {
        let a = Ulid::from_parts(1, 0);
        let b = Ulid::from_parts(2, 0);
        assert!(TurnId::from(a) < TurnId::from(b));
    }
}
