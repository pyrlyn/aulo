//! `config/default.toml` is documentation, so the only way to keep it honest is
//! to read it back: its settings must parse into `Config`, and a key added to
//! `Config` without a line in the file must fail here.
//!
//! File convention: `##` lines are prose, a single-`#` line is a commented-out
//! setting or table header. A trailing `# unset by default` marks a setting
//! (or a whole table) that shows an example, not the default.

// Helpers outside `#[test]` fns unwrap; clippy only exempts the test fns themselves.
#![allow(clippy::unwrap_used)]

use aulo_config::{Config, DEFAULT_TOML, schema_json};
use serde_json::Value;

const UNSET: &str = "# unset by default";

/// The file's settings with the leading `# ` removed. `with_examples` keeps
/// the lines (and tables) marked as unset by default.
fn uncommented(with_examples: bool) -> String {
    let mut in_unset_table = false;
    let mut out = String::new();
    for line in DEFAULT_TOML.lines() {
        let Some(code) = line.strip_prefix("# ") else {
            continue;
        };
        let unset = code.ends_with(UNSET);
        if code.starts_with('[') {
            in_unset_table = unset;
        }
        if with_examples || !(unset || in_unset_table) {
            out.push_str(code);
            out.push('\n');
        }
    }
    out
}

#[test]
fn shipped_file_is_inert_and_equals_the_defaults() {
    for line in DEFAULT_TOML.lines() {
        assert!(
            line.is_empty() || line.starts_with('#'),
            "an active line in default.toml: {line:?}"
        );
    }
    assert_eq!(
        toml::from_str::<Config>(DEFAULT_TOML).unwrap(),
        Config::default()
    );
}

#[test]
fn uncommenting_every_default_changes_nothing() {
    let parsed: Config = toml::from_str(&uncommented(false)).unwrap();
    assert_eq!(parsed, Config::default());
}

#[test]
fn the_examples_are_valid_settings() {
    let parsed: Config = toml::from_str(&uncommented(true)).unwrap();
    assert_ne!(parsed, Config::default());
    assert_eq!(parsed.providers.endpoints.len(), 1);
    assert_eq!(parsed.mcp.servers.len(), 2);
}

/// Every leaf key path of the schema; map entries are the wildcard `*`.
fn leaf_paths(schema: &Value, defs: &Value, path: &str, out: &mut Vec<String>) {
    if let Some(target) = schema["$ref"].as_str() {
        let name = target.trim_start_matches("#/$defs/");
        return leaf_paths(&defs[name], defs, path, out);
    }
    let join = |key: &str| {
        if path.is_empty() {
            key.to_owned()
        } else {
            format!("{path}.{key}")
        }
    };
    if let Some(properties) = schema["properties"].as_object() {
        for (key, child) in properties {
            leaf_paths(child, defs, &join(key), out);
        }
    } else if schema["additionalProperties"].is_object() {
        leaf_paths(&schema["additionalProperties"], defs, &join("*"), out);
    } else if let Some(variants) = schema["oneOf"].as_array() {
        // Only a tagged enum of tables has `properties` in its variants; a
        // plain string enum has constants and stays a leaf.
        if variants.iter().any(|v| v["properties"].is_object()) {
            for variant in variants {
                leaf_paths(variant, defs, path, out);
            }
        } else {
            out.push(path.to_owned());
        }
    } else {
        out.push(path.to_owned());
    }
}

/// `(table, key)` of every setting line, with commented tables tracked.
fn documented() -> Vec<(String, String)> {
    let mut table = String::new();
    let mut found = Vec::new();
    for line in DEFAULT_TOML.lines() {
        let Some(code) = line.strip_prefix("# ") else {
            continue;
        };
        if let Some(header) = code.strip_prefix('[') {
            table = header.split(']').next().unwrap_or_default().to_owned();
        } else if let Some((key, _)) = code.split_once(" = ") {
            found.push((table.clone(), key.to_owned()));
        }
    }
    found
}

fn table_matches(documented: &str, expected: &str) -> bool {
    let mut doc = documented.split('.');
    let mut exp = expected.split('.');
    loop {
        match (doc.next(), exp.next()) {
            (None, None) => return true,
            (Some(d), Some(e)) if e == "*" || d == e => {}
            _ => return false,
        }
    }
}

/// Schema key paths that no `(table, key)` line documents.
fn missing_from(lines: &[(String, String)]) -> Vec<String> {
    let schema: Value = serde_json::from_str(&schema_json().unwrap()).unwrap();
    let mut paths = Vec::new();
    leaf_paths(&schema, &schema["$defs"], "", &mut paths);
    assert!(
        paths.contains(&"mcp.servers.*.command".to_owned()),
        "{paths:?}"
    );
    paths
        .into_iter()
        .filter(|path| {
            let (table, key) = path.rsplit_once('.').unwrap_or(("", path));
            !lines
                .iter()
                .any(|(t, k)| k == key && table_matches(t, table))
        })
        .collect()
}

#[test]
fn every_config_key_is_in_the_file() {
    let missing = missing_from(&documented());
    assert!(
        missing.is_empty(),
        "keys missing from default.toml: {missing:?}"
    );
}

#[test]
fn the_completeness_check_catches_a_missing_key() {
    let without: Vec<_> = documented()
        .into_iter()
        .filter(|(_, key)| key != "approval_timeout_secs")
        .collect();
    assert_eq!(missing_from(&without), ["policy.approval_timeout_secs"]);
}
