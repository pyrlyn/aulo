//! Synthesis through the real WinRT `SpeechSynthesizer`; runs on the Windows
//! CI runner. Nothing is played through the speakers.

#![cfg(windows)]
// Test helpers outside `#[test]` functions may unwrap too (the workspace rule).
#![allow(clippy::unwrap_used)]

use std::thread;
use std::time::{Duration, Instant};

use aulo_speech::{SpeechRate, TtsEngine, TtsPoll, TtsRequest, TurnId};
use aulo_speech_system::SystemTts;

const PHRASE: &str = "Hello from aulo. This is a short test.";

fn request(language: Option<&str>) -> TtsRequest<'_> {
    TtsRequest {
        turn_id: TurnId::new(),
        voice: None,
        language,
        rate: SpeechRate::NORMAL,
    }
}

fn drain(engine: &mut SystemTts) -> Vec<f32> {
    let mut audio = Vec::new();
    let mut buffer = [0.0; 1024];
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(60), "timed out");
        match engine.poll(&mut buffer).unwrap() {
            TtsPoll::Audio { samples } => audio.extend_from_slice(&buffer[..samples]),
            TtsPoll::Pending => thread::sleep(Duration::from_millis(5)),
            TtsPoll::Done => return audio,
        }
    }
}

#[test]
fn voices_are_listed() {
    let engine = SystemTts::new(None).unwrap();
    assert!(!engine.voices().is_empty(), "Windows ships system voices");
    assert!(engine.voices().iter().all(|v| !v.id.is_empty()));
}

#[test]
fn phrase_yields_speech() {
    let mut engine = SystemTts::new(None).unwrap();
    let format = engine.begin(&request(None)).unwrap();
    assert_eq!(format.channels(), 1);
    engine.push_text(PHRASE).unwrap();
    engine.finish().unwrap();
    let audio = drain(&mut engine);
    let seconds = audio.len() as f32 / format.sample_rate_hz() as f32;
    let peak = audio.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));
    assert!((0.5..30.0).contains(&seconds), "{seconds} s of audio");
    assert!(peak > 0.01 && peak <= 1.0, "peak {peak}");
}

#[test]
fn cancel_ends_the_reply() {
    let mut engine = SystemTts::new(None).unwrap();
    engine.begin(&request(None)).unwrap();
    engine.push_text(PHRASE).unwrap();
    engine.cancel();
    assert_eq!(engine.poll(&mut [0.0; 64]).unwrap(), TtsPoll::Done);
    // A cancelled sentence must not leak into the next reply.
    engine.begin(&request(None)).unwrap();
    engine.finish().unwrap();
    assert_eq!(drain(&mut engine).len(), 0);
}
