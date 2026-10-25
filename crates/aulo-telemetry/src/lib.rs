//! Logging for aulo: JSON log files with daily rotation, a human log on stderr and, behind the
//! `otlp` feature, OTLP trace export. [`telemetry_setup`] owns the subscriber; this crate names the
//! process `aulo`, honours [`LOG_ENV`] and masks aulod API tokens on top of the shared rules.

use std::path::PathBuf;

pub use telemetry_setup::{DEFAULT_MAX_FILES, Error, Guard, init, subscriber};

/// Environment variable that overrides the configured level; accepts any `EnvFilter` directive
/// such as `debug` or `aulo_store=trace,info`.
pub const LOG_ENV: &str = "AULO_LOG";

/// File prefix (`aulo.<date>.log`) and the OTLP service name.
const APP: &str = "aulo";

/// aulod API tokens. Matched case-insensitively by [`telemetry_setup`], on top of its built-in shapes.
const AULO_TOKEN: &str = r"\baulo_[0-9a-f]{16,}";

/// Where and how to log.
///
/// The fields are [`telemetry_setup::Settings`]. [`Settings::new`] fills the aulo defaults: the
/// app name, `AULO_LOG` when it is set, the aulod token pattern, stderr on, two weeks of files
/// and no OTLP endpoint.
#[derive(Debug, Clone)]
pub struct Settings(telemetry_setup::Settings);

impl std::ops::Deref for Settings {
    type Target = telemetry_setup::Settings;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Settings {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Settings {
    /// Settings with the stderr log on, [`DEFAULT_MAX_FILES`], no OTLP, and `AULO_LOG` honoured.
    #[must_use]
    pub fn new(level: impl Into<String>, log_dir: impl Into<PathBuf>) -> Self {
        let mut inner =
            telemetry_setup::Settings::new(APP, level, log_dir).filter_from_env(LOG_ENV);
        inner.extra_secret_patterns.push(AULO_TOKEN.to_owned());
        Self(inner)
    }
}

pub mod redact {
    //! Secret masking for aulo, using [`telemetry_setup`]'s rules plus the aulod token shape.
    //!
    //! The free functions keep the built-in fail-closed behaviour: if the rules are unavailable,
    //! the whole value is masked.

    use std::borrow::Cow;
    use std::sync::LazyLock;

    use serde_json::Value;
    use telemetry_setup::redact::Redactor;
    pub use telemetry_setup::redact::{REDACTED, is_secret_name};

    static REDACTOR: LazyLock<Option<Redactor>> =
        LazyLock::new(|| Redactor::new(&[crate::AULO_TOKEN]).ok());

    /// `text` with credential-shaped values, aulod API tokens and the values of secret-named
    /// pairs masked.
    ///
    /// Fails closed: if the patterns are unavailable, the whole text is masked.
    #[must_use]
    pub fn scrub_text(text: &str) -> Cow<'_, str> {
        match REDACTOR.as_ref() {
            Some(redactor) => redactor.scrub_text(text),
            None => Cow::Borrowed(REDACTED),
        }
    }

    /// Masks a parsed JSON value in place: secret-named keys lose their whole value (of any
    /// type), every other string goes through [`scrub_text`].
    pub fn scrub_json(value: &mut Value) {
        match REDACTOR.as_ref() {
            Some(redactor) => redactor.scrub_json(value),
            None => *value = Value::String(REDACTED.to_owned()),
        }
    }

    /// One log line without its newline: a JSON object is parsed and masked structurally,
    /// anything else (the human stderr format) is masked as text.
    #[must_use]
    pub fn scrub_line(line: &str) -> String {
        match REDACTOR.as_ref() {
            Some(redactor) => redactor.scrub_line(line),
            None => REDACTED.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests;
