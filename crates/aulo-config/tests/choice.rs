//! `[voice.*]` sections become validated engine choices; bad values are
//! config errors naming the key and the layer that set it.

// `Jail::expect_with` fixes the closure's error type to the large `figment::Error`.
#![allow(clippy::result_large_err)]

use aulo_config::{Config, EngineConfig, LoadOptions, Origin};
use figment::Jail;

#[test]
fn a_valid_section_becomes_an_engine_choice() {
    let section = EngineConfig {
        engine: Some("sherpa-kokoro".into()),
        model: Some("kokoro-v1".into()),
        voice: Some("af_heart".into()),
        rate: Some(1.25),
        fallback: vec!["system".into()],
    };
    let choice = section.choice("voice.tts", None).unwrap();
    assert_eq!(choice.engine.unwrap().as_str(), "sherpa-kokoro");
    assert_eq!(choice.rate.unwrap().get(), 1.25);
    assert_eq!(choice.fallback[0].as_str(), "system");
    assert_eq!(choice.voice.as_deref(), Some("af_heart"));
    assert_eq!(
        EngineConfig::default()
            .choice("voice.stt", None)
            .unwrap()
            .engine,
        None
    );
}

#[test]
fn bad_engine_ids_are_config_errors() {
    for section in [
        EngineConfig {
            engine: Some("Kokoro".into()),
            ..EngineConfig::default()
        },
        EngineConfig {
            fallback: vec!["ok".into(), "not ok".into()],
            ..EngineConfig::default()
        },
    ] {
        let error = section.choice("voice.tts", None).unwrap_err();
        assert!(error.message.contains("invalid engine id"), "{error}");
    }
}

#[test]
fn out_of_range_rate_is_a_config_error_from_its_layer() {
    Jail::expect_with(|jail| {
        let home = jail.directory().join("home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("config.toml"), "[voice.tts]\nrate = 3.0\n").unwrap();
        let loaded = Config::load(&LoadOptions {
            home: Some(home.clone()),
            ..LoadOptions::default()
        })
        .unwrap();
        let error = loaded
            .config
            .voice
            .tts
            .choice("voice.tts", Some(&loaded.provenance))
            .unwrap_err();
        assert_eq!(error.origin, Some(Origin::File(home.join("config.toml"))));
        assert_eq!(
            error.message,
            "invalid speech rate: must be within 0.5..=2.0 for key `voice.tts.rate`"
        );
        Ok(())
    });
}
