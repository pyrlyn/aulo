//! Registry, selection, fallback and switching, driven with the fake engines
//! from `aulo-speech`'s testkit.

// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(clippy::unwrap_used, clippy::panic)]

use aulo_speech::testkit::{FAKE_VOICE, FakeStt, FakeTts};
use aulo_speech::{
    EngineChoice, EngineId, EngineSpec, SpeechError, SpeechRate, SttEngine, TtsEngine, TtsPoll,
    TurnId,
};
use aulo_types::{AuloEvent, NoticeLevel};
use aulo_voice::{
    ActiveStt, ActiveTts, Overrides, RegistryError, SttRegistry, TtsRegistry, candidates,
};

const SAMPLES: usize = 320;

fn id(id: &str) -> EngineId {
    EngineId::new(id).unwrap()
}

fn choice(engine: &str) -> EngineChoice {
    EngineChoice {
        engine: Some(id(engine)),
        ..EngineChoice::default()
    }
}

fn with_fallback(engine: &str, fallback: &[&str]) -> EngineChoice {
    EngineChoice {
        fallback: fallback.iter().map(|f| id(f)).collect(),
        ..choice(engine)
    }
}

fn ids(specs: &[EngineSpec]) -> Vec<&str> {
    specs.iter().map(|s| s.engine.as_str()).collect()
}

/// `alpha` and `beta` work, `broken` starts and then reports itself
/// unavailable, `missing` cannot even be built.
fn tts_registry() -> TtsRegistry {
    let mut registry = TtsRegistry::new();
    for name in ["alpha", "beta"] {
        registry
            .register(id(name), move |_| {
                Ok(Box::new(FakeTts::new(id(name))) as Box<dyn TtsEngine>)
            })
            .unwrap();
    }
    registry
        .register(id("broken"), |_| {
            Ok(Box::new(FakeTts::failing(
                id("broken"),
                SpeechError::unavailable("plugin process exited"),
            )) as Box<dyn TtsEngine>)
        })
        .unwrap();
    registry
        .register(id("missing"), |_| {
            Err(SpeechError::unavailable("model file not found"))
        })
        .unwrap();
    registry
}

fn speak(tts: &mut ActiveTts, registry: &TtsRegistry, notices: &mut Vec<AuloEvent>) -> String {
    tts.begin(
        registry,
        &mut |n| notices.push(n),
        TurnId::new(),
        Some("en"),
    )
    .unwrap();
    tts.active().unwrap().to_string()
}

fn notice_sources(notices: &[AuloEvent]) -> Vec<&str> {
    notices
        .iter()
        .map(|n| match n {
            AuloEvent::Notice {
                level: NoticeLevel::Warn,
                source: Some(source),
                ..
            } => source.as_str(),
            other => panic!("not a warning notice: {other:?}"),
        })
        .collect()
}

#[test]
fn chat_overrides_bot_which_overrides_config() {
    let config = EngineChoice {
        voice: Some("config-voice".into()),
        rate: Some(SpeechRate::new(1.2).unwrap()),
        ..with_fallback("alpha", &["gamma"])
    };
    let bot = EngineChoice {
        model: Some("bot-model".into()),
        ..choice("beta")
    };
    let chat = choice("delta");
    let specs = candidates(
        &config,
        Overrides {
            bot: Some(&bot),
            chat: Some(&chat),
        },
    );
    assert_eq!(ids(&specs), ["delta", "beta", "alpha", "gamma"]);
    // Model and voice stay with the layer that chose the engine.
    assert_eq!(specs[0].model, None);
    assert_eq!(specs[1].model.as_deref(), Some("bot-model"));
    assert_eq!(specs[2].voice.as_deref(), Some("config-voice"));
    assert_eq!(specs[3].voice, None);
    // Rate is not tied to an engine, so the configured one applies to all.
    assert!(specs.iter().all(|s| s.rate.get() == 1.2));
}

