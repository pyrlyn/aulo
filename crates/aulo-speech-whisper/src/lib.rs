//! Local speech recognition on whisper.cpp: [`WhisperStt`], OpenAI Whisper
//! `tiny.en` in ggml format, decoded offline per VAD segment. The engine
//! pushes no partials and hears English only; other languages fall back to
//! another engine.
//!
//! The decoding itself is `speech_capture::Transcriber`, the code cox and runa
//! share; this crate adds what aulo's engine contract needs around it: the
//! catalog model check, a worker thread that loads the model once, bounded
//! audio buffering and a transcript cap (all from `aulo-speech-local`).
//!
//! # Build
//!
//! whisper.cpp is a cmake C++ build, so it is compiled only with the
//! `whisper` cargo feature, which needs cmake and a C++ toolchain. Without
//! it this crate still validates configuration and model files, and
//! [`WhisperFactory::build`] returns [`SpeechError::Unavailable`], so the
//! registry falls back instead of the build failing.
//!
//! # Model
//!
//! [`WHISPER_MODEL_ID`] is the `aulo-models` catalog entry for
//! `ggml-tiny.en.bin` (78 MB) from the `ggerganov/whisper.cpp` repository,
//! pinned to a commit and a SHA-256. Whisper's weights are MIT-licensed
//! (<https://huggingface.co/ggerganov/whisper.cpp> model card, checked
//! 2026-10-07; the OpenAI repository is MIT too), so they need no on-screen
//! attribution beyond keeping the licence with the file.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use aulo_models::{Model, ModelKind};
use aulo_speech::{
    Capabilities, EngineInfo, EngineKind, EngineSpec, LanguageSupport, SpeechError, SttEngine,
};
use aulo_speech_local::{
    OfflineStt, SttJob, Worker, check_installed, decoder, max_utterance_samples,
};

/// The id config uses to pick this engine (`[voice.stt] engine = ...`).
pub const WHISPER_ENGINE_ID: &str = "whisper-cpp";

/// The catalog id of the only model this engine loads. Pinning the id pins the
/// file's size and hash, so a model that is not this exact one never reaches
/// whisper.cpp.
pub const WHISPER_MODEL_ID: &str = "whisper-tiny.en";

const MODEL_FILE: &str = "ggml-tiny.en.bin";

/// The `.en` models were trained on English only; a language hint other than
/// this is refused so the registry can try an engine that knows it.
const LANGUAGE: &str = "en";

/// Whisper decodes in 30 s windows and walks longer audio window by window, so
/// the cap is about memory, not about the model.
const DEFAULT_MAX_UTTERANCE: Duration = Duration::from_secs(60);

/// Whisper tiny.en on one VAD segment at a time, decoded whole on the worker
/// after `finish`. Not streaming: no partials.
pub type WhisperStt = OfflineStt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperConfig {
    /// Longest VAD segment kept; audio beyond it is dropped as overflow.
    pub max_utterance: Duration,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            max_utterance: DEFAULT_MAX_UTTERANCE,
        }
    }
}

/// Builds [`WhisperStt`] engines for the registry. The model is loaded on the
/// first build, not here, so a daemon that never selects this engine never
/// pays for it; every engine built afterwards shares that load.
#[derive(Debug)]
pub struct WhisperFactory {
    model_file: PathBuf,
    max_samples: usize,
    worker: OnceLock<Result<Worker<SttJob>, SpeechError>>,
}

impl WhisperFactory {
    /// `model` is the catalog entry and `dir` its install directory from
    /// `ModelManager::path`. The file is checked here, before whisper.cpp
    /// ever sees it.
    pub fn new(model: &Model, dir: &Path, config: WhisperConfig) -> Result<Self, SpeechError> {
        let max_samples = max_utterance_samples(config.max_utterance)?;
        check_installed(model, dir, WHISPER_MODEL_ID, ModelKind::Stt)?;
        Ok(Self {
            model_file: dir.join(MODEL_FILE),
            max_samples,
            worker: OnceLock::new(),
        })
    }

    /// A failed load is remembered: every later build fails at once with the
    /// same error, so the registry falls back without reloading a broken model
    /// on every utterance.
    pub fn build(&self, spec: &EngineSpec) -> Result<Box<dyn SttEngine>, SpeechError> {
        if spec.model.as_deref().is_some_and(|m| m != WHISPER_MODEL_ID) {
            return Err(SpeechError::unsupported("model is not available here"));
        }
        let worker = self
            .worker
            .get_or_init(|| {
                let file = self.model_file.clone();
                decoder("aulo-whisper-stt", move || load(&file))
            })
            .clone()?;
        Ok(Box::new(OfflineStt::new(
            info(spec),
            worker,
            self.max_samples,
        )))
    }

    /// The closure form `EngineRegistry::register` takes.
    pub fn into_factory(
        self,
    ) -> impl Fn(&EngineSpec) -> Result<Box<dyn SttEngine>, SpeechError> + Send + Sync + 'static
    {
        move |spec| self.build(spec)
    }
}

#[cfg(feature = "whisper")]
fn load(
    file: &Path,
) -> Result<impl FnMut(&[f32]) -> Result<String, SpeechError> + use<>, SpeechError> {
    let transcriber = speech_capture::Transcriber::load(file)
        .map_err(|error| SpeechError::unavailable(&error.to_string()))?;
    Ok(move |samples: &[f32]| {
        transcriber
            .transcribe(samples, Some(LANGUAGE))
            .map_err(|error| SpeechError::failed(&error.to_string()))
    })
}

