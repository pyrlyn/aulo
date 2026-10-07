//! Rendering of an effective config for people: TOML text or one
//! `key = value  # origin` line per leaf, with secrets masked either way.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use aulo_telemetry::redact::{REDACTED, scrub_text};
use toml::Value;

use crate::error::ConfigError;
use crate::load::{Loaded, Provenance};
use crate::model::Config;

impl Config {
    /// The config as TOML with every string value masked by [`mask`].
    pub fn to_toml_redacted(&self) -> Result<String, ConfigError> {
        toml::to_string(&redacted_value(self)?).map_err(render_error)
    }
}

impl Loaded {
    /// One `key = value  # origin` line per leaf, keys sorted, values masked.
    pub fn render_origins(&self) -> Result<String, ConfigError> {
        let mut leaves = BTreeMap::new();
        flatten(&redacted_value(&self.config)?, String::new(), &mut leaves);
        let mut out = String::new();
        for (key, value) in leaves {
            // A leaf without provenance cannot happen for a loaded config; a
            // bare line is better than hiding the value if it ever does.
            match origin_text(&self.provenance, &key) {
                Some(origin) => writeln!(out, "{key} = {value}  # {origin}"),
                None => writeln!(out, "{key} = {value}"),
            }
            .map_err(|e| render_error(e.to_string()))?;
        }
        Ok(out)
    }
}

/// Masks credential-shaped text (the log redaction rules, so there is one rule
/// set) and the userinfo of any URL, which the log rules do not know about.
pub fn mask(text: &str) -> String {
    let scrubbed = scrub_text(text);
    let mut out = String::with_capacity(scrubbed.len());
    let mut rest = scrubbed.as_ref();
    while let Some(i) = rest.find("://") {
        let (head, tail) = rest.split_at(i + 3);
        out.push_str(head);
        let authority_len = tail.find(['/', '?', '#']).unwrap_or(tail.len());
        let (authority, after) = tail.split_at(authority_len);
        match authority.rfind('@') {
            Some(at) => {
                out.push_str(REDACTED);
                out.push_str(&authority[at..]);
            }
            None => out.push_str(authority),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

fn redacted_value(config: &Config) -> Result<Value, ConfigError> {
    let mut value = Value::try_from(config).map_err(render_error)?;
    mask_strings(&mut value);
    Ok(value)
}

// Walks the serialized form, not the typed fields, so a field added to
// `Config` later is masked without anyone remembering to list it. Table keys
// are user-chosen names, not values, and stay readable.
fn mask_strings(value: &mut Value) {
    match value {
        Value::String(text) => *text = mask(text),
        Value::Array(items) => items.iter_mut().for_each(mask_strings),
        Value::Table(table) => table.iter_mut().for_each(|(_, v)| mask_strings(v)),
        _ => {}
    }
}

fn flatten(value: &Value, path: String, out: &mut BTreeMap<String, String>) {
    match value {
        Value::Table(table) => {
            for (key, child) in table {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                flatten(child, child_path, out);
            }
        }
        leaf => {
            out.insert(path, leaf.to_string());
        }
    }
}

fn origin_text(provenance: &Provenance, key: &str) -> Option<String> {
    provenance.get(key).map(ToString::to_string)
}

fn render_error(error: impl ToString) -> ConfigError {
    ConfigError {
        origin: None,
        message: format!("cannot render: {}", error.to_string()),
    }
}
