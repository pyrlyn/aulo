//! The one error type every engine returns. It is shared rather than an
//! associated type so engines of different origins fit in one registry, and its
//! categories decide fallback, which is the fail-open rule for voice.

use std::fmt;

/// Longest detail kept from an engine, in bytes. Details come from cloud
/// services and plugins, which are untrusted, and end up in notices and logs.
pub const MAX_ERROR_DETAIL_BYTES: usize = 512;

/// Engine-supplied text, capped and flattened to one line on construction so
/// an engine cannot flood logs or forge extra log lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorDetail(String);

impl ErrorDetail {
    pub fn new(text: &str) -> Self {
        let flat = text[..text.floor_char_boundary(MAX_ERROR_DETAIL_BYTES)]
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        Self(flat)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ErrorDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SpeechError {
    /// The engine cannot run here: model missing, platform API absent, no key,
    /// plugin process down.
    #[error("engine unavailable: {0}")]
    Unavailable(ErrorDetail),
    /// The engine runs, but not for this request: language, voice or audio
    /// format it does not handle. Another engine may.
    #[error("not supported: {0}")]
    Unsupported(ErrorDetail),
    /// The engine broke while working: decode error, network drop, bad reply.
    #[error("engine failed: {0}")]
    Failed(ErrorDetail),
    /// The engine's bounded input queue was full and this many per-channel
    /// samples were dropped. Not a failure: the pipeline counts it and goes on.
    #[error("engine queue full, dropped {dropped} samples")]
    Overflow { dropped: usize },
    /// A value failed validation. Static strings keep this allocation-free,
    /// because it can be raised on the audio path.
    #[error("invalid {what}: {reason}")]
    Invalid {
        what: &'static str,
        reason: &'static str,
    },
    /// The caller broke the call order, for example pushing before `begin`.
    #[error("called out of order: {0}")]
    OutOfOrder(&'static str),
}

impl SpeechError {
    pub fn unavailable(detail: &str) -> Self {
        Self::Unavailable(ErrorDetail::new(detail))
    }

    pub fn unsupported(detail: &str) -> Self {
        Self::Unsupported(ErrorDetail::new(detail))
    }

    pub fn failed(detail: &str) -> Self {
        Self::Failed(ErrorDetail::new(detail))
    }

    pub fn invalid(what: &'static str, reason: &'static str) -> Self {
        Self::Invalid { what, reason }
    }

    /// True when the registry should warn and hand the work to the next engine
    /// in the fallback list. Overflow and caller bugs stay with this engine:
    /// switching would not fix them.
    pub fn should_fall_back(&self) -> bool {
        matches!(
            self,
            Self::Unavailable(_) | Self::Unsupported(_) | Self::Failed(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_detail_is_capped_on_a_char_boundary() {
        let text = "я".repeat(MAX_ERROR_DETAIL_BYTES);
        let detail = ErrorDetail::new(&text);
        assert!(detail.as_str().len() <= MAX_ERROR_DETAIL_BYTES);
        assert!(detail.as_str().chars().all(|c| c == 'я'));
    }

    #[test]
    fn control_characters_cannot_forge_log_lines() {
        let detail = ErrorDetail::new("bad\nINFO forged\x1b[2J");
        assert_eq!(detail.as_str(), "bad INFO forged [2J");
    }

    #[test]
    fn only_engine_faults_trigger_fallback() {
        assert!(SpeechError::unavailable("no model").should_fall_back());
        assert!(SpeechError::unsupported("ru").should_fall_back());
        assert!(SpeechError::failed("socket closed").should_fall_back());
        assert!(!SpeechError::Overflow { dropped: 320 }.should_fall_back());
        assert!(!SpeechError::OutOfOrder("push before begin").should_fall_back());
        assert!(!SpeechError::invalid("audio frame", "odd length").should_fall_back());
    }
}
