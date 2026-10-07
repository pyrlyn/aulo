//! Transcribes the English fixture clips (macOS `say`, Samantha, 16 kHz mono
//! 16-bit; the same clips as the Parakeet and benchmark tests) through the
//! `SttEngine` contract and checks the word error rate.
//!
//! Needs the `whisper` feature, which compiles whisper.cpp, and the 78 MB
//! model, so it runs only when `AULO_WHISPER_DIR` points at an installed
//! `whisper-tiny.en` directory (`aulo models pull whisper-tiny.en` puts it at
//! `<AULO_HOME>/models/whisper-tiny.en`). Without the variable it is skipped,
//! not failed, so `cargo test --workspace` needs no network and no model.
//!
//! `AULO_WHISPER_DIR=~/.aulo/models/whisper-tiny.en \
//!   cargo test -p aulo-speech-whisper --features whisper --test fixture -- --nocapture`

#![cfg(feature = "whisper")]
// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::Path;

use aulo_models::Catalog;
use aulo_speech::{EngineId, EngineSpec, SpeechError, SpeechRate, TurnId};
use aulo_speech_bench::WordErrors;
use aulo_speech_bench::clips::CLIPS;
use aulo_speech_bench::measure::transcribe;
use aulo_speech_whisper::{WHISPER_ENGINE_ID, WHISPER_MODEL_ID, WhisperConfig, WhisperFactory};

const MODEL_DIR_ENV: &str = "AULO_WHISPER_DIR";

/// Clean synthetic English: tiny.en should get nearly all of it. 15 % over
/// the three clips (28 words) allows about four wrong words.
const MAX_WER: f64 = 0.15;

#[test]
fn whisper_transcribes_the_english_fixtures() {
    let Some(dir) = std::env::var_os(MODEL_DIR_ENV) else {
        eprintln!("skipped: set {MODEL_DIR_ENV} to an installed {WHISPER_MODEL_ID} directory");
        return;
    };
    let catalog = Catalog::embedded().unwrap();
    let model = catalog.get(WHISPER_MODEL_ID).unwrap();
    let factory = WhisperFactory::new(model, Path::new(&dir), WhisperConfig::default()).unwrap();
    let mut stt = factory
        .build(&EngineSpec {
            engine: EngineId::new(WHISPER_ENGINE_ID).unwrap(),
            model: None,
            voice: None,
            rate: SpeechRate::NORMAL,
        })
        .unwrap();

    let mut total = WordErrors::default();
    for clip in CLIPS.iter().filter(|clip| clip.language == "en") {
        let run = transcribe(stt.as_mut(), &clip.samples().unwrap(), clip.language).unwrap();
        let one = WordErrors::between(&run.text, clip.text);
        eprintln!(
            "{}: {}/{} words wrong: {:?} ({:?} for {:?} of audio)",
            clip.name, one.errors, one.words, run.text, run.elapsed, run.audio
        );
        total += one;
    }
    assert!(total.words > 0, "no English clips");
    let wer = total.rate().unwrap();
    eprintln!("WER {wer:.3} over {} words", total.words);
    assert!(wer <= MAX_WER, "WER {wer:.3} is above {MAX_WER}");

    // Russian is refused before any audio is taken, so the registry can fall back.
    let error = stt.begin(TurnId::new(), Some("ru")).unwrap_err();
    assert!(matches!(error, SpeechError::Unsupported(_)));
}
