//! Benchmarks of the deterministic parts of the speech harness. Real engines
//! are measured by `aulo bench speech`, not here, because they need models,
//! voices or keys.

use aulo_speech::EngineId;
use aulo_speech::testkit::FakeTts;
use aulo_speech_bench::fake::ScriptedStt;
use aulo_speech_bench::{Resampler, TtsInput, WordErrors, Workload, bench_stt, bench_tts, clips};
use divan::black_box;

fn main() {
    divan::main();
}

const REFERENCE: &str =
    "What is the weather like in London tomorrow? Please read my latest email out loud.";
const HYPOTHESIS: &str =
    "what is the weather in london tomorrow please read the latest email out loud";

#[divan::bench]
fn word_error_rate() -> WordErrors {
    WordErrors::between(black_box(HYPOTHESIS), black_box(REFERENCE))
}

#[divan::bench]
fn decode_clip() -> Option<usize> {
    clips::CLIPS[1].samples().ok().map(|s| s.len())
}

/// One second of 24 kHz speech down to the 16 kHz pipeline rate.
#[divan::bench]
fn resample_24k_to_16k(bencher: divan::Bencher) {
    let input = vec![0.25_f32; 24_000];
    bencher.bench_local(|| {
        let mut resampler = Resampler::new(24_000.0, 16_000);
        let mut samples = 0;
        for &sample in black_box(&input) {
            resampler.push(sample, |_| samples += 1);
        }
        samples
    });
}

#[divan::bench]
fn measure_stt_against_a_fake_engine(bencher: divan::Bencher) {
    let Ok(workload) = Workload::builtin() else {
        return;
    };
    bencher.bench_local(|| {
        let Ok(mut engine) = ScriptedStt::new("scripted", "open the browser") else {
            return None;
        };
        bench_stt(&mut engine, black_box(&workload.stt)).ok()
    });
}

#[divan::bench]
fn measure_tts_against_a_fake_engine(bencher: divan::Bencher) {
    let Ok(id) = EngineId::new("fake") else {
        return;
    };
    let inputs = vec![TtsInput {
        language: "en".to_owned(),
        text: "Please read my latest email out loud.".to_owned(),
    }];
    bencher.bench_local(|| {
        let mut engine = FakeTts::new(id.clone());
        bench_tts(&mut engine, black_box(&inputs)).ok()
    });
}