/// A named function type, so the error-only body has a decoder type to return.
#[cfg(not(feature = "whisper"))]
type Decode = fn(&[f32]) -> Result<String, SpeechError>;

#[cfg(not(feature = "whisper"))]
fn load(_file: &Path) -> Result<Decode, SpeechError> {
    Err(SpeechError::unavailable(
        "built without the `whisper` feature",
    ))
}

fn info(spec: &EngineSpec) -> EngineInfo {
    EngineInfo {
        id: spec.engine.clone(),
        kind: EngineKind::Stt,
        name: "OpenAI Whisper tiny.en (whisper.cpp)".into(),
        capabilities: Capabilities {
            streaming: false,
            languages: LanguageSupport::Listed(vec![LANGUAGE.to_owned()]),
            offline: true,
            needs_network: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use aulo_models::Catalog;
    use aulo_speech::{EngineId, SpeechRate};

    use super::*;

    fn spec(model: Option<&str>) -> EngineSpec {
        EngineSpec {
            engine: EngineId::new(WHISPER_ENGINE_ID).unwrap(),
            model: model.map(str::to_owned),
            voice: None,
            rate: SpeechRate::NORMAL,
        }
    }

    fn installed() -> (tempfile::TempDir, Model) {
        let catalog = Catalog::embedded().unwrap();
        let model = catalog.get(WHISPER_MODEL_ID).unwrap().clone();
        let dir = tempfile::tempdir().unwrap();
        for file in &model.files {
            // Sparse: the check reads sizes only.
            let handle = fs::File::create(dir.path().join(&file.path)).unwrap();
            handle.set_len(file.size).unwrap();
        }
        (dir, model)
    }

    #[test]
    fn the_catalog_entry_is_the_ggml_file_this_engine_loads() {
        let catalog = Catalog::embedded().unwrap();
        let model = catalog.get(WHISPER_MODEL_ID).unwrap();
        assert_eq!(model.kind, ModelKind::Stt);
        assert_eq!(model.license, "MIT");
        assert_eq!(model.files.len(), 1);
        assert_eq!(model.files[0].path, MODEL_FILE);
    }

    #[test]
    fn the_engine_is_english_only_offline_and_not_streaming() {
        let (dir, model) = installed();
        let factory = WhisperFactory::new(&model, dir.path(), WhisperConfig::default()).unwrap();
        assert!(factory.worker.get().is_none(), "nothing is loaded yet");
        let info = info(&spec(None));
        assert!(!info.capabilities.streaming && info.capabilities.offline);
        assert!(!info.capabilities.needs_network);
        assert!(info.capabilities.languages.supports("en-GB"));
        assert!(!info.capabilities.languages.supports("ru"));
    }

    #[test]
    fn config_and_files_are_validated_before_loading() {
        let (dir, model) = installed();
        let zero = WhisperConfig {
            max_utterance: Duration::ZERO,
        };
        let error = WhisperFactory::new(&model, dir.path(), zero).unwrap_err();
        assert!(matches!(error, SpeechError::Invalid { .. }));

        let empty = tempfile::tempdir().unwrap();
        let error = WhisperFactory::new(&model, empty.path(), WhisperConfig::default());
        assert!(matches!(error, Err(SpeechError::Unavailable(_))));

        fs::write(dir.path().join(MODEL_FILE), b"<html>404</html>").unwrap();
        let error = WhisperFactory::new(&model, dir.path(), WhisperConfig::default());
        assert!(matches!(error, Err(SpeechError::Unavailable(_))));
    }

    #[test]
    fn another_model_name_is_refused_without_loading() {
        let (dir, model) = installed();
        let factory = WhisperFactory::new(&model, dir.path(), WhisperConfig::default()).unwrap();
        let built = factory.build(&spec(Some("whisper-large")));
        assert!(matches!(built, Err(SpeechError::Unsupported(_))));
        assert!(factory.worker.get().is_none());
    }

    #[cfg(not(feature = "whisper"))]
    #[test]
    fn without_the_feature_the_engine_is_unavailable_so_voice_falls_back() {
        let (dir, model) = installed();
        let factory = WhisperFactory::new(&model, dir.path(), WhisperConfig::default()).unwrap();
        let error = factory.build(&spec(None)).err().unwrap();
        assert!(matches!(error, SpeechError::Unavailable(_)));
        assert!(error.should_fall_back());
    }

    /// With the feature on, a file of the right size that is not a ggml model
    /// is an ordinary error, never a crash: the transcriber checks the magic
    /// before whisper.cpp reads anything.
    #[cfg(feature = "whisper")]
    #[test]
    fn a_right_sized_file_that_is_not_ggml_is_unavailable() {
        let (dir, model) = installed();
        let factory = WhisperFactory::new(&model, dir.path(), WhisperConfig::default()).unwrap();
        let error = factory.build(&spec(None)).err().unwrap();
        assert!(matches!(error, SpeechError::Unavailable(_)), "{error:?}");
        // Remembered, not retried.
        assert_eq!(factory.build(&spec(None)).err().unwrap(), error);
    }
}
