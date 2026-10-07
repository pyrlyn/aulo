//! Word error rate of Parakeet through the `SttEngine` contract on the
//! fixture WAVs (macOS `say`, Samantha and Milena, 16 kHz mono 16-bit).
//!
//! Needs the 640 MiB model, so it runs only when `AULO_PARAKEET_DIR` points
//! at an installed `parakeet-tdt-0.6b-v3-int8` directory (for example
//! `~/.aulo/models/parakeet-tdt-0.6b-v3-int8` after `aulo models pull`), and
//! is skipped, not failed, otherwise.

// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use aulo_models::Catalog;
use aulo_speech::{
    AudioFormat, AudioFrame, EngineId, EngineSpec, SpeechRate, SttPoll, Transcript, TranscriptKind,
    TurnId,
};
use aulo_speech_sherpa::{PARAKEET_ENGINE_ID, PARAKEET_MODEL_ID, ParakeetConfig, ParakeetFactory};

const MODEL_DIR_ENV: &str = "AULO_PARAKEET_DIR";

/// The spike recorded no WER, only that clean synthetic English came back
/// exactly and 17 s of synthetic Russian had 3 small errors. 10 % over the
/// whole set allows about one wrong word in Russian and none to spare.
const MAX_WER: f64 = 0.10;

const FRAME: usize = 320;

/// File, language hint, what was said.
const FIXTURES: &[(&str, &str, &str)] = &[
    (
        "en_browser.wav",
        "en",
        "Open the browser and play some music.",
    ),
    (
        "en_weather.wav",
        "en",
        "What is the weather like in London tomorrow?",
    ),
    (
        "en_email.wav",
        "en",
        "Please read my latest email out loud.",
    ),
    ("ru_browser.wav", "ru", "Открой браузер и включи музыку."),
    ("ru_weather.wav", "ru", "Какая завтра погода в Москве?"),
];

#[test]
fn parakeet_word_error_rate_is_under_the_threshold() {
    let Some(dir) = std::env::var_os(MODEL_DIR_ENV) else {
        eprintln!("skipped: set {MODEL_DIR_ENV} to an installed {PARAKEET_MODEL_ID} directory");
        return;
    };
    let catalog = Catalog::embedded().unwrap();
    let model = catalog.get(PARAKEET_MODEL_ID).unwrap();
    let factory = ParakeetFactory::new(model, Path::new(&dir), ParakeetConfig::default()).unwrap();
    let mut stt = factory
        .build(&EngineSpec {
            engine: EngineId::new(PARAKEET_ENGINE_ID).unwrap(),
            model: None,
            voice: None,
            rate: SpeechRate::NORMAL,
        })
        .unwrap();

    let (mut errors, mut words) = (0, 0);
    for (file, language, reference) in FIXTURES {
        let samples = read_wav(&fixture(file));
        let turn = TurnId::new();
        let mut out = Transcript::with_capacity(turn, 256);
        stt.begin(turn, Some(language)).unwrap();
        for (i, chunk) in samples.chunks(FRAME).enumerate() {
            let at = (i * FRAME) as u64;
            stt.push(AudioFrame::new(chunk, AudioFormat::PIPELINE, at).unwrap())
                .unwrap();
        }
        stt.finish().unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        while stt.poll(&mut out).unwrap() != SttPoll::Updated {
            assert!(Instant::now() < deadline, "{file}: no transcript");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!((out.kind, out.turn_id), (TranscriptKind::Final, turn));
        let (wrong, total) = word_errors(&out.text, reference);
        eprintln!("{file}: {wrong}/{total} words wrong: {:?}", out.text);
        errors += wrong;
        words += total;
    }
    let wer = errors as f64 / words as f64;
    eprintln!("WER {wer:.3} over {words} words");
    assert!(wer <= MAX_WER, "WER {wer:.3} is above {MAX_WER}");
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn read_wav(path: &Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).unwrap();
    let spec = reader.spec();
    assert_eq!(
        (spec.sample_rate, spec.channels, spec.bits_per_sample),
        (16_000, 1, 16)
    );
    reader
        .samples::<i16>()
        .map(|s| f32::from(s.unwrap()) / f32::from(i16::MAX))
        .collect()
}

/// Case, punctuation and `ё` do not count as errors.
fn normalize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace('ё', "е")
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Word-level Levenshtein distance (substitutions, insertions, deletions)
/// and the reference length, the two terms of WER.
fn word_errors(hypothesis: &str, reference: &str) -> (usize, usize) {
    let (hyp, reference) = (normalize(hypothesis), normalize(reference));
    let mut row: Vec<usize> = (0..=hyp.len()).collect();
    for (i, want) in reference.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, got) in hyp.iter().enumerate() {
            let substitute = diagonal + usize::from(want != got);
            diagonal = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(diagonal + 1);
        }
    }
    (row[hyp.len()], reference.len())
}

#[test]
fn word_errors_count_edits_not_formatting() {
    assert_eq!(word_errors("Open the door.", "open the DOOR"), (0, 3));
    assert_eq!(word_errors("open door", "open the door"), (1, 3));
    assert_eq!(word_errors("open a big door", "open the door"), (2, 3));
    assert_eq!(word_errors("ещё", "еще"), (0, 1));
    assert_eq!(word_errors("", "open the door"), (3, 3));
}
