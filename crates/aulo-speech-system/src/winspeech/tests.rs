//! The engine, worker and decoder against a fake backend that returns WAV
//! bytes, so every behavior but the WinRT calls themselves runs on any host.

use std::io::{Cursor, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use aulo_speech::{SpeechError, SpeechRate, TtsEngine, TtsPoll, TtsRequest, TurnId, Voice};

use super::engine::WindowsTts;
use super::worker::{Backend, Catalog, OUTPUT_HZ};
use crate::wav::wav_bytes;

type Spoken = Arc<Mutex<Vec<(String, String, f32)>>>;

/// What `synthesize` does, shared with the test that drives it.
#[derive(Clone, Default)]
struct Script {
    spoken: Spoken,
    /// Texts that fail.
    fail_on: Option<&'static str>,
    /// A text that waits for `gate` before returning audio.
    hold: Option<(&'static str, Arc<AtomicBool>)>,
    /// Whether a held text notices a cancel, as the real backend does.
    honor_cancel: bool,
    rate_hz: Option<u32>,
    frames: Option<usize>,
    dropped: Arc<AtomicBool>,
}

struct Fake {
    script: Script,
    voices: Vec<Voice>,
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.script.dropped.store(true, Ordering::Release);
    }
}

fn voice(id: &str, language: &str) -> Voice {
    Voice {
        id: id.into(),
        name: id.into(),
        language: Some(language.into()),
    }
}

impl Fake {
    fn new(script: Script) -> Self {
        let voices = vec![
            voice("us", "en-US"),
            voice("de", "de-DE"),
            voice("gb", "en-GB"),
        ];
        Self { script, voices }
    }
}

impl Backend for Fake {
    fn catalog(&self) -> Result<Catalog, SpeechError> {
        Ok(Catalog {
            voices: self.voices.clone(),
            default: Some(1),
        })
    }

    fn synthesize(
        &mut self,
        text: &str,
        voice: &str,
        rate: SpeechRate,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<Box<dyn Read>>, SpeechError> {
        let script = &self.script;
        let entry = (text.to_owned(), voice.to_owned(), rate.get());
        script.spoken.lock().unwrap().push(entry);
        if script.fail_on == Some(text) {
            return Err(SpeechError::failed("voice crashed"));
        }
        if let Some((held, gate)) = &script.hold
            && *held == text
        {
            while !gate.load(Ordering::Acquire) {
                if script.honor_cancel && cancelled() {
                    return Ok(None);
                }
                thread::sleep(Duration::from_millis(1));
            }
        }
        let level = if text == "stale" { 16_384 } else { 8_192 };
        let frames = vec![level; script.frames.unwrap_or(1_000)];
        let bytes = wav_bytes(script.rate_hz.unwrap_or(OUTPUT_HZ), 1, &frames);
        Ok(Some(Box::new(Cursor::new(bytes))))
    }
}

fn engine(script: &Script) -> WindowsTts {
    let script = script.clone();
    WindowsTts::start(move || Ok(Fake::new(script)), None).unwrap()
}

fn request<'a>(voice: Option<&'a str>, language: Option<&'a str>, rate: f32) -> TtsRequest<'a> {
    TtsRequest {
        turn_id: TurnId::new(),
        voice,
        language,
        rate: SpeechRate::new(rate).unwrap(),
    }
}

fn until(mut condition: impl FnMut() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(started.elapsed() < Duration::from_secs(10), "timed out");
        thread::sleep(Duration::from_millis(1));
    }
}

/// Polls until the reply ends; the error is the one `poll` reported, after
/// the audio that came before it.
fn drain(engine: &mut WindowsTts) -> (Vec<f32>, Option<SpeechError>) {
    let mut audio = Vec::new();
    let mut buffer = [0.0; 512];
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(20), "timed out");
        match engine.poll(&mut buffer) {
            Ok(TtsPoll::Audio { samples }) => audio.extend_from_slice(&buffer[..samples]),
            Ok(TtsPoll::Pending) => thread::sleep(Duration::from_millis(1)),
            Ok(TtsPoll::Done) => return (audio, None),
            Err(error) => return (audio, Some(error)),
        }
    }
}

