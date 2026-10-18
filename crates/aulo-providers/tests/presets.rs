//! Fixture configs: every preset builds, and a bad section only warns.
//!
//! Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(clippy::panic, clippy::unwrap_used)]

use std::path::Path;

use aulo_config::{Config, LoadOptions, ProviderKind};
use aulo_providers::{
    ANTHROPIC_CONTEXT_TOKENS, DEFAULT_CONTEXT_TOKENS, ProviderId, ProviderSet, SkipReason, Wire,
    build_providers,
};

fn load_fixture(name: &str) -> aulo_config::ProvidersConfig {
    // `AULO_*` is a higher config layer than the file. Changing the process
    // environment is unsafe, so an ambient variable would override the fixture;
    // fail with its name instead of building the wrong providers.
    if let Some((key, _)) = std::env::vars().find(|(key, _)| key.starts_with("AULO_")) {
        panic!("unset {key} before this test; it overrides the fixture");
    }

    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::copy(fixture, dir.path().join("config.toml")).unwrap();
    Config::load(&LoadOptions {
        home: Some(dir.path().to_path_buf()),
        project_dir: Some(project),
        overrides: None,
    })
    .unwrap()
    .config
    .providers
}

fn built(name: &str) -> ProviderSet {
    build_providers(&load_fixture(name))
}

#[test]
fn every_preset_builds_from_the_fixture() {
    let set = built("presets.toml");
    assert!(
        set.warnings.is_empty(),
        "unexpected warnings: {:?}",
        set.warnings
    );

    let expect = [
        (
            "runa",
            ProviderKind::Runa,
            Wire::Chat,
            "http://127.0.0.1:8080/v1",
            ProviderId::Local,
            DEFAULT_CONTEXT_TOKENS,
        ),
        (
            "ollama",
            ProviderKind::Ollama,
            Wire::Chat,
            "http://127.0.0.1:11434/v1",
            ProviderId::Local,
            DEFAULT_CONTEXT_TOKENS,
        ),
        (
            "lm-studio",
            ProviderKind::LmStudio,
            Wire::Chat,
            "http://127.0.0.1:1234/v1",
            ProviderId::Local,
            DEFAULT_CONTEXT_TOKENS,
        ),
        (
            "llama-cpp",
            ProviderKind::LlamaCpp,
            Wire::Chat,
            "http://127.0.0.1:8080/v1",
            ProviderId::Local,
            DEFAULT_CONTEXT_TOKENS,
        ),
        (
            "openai",
            ProviderKind::Openai,
            Wire::Responses,
            "https://api.openai.com/v1",
            ProviderId::OpenAi,
            DEFAULT_CONTEXT_TOKENS,
        ),
        (
            "anthropic",
            ProviderKind::Anthropic,
            Wire::Anthropic,
            "https://api.anthropic.com",
            ProviderId::Anthropic,
            ANTHROPIC_CONTEXT_TOKENS,
        ),
        (
            "gemini",
            ProviderKind::Gemini,
            Wire::Chat,
            "https://generativelanguage.googleapis.com/v1beta/openai",
            ProviderId::Local,
            DEFAULT_CONTEXT_TOKENS,
        ),
        (
            "xai",
            ProviderKind::Xai,
            Wire::Chat,
            "https://api.x.ai/v1",
            ProviderId::Local,
            DEFAULT_CONTEXT_TOKENS,
        ),
        (
            "openrouter",
            ProviderKind::Openrouter,
            Wire::Chat,
            "https://openrouter.ai/api/v1",
            ProviderId::Local,
            DEFAULT_CONTEXT_TOKENS,
        ),
        (
            "deepseek",
            ProviderKind::Deepseek,
            Wire::Chat,
            "https://api.deepseek.com/v1",
            ProviderId::Local,
            DEFAULT_CONTEXT_TOKENS,
        ),
        (
            "vllm",
            ProviderKind::OpenaiCompatible,
            Wire::Chat,
            "http://127.0.0.1:8000/v1",
            ProviderId::Local,
            DEFAULT_CONTEXT_TOKENS,
        ),
        (
            "custom",
            ProviderKind::OpenaiCompatible,
            Wire::Chat,
            "https://llm.example.com/v1",
            ProviderId::Local,
            DEFAULT_CONTEXT_TOKENS,
        ),
    ];

    assert_eq!(set.endpoints.len(), expect.len());
    for (name, kind, wire, base, id, context) in expect {
        let endpoint = set
            .endpoints
            .get(name)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(endpoint.kind, kind, "{name}");
        assert_eq!(endpoint.wire, wire, "{name}");
        assert_eq!(endpoint.base_url, base, "{name}");
        assert_eq!(endpoint.as_provider().id(), id, "{name}");
        assert_eq!(
            endpoint.as_provider().capabilities().max_context,
            context,
            "{name}"
        );
    }
}

#[test]
fn a_bad_section_only_warns() {
    let set = built("bad.toml");
    let names: Vec<_> = set.endpoints.keys().cloned().collect();
    assert_eq!(names, vec!["ollama".to_owned()]);
    assert_eq!(
        set.endpoints["ollama"].base_url,
        "http://127.0.0.1:11434/v1"
    );

    let reasons: Vec<_> = set
        .warnings
        .iter()
        .map(|warning| (warning.section.as_str(), warning.reason.clone()))
        .collect();
    assert_eq!(
        reasons,
        vec![
            ("providers.endpoints.custom", SkipReason::MissingBaseUrl,),
            (
                "providers.endpoints.embedded",
                SkipReason::EmbeddedCredentials,
            ),
            ("providers.endpoints.weird", SkipReason::BadBaseUrl),
            (
                "providers.tiers.main",
                SkipReason::UnknownEndpoint {
                    endpoint: "custom".into(),
                },
            ),
            (
                "providers.tiers.cheap",
                SkipReason::UnknownEndpoint {
                    endpoint: "missing".into(),
                },
            ),
        ]
    );

    let rendered = set
        .warnings
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!rendered.contains("s3cr3t-token"), "{rendered}");
    assert!(!rendered.contains("api.openai.com"), "{rendered}");
}

#[test]
fn an_empty_providers_section_builds_nothing() {
    let set = build_providers(&aulo_config::ProvidersConfig::default());
    assert!(set.endpoints.is_empty());
    assert!(set.warnings.is_empty());
}
