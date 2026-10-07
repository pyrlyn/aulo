//! Speech benchmarks: word error rate, real-time factor and time to first
//! audio of every speech engine that can run on this machine, measured on the
//! fixture clips of the engine tests.
//!
//! - [`wer`] is the word error rate, [`clips`] the embedded workload.
//! - [`measure`] drives one engine through the push-and-poll contract and
//!   times it; [`discover`] lists the engines (local models, system voices,
//!   cloud engines with a key in the environment); [`report`] prints rows.
//! - [`run`] ties them together. The first clip of an engine includes its
//!   warm-up, which is what a user's first sentence pays too.
//!
//! No network is touched unless a cloud key is set: an engine without one is
//! reported unavailable without being built.

pub mod clips;
pub mod discover;
#[cfg(feature = "testkit")]
pub mod fake;
pub mod measure;
pub mod report;
mod runloop;
pub mod wer;

pub use aulo_speech_system::Resampler;
pub use discover::{Candidate, Engine, Keys};
pub use measure::{Metrics, SttInput, TtsInput, Workload, bench_stt, bench_tts};
pub use report::{Kind, Outcome, Row, render_json, render_table};
pub use runloop::with_main_run_loop;
pub use wer::WordErrors;

/// Benchmarks the candidates whose id equals `only` (all when `None`), one
/// after the other, and returns a row each. An engine that cannot be built, or
/// breaks on a clip, becomes a row with the reason, never an error.
pub fn run(candidates: Vec<Candidate>, only: Option<&str>, workload: &Workload) -> Vec<Row> {
    candidates
        .into_iter()
        .filter(|c| only.is_none_or(|id| id == c.id))
        .map(|candidate| {
            let outcome = match (candidate.build)() {
                Err(why) => Outcome::Unavailable(why),
                Ok(engine) => {
                    let measured = match engine {
                        Engine::Stt(mut e) => bench_stt(e.as_mut(), &workload.stt),
                        Engine::Tts(mut e) => bench_tts(e.as_mut(), &workload.tts),
                    };
                    measured.map_or_else(Outcome::Failed, Outcome::Measured)
                }
            };
            Row {
                kind: candidate.kind,
                engine: candidate.id,
                outcome,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
