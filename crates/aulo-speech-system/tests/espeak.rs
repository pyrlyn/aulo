//! Runs the real `espeak-ng` when it is on `PATH` (the Linux CI runner
//! installs it) and does nothing otherwise, so the suite stays green on
//! machines without it.

// Helpers outside `#[test]` functions unwrap too, which clippy only allows inside them.
#![allow(clippy::unwrap_used)]

use std::env;
use std::thread;
use std::time::{Duration, Instant};

use aulo_speech::{EngineId, EngineSpec, SpeechRate, TtsEngine, TtsPoll, TtsRequest, TurnId};
use aulo_speech_system::espeak::{ENGINE_ID, factory};

const DEADLINE: Duration = Duration::from_secs(30);

fn espeak_on_path() -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|dir| dir.join("espeak-ng").is_file())
}

fn engine() -> Box<dyn TtsEngine> {
    let spec = EngineSpec {
        engine: EngineId::new(ENGINE_ID).unwrap(),
        model: None,
        voice: None,
        rate: SpeechRate::NORMAL,
    };
    factory(&spec).unwrap()
}

fn request<'a>(voice: Option<&'a str>, language: Option<&'a str>) -> TtsRequest<'a> {
    TtsRequest {
        turn_id: TurnId::new(),
        voice,
        language,
        rate: SpeechRate::NORMAL,
    }
}

#[test]
fn the_real_espeak_ng_speaks_english() {
    if !espeak_on_path() {
        eprintln!("skipped: espeak-ng is not on PATH");
        return;
    }
    let mut engine = engine();
    assert!(!engine.voices().is_empty());
    let english = engine
        .voices()
        .iter()
        .find(|v| v.language.as_deref().is_some_and(|l| l.starts_with("en")))
        .map(|v| v.id.clone())
        .unwrap();
    // Naming a listed voice checks that the id from `--voices` works for `-v`.
    let format = engine.begin(&request(Some(&english), None)).unwrap();
    assert_eq!(format.channels(), 1);
    assert!(format.sample_rate_hz() >= 8_000);
    engine.push_text("Hello, this is a test.").unwrap();
    engine.push_text("And a second sentence.").unwrap();
    engine.finish().unwrap();

    let started = Instant::now();
    let mut out = [0.0; 1024];
    let (mut total, mut peak) = (0, 0.0_f32);
    loop {
        match engine.poll(&mut out).unwrap() {
            TtsPoll::Audio { samples } => {
                total += samples;
                peak = out[..samples].iter().fold(peak, |p, s| p.max(s.abs()));
            }
            TtsPoll::Pending => thread::sleep(Duration::from_millis(5)),
            TtsPoll::Done => break,
        }
        assert!(started.elapsed() < DEADLINE, "the reply never finished");
    }
    // Two sentences are well over a second of speech, and not silence.
    assert!(total > usize::try_from(format.sample_rate_hz()).unwrap());
    assert!(peak > 0.05, "the audio is silent");
}

#[test]
fn the_real_espeak_ng_stops_on_cancel() {
    if !espeak_on_path() {
        eprintln!("skipped: espeak-ng is not on PATH");
        return;
    }
    let mut engine = engine();
    engine.begin(&request(None, Some("en"))).unwrap();
    engine
        .push_text(&"A long sentence goes on and on. ".repeat(50))
        .unwrap();
    let mut out = [0.0; 1024];
    let started = Instant::now();
    while !matches!(engine.poll(&mut out).unwrap(), TtsPoll::Audio { .. }) {
        thread::sleep(Duration::from_millis(5));
        assert!(started.elapsed() < DEADLINE, "no audio arrived");
    }
    engine.cancel();
    assert_eq!(engine.poll(&mut out).unwrap(), TtsPoll::Done);
}