#[test]
fn a_voice_only_override_refines_the_engine_below_it() {
    let config = EngineChoice {
        voice: Some("config-voice".into()),
        ..with_fallback("alpha", &["alpha", "beta"])
    };
    let chat = EngineChoice {
        voice: Some("chat-voice".into()),
        rate: Some(SpeechRate::new(0.8).unwrap()),
        ..EngineChoice::default()
    };
    let specs = candidates(
        &config,
        Overrides {
            chat: Some(&chat),
            ..Overrides::default()
        },
    );
    assert_eq!(ids(&specs), ["alpha", "beta"]);
    assert_eq!(specs[0].voice.as_deref(), Some("chat-voice"));
    assert_eq!(specs[1].rate.get(), 0.8);
}

#[test]
fn nothing_configured_means_no_engine() {
    let registry = tts_registry();
    let mut tts = ActiveTts::new(candidates(&EngineChoice::default(), Overrides::default()));
    let mut notices = Vec::new();
    let error = tts
        .begin(&registry, &mut |n| notices.push(n), TurnId::new(), None)
        .unwrap_err();
    assert_eq!(error, RegistryError::NoEngine { kind: "tts" });
    assert!(notices.is_empty());
}

#[test]
fn switches_tts_mid_chat() {
    let registry = tts_registry();
    let config = choice("alpha");
    let mut tts = ActiveTts::new(candidates(&config, Overrides::default()));
    let mut notices = Vec::new();
    let mut out = [0.0; SAMPLES];

    assert_eq!(speak(&mut tts, &registry, &mut notices), "alpha");
    let engine = tts.engine_mut().unwrap();
    engine.push_text("First sentence.").unwrap();
    assert!(matches!(
        engine.poll(&mut out).unwrap(),
        TtsPoll::Audio { .. }
    ));

    // `aulo voice use beta` arrives while the reply is being spoken.
    let chat = choice("beta");
    tts.switch(candidates(
        &config,
        Overrides {
            chat: Some(&chat),
            ..Overrides::default()
        },
    ));
    assert_eq!(tts.active().unwrap().as_str(), "alpha");
    let engine = tts.engine_mut().unwrap();
    engine.push_text("Second sentence.").unwrap();
    engine.finish().unwrap();
    while engine.poll(&mut out).unwrap() != TtsPoll::Done {}

    assert_eq!(speak(&mut tts, &registry, &mut notices), "beta");
    assert!(notices.is_empty());
}

#[test]
fn a_failing_engine_falls_back_with_a_notice() {
    let registry = tts_registry();
    let config = with_fallback("broken", &["missing", "unknown", "beta"]);
    let mut tts = ActiveTts::new(candidates(&config, Overrides::default()));
    let mut notices = Vec::new();

    assert_eq!(speak(&mut tts, &registry, &mut notices), "beta");
    assert_eq!(notice_sources(&notices), ["broken", "missing", "unknown"]);
    let AuloEvent::Notice { message, .. } = &notices[0] else {
        unreachable!()
    };
    assert_eq!(
        message,
        "tts engine `broken` failed, trying the next one: engine unavailable: plugin process exited"
    );
    let AuloEvent::Notice { message, .. } = &notices[2] else {
        unreachable!()
    };
    assert!(message.ends_with("engine is not registered"), "{message}");

    // The working engine stays in use; the broken ones are not retried.
    notices.clear();
    assert_eq!(speak(&mut tts, &registry, &mut notices), "beta");
    assert!(notices.is_empty());
}

#[test]
fn a_voice_the_engine_lacks_falls_back_to_the_next_engine() {
    let registry = tts_registry();
    let config = EngineChoice {
        voice: Some("boris".into()),
        ..with_fallback("alpha", &["beta"])
    };
    let mut tts = ActiveTts::new(candidates(&config, Overrides::default()));
    let mut notices = Vec::new();
    assert_eq!(speak(&mut tts, &registry, &mut notices), "beta");
    assert_eq!(notice_sources(&notices), ["alpha"]);

    let known = EngineChoice {
        voice: Some(FAKE_VOICE.into()),
        ..choice("alpha")
    };
    tts.switch(candidates(&known, Overrides::default()));
    assert_eq!(speak(&mut tts, &registry, &mut notices), "alpha");
}

