//! Logging for aulo: JSON log files with daily rotation, a human log on stderr and, behind the
//! `otlp` feature, OTLP trace export. Secrets are masked before any line reaches a sink
//! (see [`redact`]).

use std::path::PathBuf;

use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::Registry;
use tracing_subscriber::{EnvFilter, Layer, fmt};

#[cfg(feature = "otlp")]
mod otlp;
pub mod redact;
mod writer;

use writer::Scrubbed;

/// Environment variable that overrides the configured level; accepts any `EnvFilter` directive
/// such as `debug` or `aulo_store=trace,info`.
pub const LOG_ENV: &str = "AULO_LOG";

/// Rotated files kept when the caller does not say otherwise: two weeks of daily logs.
pub const DEFAULT_MAX_FILES: usize = 14;

/// A layer boxed so the optional OTLP layer has the same type with and without the feature.
type BoxedLayer = Box<dyn Layer<Registry> + Send + Sync>;

/// Why telemetry could not start.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid log filter `{directive}`: {source}")]
    Filter {
        directive: String,
        source: tracing_subscriber::filter::ParseError,
    },
    #[error("cannot open the log directory: {0}")]
    LogDir(#[from] tracing_appender::rolling::InitError),
    #[error("a global tracing subscriber is already installed")]
    AlreadyInstalled(#[from] tracing::subscriber::SetGlobalDefaultError),
    #[error("OTLP export is configured but this build lacks the `otlp` feature")]
    OtlpUnavailable,
    #[cfg(feature = "otlp")]
    #[error("cannot start OTLP export: {0}")]
    Otlp(String),
}

/// Where and how to log.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Level from config (`info`, `aulo_store=debug,warn`, ...), used when `filter_override` is unset.
    pub level: String,
    /// Takes precedence over `level`; [`Settings::new`] fills it from `AULO_LOG`.
    pub filter_override: Option<String>,
    /// Directory for the rotated JSON files; created when missing.
    pub log_dir: PathBuf,
    /// Rotated files to keep; values below 1 are treated as 1.
    pub max_files: usize,
    /// Also write a human-readable log to stderr (the CLI wants it, the daemon does not).
    pub stderr: bool,
    /// Full URL of an OTLP/HTTP traces endpoint, such as `http://localhost:4318/v1/traces`.
    /// Needs the `otlp` feature; plain HTTP only.
    pub otlp_endpoint: Option<String>,
}

impl Settings {
    /// Settings with the stderr log on, [`DEFAULT_MAX_FILES`], no OTLP, and `AULO_LOG` honoured.
    #[must_use]
    pub fn new(level: impl Into<String>, log_dir: impl Into<PathBuf>) -> Self {
        Self {
            level: level.into(),
            filter_override: std::env::var(LOG_ENV).ok().filter(|v| !v.trim().is_empty()),
            log_dir: log_dir.into(),
            max_files: DEFAULT_MAX_FILES,
            stderr: true,
            otlp_endpoint: None,
        }
    }

    fn filter(&self) -> Result<EnvFilter, Error> {
        let directive = self.filter_override.as_deref().unwrap_or(&self.level);
        EnvFilter::try_new(directive).map_err(|source| Error::Filter {
            directive: directive.to_owned(),
            source,
        })
    }
}

/// Keeps buffered logs and traces flowing; drop it (normally at the end of `main`) to flush them.
#[must_use = "dropping the guard stops log delivery"]
#[derive(Debug)]
pub struct Guard {
    _file: WorkerGuard,
    #[cfg(feature = "otlp")]
    _otlp: Option<otlp::Provider>,
}

/// Builds the subscriber without installing it, so tests and embedders can scope it with
/// `tracing::subscriber::with_default`.
///
/// # Errors
/// An invalid filter, an unusable log directory, or OTLP requested without the feature.
pub fn subscriber(settings: &Settings) -> Result<(impl Subscriber + Send + Sync, Guard), Error> {
    let filter = settings.filter()?;

    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("aulo")
        .filename_suffix("log")
        .max_log_files(settings.max_files.max(1))
        .build(&settings.log_dir)?;
    // Non-blocking so a slow disk never stalls an agent turn.
    let (file_writer, file_guard) = tracing_appender::non_blocking(appender);

    let file = fmt::layer()
        .json()
        .with_ansi(false)
        .with_writer(Scrubbed(file_writer));
    // No ANSI: escape codes around a field name would hide `token=` from the redaction patterns.
    let stderr = settings.stderr.then(|| {
        fmt::layer()
            .with_ansi(false)
            .with_writer(Scrubbed(std::io::stderr))
    });

    let (extra, guard) = extra_layer(settings, file_guard)?;
    let subscriber = Registry::default()
        .with(extra)
        .with(filter)
        .with(file)
        .with(stderr);
    Ok((subscriber, guard))
}

/// Installs the global subscriber and returns the guard to keep alive.
///
/// # Errors
/// Everything [`subscriber`] reports, and a subscriber that is already installed.
pub fn init(settings: &Settings) -> Result<Guard, Error> {
    let (subscriber, guard) = subscriber(settings)?;
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(guard)
}

#[cfg(feature = "otlp")]
fn extra_layer(
    settings: &Settings,
    file: WorkerGuard,
) -> Result<(Option<BoxedLayer>, Guard), Error> {
    let (layer, provider) = match settings.otlp_endpoint.as_deref() {
        Some(endpoint) => {
            let (layer, provider) = otlp::layer(endpoint)?;
            (Some(layer), Some(provider))
        }
        None => (None, None),
    };
    Ok((
        layer,
        Guard {
            _file: file,
            _otlp: provider,
        },
    ))
}

#[cfg(not(feature = "otlp"))]
fn extra_layer(
    settings: &Settings,
    file: WorkerGuard,
) -> Result<(Option<BoxedLayer>, Guard), Error> {
    if settings.otlp_endpoint.is_some() {
        return Err(Error::OtlpUnavailable);
    }
    Ok((None, Guard { _file: file }))
}

#[cfg(test)]
mod tests;
