use aulo_speech::EngineId;
use aulo_speech::SpeechError;
use aulo_speech::testkit::FakeTts;

use crate::fake::ScriptedStt;
use crate::*;

fn workload() -> Workload {
    Workload::builtin().unwrap()
}

fn stt_candidate(id: &'static str, engine: fn() -> Result<ScriptedStt, SpeechError>) -> Candidate {
    Candidate::stt(id, move || {
        let e = engine().map_err(|e| e.to_string())?;
        Ok(Box::new(e))
    })
}

fn tts_candidate(id: &'static str) -> Candidate {
    Candidate::tts(id, move || {
        let id = EngineId::new(id).map_err(|e| e.to_string())?;
        Ok(Box::new(FakeTts::new(id)))
    })
}

fn measured(row: &Row) -> Metrics {
    match &row.outcome {
        Outcome::Measured(m) => *m,
        other => panic!("{}: {other:?}", row.engine),
    }
}

#[test]
fn a_perfect_streaming_engine_scores_zero_wer_and_a_first_partial() {
    // The reference is the same sentence for every clip, so only one language
    // group can be perfect: restrict the engine to English and compare to it.
    let mut workload = workload();
    workload.stt.retain(|i| i.name == "en_browser");
    let text = workload.stt[0].reference.clone();
    let candidates = vec![Candidate::stt("scripted", move || {
        let engine = ScriptedStt::new("scripted", &text)
            .map_err(|e| e.to_string())?
            .with_partial("open");
        Ok(Box::new(engine))
    })];
    let rows = run(candidates, None, &workload);
    let m = measured(&rows[0]);
    assert_eq!((m.wer, m.clips, m.skipped), (Some(0.0), 1, 0));
    assert!(m.first.is_some());
    assert!(m.rtf.is_finite() && m.rtf >= 0.0);
}

#[test]
fn wrong_words_and_unsupported_languages_are_counted() {
    let rows = run(
        vec![stt_candidate("english", || {
            Ok(ScriptedStt::new("english", "open the browser")?.with_languages(&["en"]))
        })],
        None,
        &workload(),
    );
    let m = measured(&rows[0]);
    // Three English clips are heard, two Russian ones skipped.
    assert_eq!((m.clips, m.skipped), (3, 2));
    assert!(m.wer.unwrap() > 0.0);
    assert_eq!(m.first, None);
}

#[test]
fn speech_reports_first_audio_and_a_real_time_factor() {
    let rows = run(vec![tts_candidate("fake")], None, &workload());
    let m = measured(&rows[0]);
    assert_eq!((rows[0].kind, m.wer, m.clips), (Kind::Tts, None, 3));
    assert!(m.first.is_some());
    assert!(m.rtf.is_finite());
}

#[test]
fn an_engine_that_cannot_start_is_a_row_not_an_error() {
    let candidates = vec![
        Candidate::stt("broken", || Err("model is not installed".to_owned())),
        tts_candidate("fake"),
    ];
    let rows = run(candidates, None, &workload());
    assert_eq!(
        rows[0].outcome,
        Outcome::Unavailable("model is not installed".to_owned())
    );
    assert!(matches!(rows[1].outcome, Outcome::Measured(_)));
}

#[test]
fn an_engine_failing_mid_run_is_a_failed_row() {
    let candidates = vec![Candidate::tts("fails", || {
        let id = EngineId::new("fails").map_err(|e| e.to_string())?;
        Ok(Box::new(FakeTts::failing(
            id,
            SpeechError::failed("socket closed"),
        )))
    })];
    let rows = run(candidates, None, &workload());
    assert_eq!(
        rows[0].outcome,
        Outcome::Failed("engine failed: socket closed".to_owned())
    );
}

#[test]
fn the_engine_filter_runs_only_the_named_engine() {
    let candidates = vec![tts_candidate("fake"), tts_candidate("other")];
    let rows = run(candidates, Some("other"), &workload());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].engine, "other");
}

#[test]
fn an_engine_without_a_supported_language_has_nothing_to_measure() {
    let rows = run(
        vec![stt_candidate("klingon", || {
            Ok(ScriptedStt::new("klingon", "x")?.with_languages(&["tlh"]))
        })],
        None,
        &workload(),
    );
    assert!(matches!(&rows[0].outcome, Outcome::Failed(why) if why.contains("no input")));
}

#[test]
fn engines_without_a_key_stay_unbuilt_and_off_the_network() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let home = std::env::temp_dir().join("aulo-bench-no-such-home");
    let mut candidates = discover::builtin(&home, runtime.handle(), &Keys::default());
    // The system voices are real and differ per machine; this test is about
    // models and keys.
    candidates.retain(|c| c.id != "system");
    let rows = run(
        candidates,
        None,
        &Workload {
            stt: vec![],
            tts: vec![],
        },
    );
    let reason = |kind, engine: &str| {
        rows.iter()
            .find(|r| (r.kind, r.engine.as_str()) == (kind, engine))
            .map(|r| r.outcome.clone())
    };
    for (kind, engine, variable) in [
        (Kind::Stt, "openai", "OPENAI_API_KEY"),
        (Kind::Stt, "deepgram", "DEEPGRAM_API_KEY"),
        (Kind::Tts, "openai", "OPENAI_API_KEY"),
        (Kind::Tts, "elevenlabs", "ELEVENLABS_API_KEY"),
    ] {
        assert_eq!(
            reason(kind, engine),
            Some(Outcome::Unavailable(format!("no key: set {variable}")))
        );
    }
    for engine in ["sherpa-parakeet", "whisper-cpp"] {
        let Some(Outcome::Unavailable(why)) = reason(Kind::Stt, engine) else {
            panic!("{engine} should be unavailable without its model");
        };
        assert!(why.contains("not installed"), "{why}");
    }
}
