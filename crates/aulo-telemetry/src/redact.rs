//! Secret masking shared by every log sink.
//!
//! Redaction works on the finished line, not on tracing fields, so one rule set covers events,
//! span fields, JSON and the human format alike, and a field added later cannot bypass it.

use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

/// What a masked value is replaced with.
pub const REDACTED: &str = "[REDACTED]";

/// Substrings that mark a field or `name=value` pair as secret, matched case-insensitively.
/// Matching a substring rather than a whole word over-masks `cache_key`, which is the safe side.
const SECRET_NAME_PARTS: &[&str] = &[
    "token",
    "key",
    "password",
    "passwd",
    "secret",
    "authorization",
    "credential",
    "cookie",
];

/// Credential shapes that are masked wherever they appear, even under an innocent field name:
/// bearer headers, provider API keys, forge tokens, aulod API tokens, cloud keys and JWTs.
const VALUE_PATTERN: &str = concat!(
    r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{8,}",
    r"|\b(?:sk|pk|rk)-[A-Za-z0-9_-]{16,}",
    r"|\bgh[pousr]_[A-Za-z0-9]{20,}",
    r"|\bgithub_pat_[A-Za-z0-9_]{20,}",
    r"|\baulo_[0-9a-f]{16,}",
    r"|\bxox[abprs]-[A-Za-z0-9-]{10,}",
    r"|\bAKIA[0-9A-Z]{16}\b",
    r"|\bAIza[0-9A-Za-z_-]{35}",
    r"|\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
);

/// `name=value` or `name: value` where the name looks secret; the value is quoted or one token.
const PAIR_PATTERN: &str = concat!(
    r"(?i)\b([\w.-]*(?:token|key|password|passwd|secret|authorization|credential|cookie)[\w.-]*)",
    r#"(\s*[=:]\s*)("(?:[^"\\]|\\.)*"|'[^']*'|[^\s,;}]+)"#,
);

/// Keeps the name and separator, masks the value.
const PAIR_REPLACEMENT: &str = "${1}${2}[REDACTED]";

struct Patterns {
    value: Regex,
    pair: Regex,
}

// A pattern that fails to compile must not turn into a panic or a silent pass-through, so the
// failure is kept and `scrub_text` masks everything instead. A unit test pins that both compile.
static PATTERNS: LazyLock<Option<Patterns>> = LazyLock::new(|| {
    Some(Patterns {
        value: Regex::new(VALUE_PATTERN).ok()?,
        pair: Regex::new(PAIR_PATTERN).ok()?,
    })
});

/// Whether a field or key name marks its value as secret.
#[must_use]
pub fn is_secret_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    SECRET_NAME_PARTS.iter().any(|part| name.contains(part))
}

/// `text` with credential-shaped values and the values of secret-named pairs masked.
///
/// Fails closed: if the patterns are unavailable, the whole text is masked.
#[must_use]
pub fn scrub_text(text: &str) -> Cow<'_, str> {
    let Some(patterns) = PATTERNS.as_ref() else {
        return Cow::Borrowed(REDACTED);
    };
    // Values first, so `Authorization: Bearer abc` is already one token when the pair rule runs.
    let after_values = patterns.value.replace_all(text, REDACTED);
    if let Cow::Owned(masked) = patterns.pair.replace_all(&after_values, PAIR_REPLACEMENT) {
        return Cow::Owned(masked);
    }
    after_values
}

/// Masks a parsed JSON log record in place: secret-named keys lose their whole value (of any
/// type), every other string goes through [`scrub_text`].
pub fn scrub_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map.iter_mut() {
                if is_secret_name(key) {
                    *inner = Value::String(REDACTED.to_owned());
                } else {
                    scrub_json(inner);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(scrub_json),
        Value::String(text) => {
            if let Cow::Owned(masked) = scrub_text(text) {
                *text = masked;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// One log line without its newline: a JSON object is parsed and masked structurally, anything
/// else (the human stderr format) is masked as text.
#[must_use]
pub fn scrub_line(line: &str) -> String {
    match serde_json::from_str::<Value>(line) {
        Ok(mut record @ Value::Object(_)) => {
            scrub_json(&mut record);
            record.to_string()
        }
        _ => scrub_text(line).into_owned(),
    }
}