fn speak(engine: &mut WindowsTts, texts: &[&str]) -> (Vec<f32>, Option<SpeechError>) {
    engine.begin(&request(Some("us"), None, 1.0)).unwrap();
    for text in texts {
        engine.push_text(text).unwrap();
    }
    engine.finish().unwrap();
    drain(engine)
}

#[test]
fn a_backend_that_fails_or_has_no_voices_is_unavailable() {
    let failing = WindowsTts::start::<Fake>(|| Err(SpeechError::unavailable("no WinRT")), None);
    assert!(matches!(failing.unwrap_err(), SpeechError::Unavailable(_)));
    let script = Script::default();
    let empty = WindowsTts::start(
        move || {
            let mut fake = Fake::new(script);
            fake.voices.clear();
            Ok(fake)
        },
        None,
    );
    assert!(matches!(empty.unwrap_err(), SpeechError::Unavailable(_)));
}

#[test]
fn voices_come_from_the_backend_and_language_picks_one() {
    let script = Script::default();
    let mut tts = engine(&script);
    assert_eq!(tts.voices().len(), 3);
    let format = tts.begin(&request(None, Some("en-GB"), 1.0)).unwrap();
    assert_eq!((format.sample_rate_hz(), format.channels()), (OUTPUT_HZ, 1));
    tts.push_text("hello").unwrap();
    tts.finish().unwrap();
    drain(&mut tts);
    // No language: the system's default voice.
    tts.begin(&request(None, None, 1.0)).unwrap();
    tts.push_text("hallo").unwrap();
    tts.finish().unwrap();
    drain(&mut tts);
    // A named voice beats the language.
    speak(&mut tts, &["named"]);
    let spoken = script.spoken.lock().unwrap();
    let voices: Vec<_> = spoken.iter().map(|s| s.1.as_str()).collect();
    assert_eq!(voices, ["gb", "de", "us"]);
}

#[test]
fn unknown_voices_and_languages_are_unsupported_so_the_registry_falls_back() {
    let mut tts = engine(&Script::default());
    for bad in [
        request(Some("no.such.voice"), None, 1.0),
        request(None, Some("tlh"), 1.0),
    ] {
        assert!(tts.begin(&bad).unwrap_err().should_fall_back());
    }
    let script = Script::default();
    let mut tts = WindowsTts::start(move || Ok(Fake::new(script)), Some("gone".into())).unwrap();
    let error = tts.begin(&request(None, Some("en"), 1.0)).unwrap_err();
    assert!(matches!(error, SpeechError::Unsupported(_)));
}

#[test]
fn sentences_arrive_in_order_at_the_declared_rate() {
    let script = Script {
        rate_hz: Some(16_000),
        frames: Some(16_000),
        ..Script::default()
    };
    let mut tts = engine(&script);
    let (audio, error) = speak(&mut tts, &["one", "two"]);
    assert!(error.is_none());
    assert!(
        audio.len().abs_diff(2 * 22_050) <= 4,
        "{} samples",
        audio.len()
    );
    assert!(audio.iter().all(|&s| (s - 0.25).abs() < 1e-6));
    let texts: Vec<_> = script
        .spoken
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.0.clone())
        .collect();
    assert_eq!(texts, ["one", "two"]);
}

#[test]
fn the_requested_rate_reaches_the_backend() {
    let script = Script::default();
    let mut tts = engine(&script);
    tts.begin(&request(None, Some("de"), 1.5)).unwrap();
    tts.push_text("schnell").unwrap();
    tts.finish().unwrap();
    drain(&mut tts);
    assert_eq!(script.spoken.lock().unwrap()[0].2, 1.5);
}

#[test]
fn a_full_ring_pauses_the_worker_and_loses_nothing() {
    let frames = (1 << 19) + 100_000;
    let script = Script {
        frames: Some(frames),
        ..Script::default()
    };
    let mut tts = engine(&script);
    tts.begin(&request(Some("us"), None, 1.0)).unwrap();
    tts.push_text("long").unwrap();
    tts.finish().unwrap();
    // The ring cannot hold the sentence, so nothing can be done until polled.
    thread::sleep(Duration::from_millis(100));
    let (audio, error) = drain(&mut tts);
    assert!(error.is_none());
    assert_eq!(audio.len(), frames);
}

