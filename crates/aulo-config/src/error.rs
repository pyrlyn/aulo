use crate::load::Origin;

/// A configuration that could not be loaded, with the layer that caused it.
#[derive(Debug, thiserror::Error)]
#[error("invalid config{}: {message}", origin.as_ref().map(|o| format!(" in {o}")).unwrap_or_default())]
pub struct ConfigError {
    /// Layer that supplied the offending value; `None` when figment cannot tell.
    pub origin: Option<Origin>,
    pub message: String,
}
