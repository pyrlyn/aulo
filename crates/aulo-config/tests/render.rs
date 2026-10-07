// Helpers outside `#[test]` fns unwrap; clippy only exempts the test fns themselves.
#![allow(clippy::unwrap_used)]

use aulo_config::{Config, LoadOptions, Loaded, McpServer, Origin, mask};
use std::path::PathBuf;

fn loaded_from(toml: &str) -> Loaded {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), toml).unwrap();
    // Env layers from the host would change origins; the test file sets only
    // keys the host is not expected to override.
    Config::load(&LoadOptions {
        home: Some(dir.path().to_owned()),
        ..LoadOptions::default()
    })
    .unwrap()
}

#[test]
fn mask_hides_url_userinfo_and_credential_pairs() {
    assert_eq!(
        mask("https://bob:hunter2@host/v1"),
        "https://[REDACTED]@host/v1"
    );
    assert_eq!(
        mask("http://host/v1?api_key=abcdef"),
        "http://host/v1?api_key=[REDACTED]"
    );
    assert_eq!(mask("--token=abc"), "--token=[REDACTED]");
    assert_eq!(mask("sk-0123456789abcdef0123"), "[REDACTED]");
    assert_eq!(mask("http://host:80/path@x"), "http://host:80/path@x");
    assert_eq!(mask("plain text"), "plain text");
}

#[test]
fn toml_output_masks_every_field_that_could_carry_a_secret() {
    let mut config = Config::default();
    config.providers.endpoints.insert(
        "x".into(),
        toml::from_str(
            r#"kind = "openai-compatible"
base_url = "https://user:s3cret@host/v1?api_key=abc123""#,
        )
        .unwrap(),
    );
    config.mcp.servers.insert(
        "s".into(),
        McpServer::Stdio {
            command: "srv".into(),
            args: vec!["--password=hunter2".into()],
        },
    );
    config.mcp.servers.insert(
        "h".into(),
        McpServer::Http {
            url: "https://u:p4ss@host/mcp".into(),
        },
    );
    config
        .policy
        .allow
        .push("Bash(curl -H 'Authorization: Bearer abcdefgh12345678')".into());
    config.plugins.dirs.push(PathBuf::from("/plugins"));

    let out = config.to_toml_redacted().unwrap();
    for secret in ["s3cret", "abc123", "hunter2", "p4ss", "abcdefgh12345678"] {
        assert!(!out.contains(secret), "{secret} leaked:\n{out}");
    }
    assert!(out.contains("[REDACTED]"));
    assert!(out.contains("/plugins"));
}

#[test]
fn plain_toml_output_round_trips_for_a_config_without_secrets() {
    let loaded = loaded_from("[voice]\nlisten = \"wake-word\"\n[plugins]\ndisabled = [\"a\"]\n");
    let out = loaded.config.to_toml_redacted().unwrap();
    assert_eq!(toml::from_str::<Config>(&out).unwrap(), loaded.config);
}

#[test]
fn origin_lines_give_each_leaf_its_layer_and_mask_values() {
    let loaded = loaded_from(
        "[providers.endpoints.x]\nkind = \"openai-compatible\"\nbase_url = \"https://u:pw@h/\"\n[policy]\napproval_timeout_secs = 5\n",
    );
    let out = loaded.render_origins().unwrap();
    let file = match loaded.provenance.get("policy.approval_timeout_secs") {
        Some(Origin::File(path)) => path.display().to_string(),
        other => panic!("unexpected origin {other:?}"),
    };
    assert!(
        out.contains(&format!("policy.approval_timeout_secs = 5  # {file}\n")),
        "{out}"
    );
    assert!(
        out.contains("voice.listen = \"push-to-talk\"  # built-in defaults\n"),
        "{out}"
    );
    assert!(
        out.contains("providers.endpoints.x.base_url = \"https://[REDACTED]@h/\""),
        "{out}"
    );
    assert!(!out.contains("pw@"), "{out}");
}
