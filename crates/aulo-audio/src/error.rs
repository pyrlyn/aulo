//! Capture errors. Device names and stream errors come from the operating
//! system, so the text is capped and flattened like every engine detail.

use aulo_speech::ErrorDetail;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AudioError {
    /// The machine has no input device (or the default one is gone).
    #[error("no input device available")]
    NoInputDevice,
    /// A configured device name matches no input device.
    #[error("input device not found: {0}")]
    DeviceNotFound(ErrorDetail),
    /// A value failed validation before any stream was opened.
    #[error("invalid {what}: {reason}")]
    Invalid {
        what: &'static str,
        reason: &'static str,
    },
    /// The platform refused or broke the stream: permission, format, driver.
    #[error("input stream failed: {0}")]
    Stream(ErrorDetail),
}

impl AudioError {
    pub fn stream(detail: &str) -> Self {
        Self::Stream(ErrorDetail::new(detail))
    }
}
