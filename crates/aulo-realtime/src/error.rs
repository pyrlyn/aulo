use thiserror::Error;

/// Why a realtime call failed. Messages are fixed text: what the server sends
/// back is untrusted and its errors quote keys and account details, so only
/// the HTTP status or a category is ever kept.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RealtimeError {
    #[error("invalid {what}: {reason}")]
    Invalid {
        what: &'static str,
        reason: &'static str,
    },
    #[error("the realtime command is not supported by this API")]
    Unsupported,
    #[error("cannot reach the realtime server")]
    Unreachable,
    #[error("the realtime server rejected the API key (HTTP {0})")]
    Rejected(u16),
    #[error("rate limited (HTTP 429)")]
    RateLimited,
    #[error("the realtime server refused the request (HTTP {0})")]
    Refused(u16),
    #[error("the realtime request timed out")]
    TimedOut,
    #[error("the realtime server broke the protocol: {0}")]
    Protocol(&'static str),
    #[error("the realtime connection is closed")]
    Closed,
}

impl RealtimeError {
    pub(crate) fn invalid(what: &'static str, reason: &'static str) -> Self {
        Self::Invalid { what, reason }
    }

    /// Only the status is reported: the reply body is untrusted and 401s quote the key.
    pub(crate) fn from_status(status: u16) -> Self {
        match status {
            401 | 403 => Self::Rejected(status),
            429 => Self::RateLimited,
            _ => Self::Refused(status),
        }
    }
}
