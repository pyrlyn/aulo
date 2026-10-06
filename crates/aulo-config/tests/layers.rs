//! Every test that loads config runs inside `Jail`, which holds a global lock
//! while it changes env vars and the working directory, so they cannot race.

// `Jail::expect_with` fixes the closure's error type to the large `figment::Error`.
// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(clippy::result_large_err, clippy::unwrap_used)]

use aulo_config::{BargeIn, Config, ConfigError, LoadOptions, McpServer, Origin, ProviderKind};
use figment::Jail;
use serde_json::json;

fn opts(jail: &Jail) -> LoadOptions {
    let dir = jail.directory();
    LoadOptions {
        home: Some(dir.join("home")),
        project_dir: Some(dir.join("project")),
        overrides: None,
    }
}

fn write(jail: &Jail, rel: &str, body: &str) {
    let path = jail.directory().join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn load(options: &LoadOptions) -> Result<aulo_config::Loaded, figment::Error> {
    Config::load(options).map_err(|e| figment::Error::from(e.to_string()))
}

fn load_err(options: &LoadOptions) -> ConfigError {
    Config::load(options).unwrap_err()
}

#[test]
fn defaults_only() {
    Jail::expect_with(|jail| {
        let loaded = load(&opts(jail))?;
        assert_eq!(loaded.config, Config::default());
        assert_eq!(
            loaded.provenance.get("voice.barge_in"),
            Some(&Origin::Default)
        );
        Ok(())
    });
}

#[test]
fn layers_override_in_order_with_provenance() {
    Jail::expect_with(|jail| {
        write(
            jail,
            "home/config.toml",
            "[voice]\nbarge_in = \"continue\"\nlisten = \"wake-word\"\n[policy]\napproval_timeout_secs = 30\nallow = [\"a\"]\n",
        );
        write(
            jail,
            "project/.aulo.toml",
            "[voice]\nlisten = \"push-to-talk\"\n[policy]\nallow = [\"b\"]\n",
        );
        jail.set_env("AULO_POLICY__APPROVAL_TIMEOUT_SECS", "45");
        let mut options = opts(jail);
        options.overrides = Some(json!({ "voice": { "stt": { "engine": "whisper" } } }));

        let loaded = load(&options)?;
        let (c, p) = (&loaded.config, &loaded.provenance);
        let home = jail.directory().join("home/config.toml");
        let project = jail.directory().join("project/.aulo.toml");

        assert_eq!(c.voice.barge_in, BargeIn::Continue);
        assert_eq!(p.get("voice.barge_in"), Some(&Origin::File(home)));
        assert_eq!(p.get("voice.listen"), Some(&Origin::File(project.clone())));
        // Arrays are replaced by the higher layer, not merged.
        assert_eq!(c.policy.allow, ["b"]);
        assert_eq!(p.get("policy.allow"), Some(&Origin::File(project)));
        assert_eq!(c.policy.approval_timeout_secs.get(), 45);
        assert_eq!(
            p.get("policy.approval_timeout_secs"),
            Some(&Origin::Env("AULO_POLICY__APPROVAL_TIMEOUT_SECS".into()))
        );
        assert_eq!(c.voice.stt.engine.as_deref(), Some("whisper"));
        assert_eq!(p.get("voice.stt.engine"), Some(&Origin::Override));
        assert_eq!(p.get("daemon.tls_cert"), Some(&Origin::Default));
        Ok(())
    });
}

#[test]
fn aulo_home_env_selects_the_global_file() {
    Jail::expect_with(|jail| {
        write(
            jail,
            "alt/config.toml",
            "[voice]\nbarge_in = \"continue\"\n",
        );
        jail.set_env("AULO_HOME", jail.directory().join("alt").display());
        let options = LoadOptions::default();
        assert_eq!(load(&options)?.config.voice.barge_in, BargeIn::Continue);
        Ok(())
    });
}

#[test]
fn unknown_key_names_the_file() {
    Jail::expect_with(|jail| {
        write(jail, "project/.aulo.toml", "[voice]\nbogus = 1\n");
        let err = load_err(&opts(jail));
        let file = jail.directory().join("project/.aulo.toml");
        assert_eq!(err.origin, Some(Origin::File(file.clone())));
        assert!(
            err.to_string().contains(&file.display().to_string()),
            "{err}"
        );
        assert!(err.to_string().contains("bogus"), "{err}");
        Ok(())
    });
}

#[test]
fn unknown_top_level_section_is_rejected() {
    Jail::expect_with(|jail| {
        write(jail, "home/config.toml", "[nope]\nx = 1\n");
        let err = load_err(&opts(jail));
        assert!(matches!(err.origin, Some(Origin::File(_))), "{err}");
        Ok(())
    });
}

#[test]
fn syntax_error_names_the_file() {
    Jail::expect_with(|jail| {
        write(jail, "home/config.toml", "[voice\n");
        let err = load_err(&opts(jail));
        let file = jail.directory().join("home/config.toml");
        assert!(
            err.to_string().contains(&file.display().to_string()),
            "{err}"
        );
        Ok(())
    });
}

#[test]
fn wrong_type_names_the_file() {
    Jail::expect_with(|jail| {
        write(
            jail,
            "home/config.toml",
            "[policy]\napproval_timeout_secs = 0\n",
        );
        let err = load_err(&opts(jail));
        assert!(
            matches!(&err.origin, Some(Origin::File(p)) if p.ends_with("config.toml")),
            "{err}"
        );
        Ok(())
    });
}

#[test]
fn unknown_env_key_names_the_variable() {
    Jail::expect_with(|jail| {
        jail.set_env("AULO_VOICE__BOGUS", "1");
        let err = load_err(&opts(jail));
        assert!(err.to_string().contains("AULO_VOICE__BOGUS"), "{err}");
        Ok(())
    });
}

#[test]
fn non_config_env_vars_are_ignored() {
    Jail::expect_with(|jail| {
        jail.set_env("AULO_UPDATE_SCHEMA", "1");
        jail.set_env("AULO_HOME", jail.directory().join("home").display());
        load(&opts(jail))?;
        Ok(())
    });
}

#[test]
fn providers_and_mcp_parse() {
    Jail::expect_with(|jail| {
        write(
            jail,
            "home/config.toml",
            r#"
[providers.endpoints.local]
kind = "ollama"
[providers.tiers.main]
provider = "local"
model = "qwen"
[mcp.servers.fs]
transport = "stdio"
command = "mcp-fs"
args = ["--ro"]
[mcp.servers.web]
transport = "http"
url = "https://example.test/mcp"
[daemon]
listen = "127.0.0.1:7443"
"#,
        );
        let c = load(&opts(jail))?.config;
        assert_eq!(c.providers.endpoints["local"].kind, ProviderKind::Ollama);
        assert_eq!(
            c.providers.tiers.main.as_ref().map(|t| t.model.as_str()),
            Some("qwen")
        );
        assert_eq!(
            c.mcp.servers["fs"],
            McpServer::Stdio {
                command: "mcp-fs".into(),
                args: vec!["--ro".into()]
            }
        );
        assert_eq!(c.daemon.listen.map(|a| a.port()), Some(7443));
        Ok(())
    });
}

#[test]
fn mcp_server_rejects_unknown_field() {
    Jail::expect_with(|jail| {
        write(
            jail,
            "home/config.toml",
            "[mcp.servers.x]\ntransport = \"http\"\nurl = \"u\"\ncommand = \"c\"\n",
        );
        let err = load_err(&opts(jail));
        assert!(matches!(err.origin, Some(Origin::File(_))), "{err}");
        Ok(())
    });
}
