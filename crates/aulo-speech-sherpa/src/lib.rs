//! Local speech engines on sherpa-onnx, the only crate that links it:
//!
//! - [`ParakeetStt`], NVIDIA Parakeet TDT 0.6B v3 (25 languages including ru
//!   and uk), decoded offline per VAD segment. The engine pushes no partials.
//! - [`KokoroTts`], Kokoro 82M v1.0 with 54 voices in 9 languages (no
//!   Russian, so a Russian reply falls back to another engine). Each pushed
//!   sentence is synthesized on its own, in order, so the first one plays
//!   while the rest is generated.
//!
//! Facts below are from the T0.5 spike, `docs/spikes/sherpa-onnx.md`.
//!
//! # Models
//!
//! [`ParakeetFactory::new`] takes the `aulo-models` catalog entry
//! [`PARAKEET_MODEL_ID`] and its directory from `ModelManager::path`, and
//! accepts no other model; [`KokoroFactory::new`] does the same with
//! [`KOKORO_MODEL_ID`]. Moonshine is not wired: sherpa-onnx 1.13.8 runs it
//! only as an offline model, and the catalog has no hash-pinned Moonshine
//! bundle yet.
//!
//! The Kokoro entry lists the 377 files of the official
//! `kokoro-multi-lang-v1_0.tar.bz2` (tag `tts-models` of k2-fsa/sherpa-onnx).
//! The archive matched the SHA-256 digest GitHub publishes for that release
//! asset, and each file's SHA-256 was computed from it; every file also
//! matched the Hugging Face mirror commit the entry downloads from, by size
//! and by LFS SHA-256 or git blob id (checked 2026-10-07).
//!
//! # Attribution
//!
//! The Parakeet weights are CC-BY-4.0. [`PARAKEET_ATTRIBUTION`] is the
//! notice to show wherever the model is offered or credited. The Kokoro
//! weights are Apache-2.0; its bundle also carries espeak-ng's data, which is
//! GPL-3.0 like the espeak-ng code linked here (spec §17 D9).
//!
//! # Build
//!
//! `sherpa-onnx-sys`'s build script downloads the prebuilt static library of
//! its own version (21 MB for macOS arm64) from the k2-fsa GitHub release
//! into `<target>/sherpa-onnx-prebuilt/` once per target directory; nothing
//! is compiled. The download is not checksummed by that script. For an
//! offline or reproducible build, put the archive (for example
//! `sherpa-onnx-v1.13.8-osx-arm64-static-lib.tar.bz2`) in a directory and
//! set `SHERPA_ONNX_ARCHIVE_DIR` to it, or point `SHERPA_ONNX_LIB_DIR` at an
//! unpacked `lib/` directory.
//!
//! # A bad model
//!
//! sherpa-onnx reports some config errors as a failed create, which becomes
//! [`aulo_speech::SpeechError::Unavailable`], but onnxruntime throws C++
//! exceptions on a model whose graph does not match, and Rust cannot catch
//! those: the process aborts. So the files are checked first (catalog id,
//! pinned sizes, ONNX header, token table), and the id pins the SHA-256 that
//! `aulo-models` verified at download. What is left: a file damaged in place
//! with its size unchanged, or a fault inside onnxruntime on valid input,
//! still aborts the daemon. Only running the engine in a child process
//! removes that.

mod model;
mod stt;
mod tts;

pub use aulo_speech_local::{MAX_THREADS, MAX_TRANSCRIPT_BYTES, MAX_UTTERANCE_LIMIT};
pub use model::{KOKORO_MODEL_ID, PARAKEET_MODEL_ID};
pub use stt::{PARAKEET_ENGINE_ID, ParakeetConfig, ParakeetFactory, ParakeetStt};
pub use tts::{
    CHUNK_SAMPLES, KOKORO_ENGINE_ID, KOKORO_SAMPLE_RATE_HZ, KokoroConfig, KokoroFactory, KokoroTts,
    MAX_SPEECH_TEXT_BYTES,
};

/// The CC-BY-4.0 notice for the Parakeet weights. The licence asks for the
/// creator, a link to the licence and a note of changes.
pub const PARAKEET_ATTRIBUTION: &str = "Speech recognition uses Parakeet TDT 0.6B v3 by NVIDIA \
     (https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), licensed under CC BY 4.0 \
     (https://creativecommons.org/licenses/by/4.0/). Exported to ONNX and quantized to int8 \
     by the k2-fsa sherpa-onnx project.";
