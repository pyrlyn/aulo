use aulo_telemetry::redact::{REDACTED, is_secret_name, scrub_json, scrub_line, scrub_text};
use serde_json::json;

#[test]
fn patterns_compile_and_leave_plain_text_alone() {
    // A pattern that failed to compile would mask everything, so plain text surviving proves both built.
    assert_eq!(
        scrub_text("started turn 4 for bot b1"),
        "started turn 4 for bot b1"
    );
}

#[test]
fn names_that_look_secret_are_detected() {
    for name in [
        "token",
        "API_KEY",
        "access_token",
        "Authorization",
        "db.password",
        "client_secret",
    ] {
        assert!(is_secret_name(name), "{name}");
    }
    for name in ["message", "level", "target", "turn_id", "bot"] {
        assert!(!is_secret_name(name), "{name}");
    }
}

#[test]
fn bearer_and_provider_keys_are_masked_anywhere() {
    for secret in [
        "Bearer abcdef0123456789",
        "sk-ant-api03-abcdefghijklmnop",
        "ghp_abcdefghijklmnopqrstuvwxyz",
        "github_pat_11ABCDEFG0123456789abc",
        "xoxb-1234567890-abcdef",
        "AKIAIOSFODNN7EXAMPLE",
        "eyJhbGciOiJIUzI1.eyJzdWIiOiIxMjM0.SflKxwRJSMeKKF2QT4",
    ] {
        let input = format!("calling upstream with {secret} now");
        let out = scrub_text(&input);
        assert!(!out.contains(secret), "{out}");
        assert!(out.contains(REDACTED), "{out}");
        assert!(out.starts_with("calling upstream with "), "{out}");
    }
}

#[test]
fn secret_named_pairs_lose_only_the_value() {
    assert_eq!(
        scrub_text("connect host=db password=hunter2 port=5432"),
        "connect host=db password=[REDACTED] port=5432"
    );
    assert_eq!(
        scrub_text(r#"login user=a token="two words" ok"#),
        r#"login user=a token=[REDACTED] ok"#
    );
    assert_eq!(
        scrub_text("Authorization: Bearer abcdef0123456789"),
        "Authorization: [REDACTED]"
    );
}

#[test]
fn scrubbing_is_idempotent() {
    let once = scrub_text("password=hunter2 Bearer abcdef0123456789").into_owned();
    assert_eq!(scrub_text(&once), once);
}

#[test]
fn json_masks_secret_keys_of_any_type_and_nested_strings() {
    let mut record = json!({
        "level": "INFO",
        "fields": {
            "message": "sent Bearer abcdef0123456789",
            "token": {"inner": 1},
            "api_key": 42,
            "turn": 7
        },
        "spans": [{"name": "turn", "password": "p"}]
    });
    scrub_json(&mut record);
    assert_eq!(
        record,
        json!({
            "level": "INFO",
            "fields": {
                "message": "sent [REDACTED]",
                "token": "[REDACTED]",
                "api_key": "[REDACTED]",
                "turn": 7
            },
            "spans": [{"name": "turn", "password": "[REDACTED]"}]
        })
    );
}

#[test]
fn line_is_masked_as_json_or_as_text() {
    let json_line = scrub_line(r#"{"fields":{"secret":"s"}}"#);
    assert_eq!(json_line, r#"{"fields":{"secret":"[REDACTED]"}}"#);
    let text_line = scrub_line("2026-01-01T00:00:00Z  INFO login: user=a secret=s");
    assert!(text_line.ends_with("secret=[REDACTED]"), "{text_line}");
}
