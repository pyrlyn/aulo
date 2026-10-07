// Helpers outside `#[test]` fns unwrap; clippy only exempts the test fns themselves.
#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::Path;

use aulo_telemetry::{Error, Settings, subscriber};

fn settings(dir: &Path, level: &str) -> Settings {
    let mut settings = Settings::new(level, dir);
    // Tests must not depend on the developer's AULO_LOG.
    settings.filter_override = None;
    settings.stderr = false;
    settings
}

/// Runs `emit` under a fresh subscriber and returns the log file's text once flushed.
fn logged(settings: &Settings, emit: impl FnOnce()) -> String {
    let (subscriber, guard) = subscriber(settings).unwrap();
    tracing::subscriber::with_default(subscriber, emit);
    drop(guard);
    let mut text = String::new();
    for entry in fs::read_dir(&settings.log_dir).unwrap() {
        text.push_str(&fs::read_to_string(entry.unwrap().path()).unwrap());
    }
    text
}

#[test]
fn writes_a_rotating_json_file_named_by_date() {
    let dir = tempfile::tempdir().unwrap();
    let log_dir = dir.path().join("nested").join("logs");
    let text = logged(&settings(&log_dir, "info"), || {
        tracing::info!(turn = 3, "hello")
    });

    let names: Vec<_> = fs::read_dir(&log_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(
        names[0].starts_with("aulo.") && names[0].ends_with(".log"),
        "{names:?}"
    );

    let record: serde_json::Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(record["fields"]["message"], "hello");
    assert_eq!(record["fields"]["turn"], 3);
    assert_eq!(record["level"], "INFO");
}

#[test]
fn secrets_never_reach_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let text = logged(&settings(dir.path(), "info"), || {
        let span = tracing::info_span!("call", api_key = "sk-live-0123456789abcdefXYZ");
        let _entered = span.enter();
        tracing::info!(
            password = "hunter2",
            user = "ada",
            "got Bearer abcdef0123456789"
        );
        tracing::warn!(note = "key sk-ant-api03-abcdefghijklmnop");
    });
    for leaked in ["hunter2", "abcdef0123456789", "sk-live", "sk-ant-api03"] {
        assert!(!text.contains(leaked), "{leaked} leaked: {text}");
    }
    assert!(text.contains("ada"), "{text}");
    assert!(text.contains("[REDACTED]"), "{text}");
}

#[test]
fn config_level_filters_events() {
    let dir = tempfile::tempdir().unwrap();
    let text = logged(&settings(dir.path(), "warn"), || {
        tracing::info!("quiet");
        tracing::warn!("loud");
    });
    assert!(text.contains("loud") && !text.contains("quiet"), "{text}");
}

#[test]
fn override_beats_the_config_level() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = settings(dir.path(), "error");
    settings.filter_override = Some("debug".into());
    let text = logged(&settings, || tracing::debug!("detail"));
    assert!(text.contains("detail"), "{text}");
}

#[test]
fn invalid_filter_is_an_error_naming_the_directive() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = settings(dir.path(), "info");
    settings.filter_override = Some("aulo=notalevel".into());
    match subscriber(&settings) {
        Err(Error::Filter { directive, .. }) => assert_eq!(directive, "aulo=notalevel"),
        other => panic!("expected a filter error, got {:?}", other.err()),
    }
}

#[cfg(not(feature = "otlp"))]
#[test]
fn otlp_without_the_feature_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = settings(dir.path(), "info");
    settings.otlp_endpoint = Some("http://localhost:4318/v1/traces".into());
    assert!(matches!(subscriber(&settings), Err(Error::OtlpUnavailable)));
}

#[cfg(feature = "otlp")]
#[test]
fn otlp_layer_builds_and_shuts_down_with_the_guard() {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = settings(dir.path(), "info");
    // Nothing listens here; export fails quietly and must not block or panic the process.
    settings.otlp_endpoint = Some("http://127.0.0.1:9/v1/traces".into());
    let text = logged(&settings, || {
        let span = tracing::info_span!("turn", token = "secret-value");
        let _entered = span.enter();
        tracing::info!("inside");
    });
    assert!(
        text.contains("inside") && !text.contains("secret-value"),
        "{text}"
    );
}