#[test]
fn an_engine_failing_mid_reply_is_replaced_from_the_next_reply() {
    let registry = tts_registry();
    let mut tts = ActiveTts::new(candidates(
        &with_fallback("alpha", &["beta"]),
        Overrides::default(),
    ));
    let mut notices = Vec::new();
    assert_eq!(speak(&mut tts, &registry, &mut notices), "alpha");

    let mut notify = |n| notices.push(n);
    // A caller bug or a full queue keeps the engine.
    tts.report(&SpeechError::OutOfOrder("poll before begin"), &mut notify);
    tts.report(&SpeechError::Overflow { dropped: 10 }, &mut notify);
    assert_eq!(tts.active().unwrap().as_str(), "alpha");
    // An engine fault retires it, without starting another mid-reply.
    tts.report(&SpeechError::failed("socket closed"), &mut notify);
    assert!(tts.active().is_none());

    assert_eq!(speak(&mut tts, &registry, &mut notices), "beta");
    assert_eq!(notice_sources(&notices), ["alpha"]);
}

#[test]
fn caller_bugs_do_not_trigger_fallback() {
    let mut registry = SttRegistry::new();
    registry
        .register(id("strict"), |_| {
            Ok(Box::new(FakeStt::failing(
                id("strict"),
                SpeechError::OutOfOrder("begin while closed"),
            )) as Box<dyn SttEngine>)
        })
        .unwrap();
    registry
        .register(id("spare"), |_| {
            Ok(Box::new(FakeStt::new(id("spare"))) as Box<dyn SttEngine>)
        })
        .unwrap();
    let mut stt = ActiveStt::new(candidates(
        &with_fallback("strict", &["spare"]),
        Overrides::default(),
    ));
    let mut notices = Vec::new();
    let error = stt
        .begin(&registry, &mut |n| notices.push(n), TurnId::new(), None)
        .unwrap_err();
    assert_eq!(
        error,
        RegistryError::Speech(SpeechError::OutOfOrder("begin while closed"))
    );
    assert_eq!(stt.active().unwrap().as_str(), "strict");
    assert!(notices.is_empty());
}

#[test]
fn when_every_engine_fails_the_next_begin_starts_over() {
    let registry = tts_registry();
    let mut tts = ActiveTts::new(candidates(
        &with_fallback("broken", &["missing"]),
        Overrides::default(),
    ));
    for _ in 0..2 {
        let mut notices = Vec::new();
        let error = tts
            .begin(&registry, &mut |n| notices.push(n), TurnId::new(), None)
            .unwrap_err();
        assert_eq!(error, RegistryError::NoEngine { kind: "tts" });
        assert_eq!(notice_sources(&notices), ["broken", "missing"]);
    }
}

#[test]
fn a_plugin_cannot_take_over_a_registered_id() {
    let mut registry = tts_registry();
    let error = registry
        .register(id("alpha"), |_| {
            Ok(Box::new(FakeTts::new(id("impostor"))) as Box<dyn TtsEngine>)
        })
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "tts engine `alpha` is already registered"
    );
    let all: Vec<&str> = registry.ids().map(EngineId::as_str).collect();
    assert_eq!(all, ["alpha", "beta", "broken", "missing"]);
}

#[test]
fn the_factory_receives_the_resolved_spec() {
    let mut registry = TtsRegistry::new();
    registry
        .register(id("echo"), |spec| {
            assert_eq!(spec.model.as_deref(), Some("kokoro-v1"));
            assert_eq!(spec.rate.get(), 1.5);
            Ok(Box::new(FakeTts::new(spec.engine.clone())) as Box<dyn TtsEngine>)
        })
        .unwrap();
    let config = EngineChoice {
        model: Some("kokoro-v1".into()),
        rate: Some(SpeechRate::new(1.5).unwrap()),
        ..choice("echo")
    };
    let mut tts = ActiveTts::new(candidates(&config, Overrides::default()));
    assert_eq!(speak(&mut tts, &registry, &mut Vec::new()), "echo");
}
