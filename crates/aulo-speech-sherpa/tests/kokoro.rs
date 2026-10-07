//! Time to first audio of Kokoro through the `TtsEngine` contract, against
//! the spike T0.5 budget: under 0.3 s when a reply starts with a short
//! sentence, at 4 threads (R3).
//!
//! Needs the 394 MiB model, so it runs only when `AULO_KOKORO_DIR` points at
//! an installed `kokoro-multi-lang-v1_0` directory (for example
//! `~/.aulo/models/kokoro-multi-lang-v1_0` after `aulo models pull`), and is
//! skipped, not failed, otherwise. Set `AULO_KOKORO_WAV` to a file path to
//! keep the English reply for listening.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use aulo_models::Catalog;
use aulo_speech::{EngineId, EngineSpec, SpeechRate, TtsEngine, TtsPoll, TtsRequest, TurnId};
use aulo_speech_sherpa::{
    CHUNK_SAMPLES, KOKORO_ENGINE_ID, KOKORO_MODEL_ID, KOKORO_SAMPLE_RATE_HZ, KokoroConfig,
    KokoroFactory,
};

const MODEL_DIR_ENV: &str = "AULO_KOKORO_DIR";
const WAV_ENV: &str = "AULO_KOKORO_WAV";
const BUDGET: Duration = Duration::from_millis(300);

/// Speaks `sentences` as one reply and returns the time to the first audio
/// and all samples.
fn speak(tts: &mut dyn TtsEngine, language: &str, sentences: &[&str]) -> (Duration, Vec<f32>) {
    let started = Instant::now();
    tts.begin(&TtsRequest {
        turn_id: TurnId::new(),
        voice: None,
        language: Some(language),
        rate: SpeechRate::NORMAL,
    })
    .unwrap();
    for sentence in sentences {
        tts.push_text(sentence).unwrap();
    }
    tts.finish().unwrap();
    let mut out = vec![0.0; CHUNK_SAMPLES];
    let (mut first, mut audio) = (None, Vec::new());
    let deadline = started + Duration::from_secs(30);
    loop {
        match tts.poll(&mut out).unwrap() {
            TtsPoll::Audio { samples } => {
                first.get_or_insert_with(|| started.elapsed());
                audio.extend_from_slice(&out[..samples]);
            }
            TtsPoll::Pending if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(1));
            }
            TtsPoll::Pending => panic!("no end of reply in 30 s"),
            TtsPoll::Done => break,
        }
    }
    (first.unwrap(), audio)
}

#[test]
fn kokoro_first_audio_is_within_the_budget() {
    let Some(dir) = std::env::var_os(MODEL_DIR_ENV) else {
        eprintln!("skipped: set {MODEL_DIR_ENV} to an installed {KOKORO_MODEL_ID} directory");
        return;
    };
    let catalog = Catalog::embedded().unwrap();
    let model = catalog.get(KOKORO_MODEL_ID).unwrap();
    let factory = KokoroFactory::new(model, Path::new(&dir), KokoroConfig::default()).unwrap();
    let spec = EngineSpec {
        engine: EngineId::new(KOKORO_ENGINE_ID).unwrap(),
        model: None,
        voice: None,
        rate: SpeechRate::NORMAL,
    };
    let loading = Instant::now();
    let mut tts = factory.build(&spec).unwrap();
    eprintln!("load {:.3} s", loading.elapsed().as_secs_f64());

    let reply = ["Sure.", "The quick brown fox jumps over the lazy dog."];
    let mut firsts = Vec::new();
    for run in 0..3 {
        let (first, audio) = speak(tts.as_mut(), "en-US", &reply);
        let seconds = audio.len() as f64 / f64::from(KOKORO_SAMPLE_RATE_HZ);
        eprintln!(
            "run {run}: first audio {:.3} s, {seconds:.2} s of audio",
            first.as_secs_f64()
        );
        assert!(seconds > 1.0, "only {seconds} s of audio");
        if run == 0
            && let Some(path) = std::env::var_os(WAV_ENV)
        {
            write_wav(Path::new(&path), &audio);
        }
        firsts.push(first);
    }
    // The warm-up in `load` is what keeps the very first reply in budget too;
    // without it onnxruntime's first run took 1 s.
    for first in firsts {
        assert!(first < BUDGET, "first audio after {first:?}");
    }

    let (first, audio) = speak(tts.as_mut(), "es", &["Claro.", "Hace buen tiempo hoy."]);
    eprintln!("es: first audio {:.3} s", first.as_secs_f64());
    assert!(!audio.is_empty());
    let request = TtsRequest {
        turn_id: TurnId::new(),
        voice: None,
        language: Some("ru"),
        rate: SpeechRate::NORMAL,
    };
    assert!(tts.begin(&request).unwrap_err().should_fall_back());
}

fn write_wav(path: &Path, samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: KOKORO_SAMPLE_RATE_HZ,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(path, spec).unwrap();
    for &sample in samples {
        wav.write_sample(sample).unwrap();
    }
    wav.finalize().unwrap();
}