#[test]
fn cancel_fences_audio_that_finishes_after_it() {
    let gate = Arc::new(AtomicBool::new(false));
    let script = Script {
        hold: Some(("stale", Arc::clone(&gate))),
        ..Script::default()
    };
    let mut tts = engine(&script);
    tts.begin(&request(Some("us"), None, 1.0)).unwrap();
    tts.push_text("stale").unwrap();
    until(|| !script.spoken.lock().unwrap().is_empty());
    tts.cancel();
    assert_eq!(tts.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
    tts.begin(&request(Some("us"), None, 1.0)).unwrap();
    tts.push_text("fresh").unwrap();
    tts.finish().unwrap();
    // This backend ignores the cancel and delivers the old sentence's audio late.
    gate.store(true, Ordering::Release);
    let (audio, error) = drain(&mut tts);
    assert!(error.is_none());
    assert_eq!(audio.len(), 1_000);
    assert!(audio.iter().all(|&s| (s - 0.25).abs() < 1e-6));
}

#[test]
fn a_cancel_stops_a_sentence_that_is_still_synthesizing() {
    let gate = Arc::new(AtomicBool::new(false));
    let script = Script {
        hold: Some(("stale", gate)),
        honor_cancel: true,
        ..Script::default()
    };
    let mut tts = engine(&script);
    tts.begin(&request(Some("us"), None, 1.0)).unwrap();
    tts.push_text("stale").unwrap();
    until(|| !script.spoken.lock().unwrap().is_empty());
    tts.cancel();
    // The gate never opens: only the cancel can free the worker for this.
    let (audio, error) = speak(&mut tts, &["fresh"]);
    assert!(error.is_none());
    assert_eq!(audio.len(), 1_000);
}

#[test]
fn a_failed_sentence_is_reported_once_and_the_engine_recovers() {
    let script = Script {
        fail_on: Some("bad"),
        ..Script::default()
    };
    let mut tts = engine(&script);
    let (_, error) = speak(&mut tts, &["ok", "bad"]);
    let error = error.unwrap();
    assert!(matches!(error, SpeechError::Failed(_)));
    assert!(error.should_fall_back());
    assert_eq!(tts.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
    let (audio, error) = speak(&mut tts, &["fine"]);
    assert!(error.is_none());
    assert_eq!(audio.len(), 1_000);
}

#[test]
fn call_order_and_text_caps_are_enforced() {
    let mut tts = engine(&Script::default());
    assert!(matches!(
        tts.push_text("hi"),
        Err(SpeechError::OutOfOrder(_))
    ));
    assert!(matches!(tts.finish(), Err(SpeechError::OutOfOrder(_))));
    assert_eq!(tts.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
    tts.begin(&request(None, Some("en"), 1.0)).unwrap();
    let long = "a".repeat(super::engine::MAX_TEXT_BYTES + 1);
    assert!(matches!(
        tts.push_text(&long),
        Err(SpeechError::Invalid { .. })
    ));
    tts.push_text("   ").unwrap();
    tts.finish().unwrap();
    assert!(matches!(
        tts.push_text("late"),
        Err(SpeechError::OutOfOrder(_))
    ));
    // Nothing was queued, so the reply is over at once.
    assert_eq!(tts.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
}

#[test]
fn a_backed_up_queue_overflows_instead_of_growing() {
    let gate = Arc::new(AtomicBool::new(false));
    let script = Script {
        hold: Some(("wait", Arc::clone(&gate))),
        ..Script::default()
    };
    let mut tts = engine(&script);
    tts.begin(&request(Some("us"), None, 1.0)).unwrap();
    let results: Vec<_> = (0..70).map(|_| tts.push_text("wait")).collect();
    assert!(
        results[..super::engine::MAX_QUEUED_TEXTS as usize]
            .iter()
            .all(Result::is_ok)
    );
    assert!(
        results
            .iter()
            .any(|r| matches!(r, Err(SpeechError::Overflow { .. })))
    );
    gate.store(true, Ordering::Release);
}

#[test]
fn dropping_the_engine_stops_a_worker_waiting_on_a_full_ring() {
    let script = Script {
        frames: Some((1 << 19) + 100_000),
        ..Script::default()
    };
    let mut tts = engine(&script);
    tts.begin(&request(Some("us"), None, 1.0)).unwrap();
    tts.push_text("long").unwrap();
    thread::sleep(Duration::from_millis(100));
    drop(tts);
    until(|| script.dropped.load(Ordering::Acquire));
}
