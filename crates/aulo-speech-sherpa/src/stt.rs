use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use aulo_models::Model;
use aulo_speech::{
    Capabilities, EngineInfo, EngineKind, EngineSpec, LanguageSupport, PIPELINE_SAMPLE_RATE_HZ,
    SpeechError, SttEngine,
};
use aulo_speech_local::{
    OfflineStt, SttJob, Worker, check_threads, decoder, max_utterance_samples,
};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};

use crate::model::{PARAKEET_MODEL_ID, ParakeetFiles};

/// Parakeet TDT v3 on one VAD segment at a time, decoded whole on the worker
/// after `finish`. Not streaming: no partials.
pub type ParakeetStt = OfflineStt;

/// The id config uses to pick this engine (`[voice.stt] engine = ...`).
pub const PARAKEET_ENGINE_ID: &str = "sherpa-parakeet";

/// The 25 languages on the model card, checked 2026-10-07:
/// <https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3>. The model detects
/// which one it hears, so a language hint only selects the engine.
const LANGUAGES: [&str; 25] = [
    "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt", "lv", "mt",
    "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "uk",
];

/// sherpa-onnx's name for the NeMo TDT transducer layout.
const MODEL_TYPE: &str = "nemo_transducer";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParakeetConfig {
    /// Decoding threads. Four gave a real-time factor of 0.07 on an M3 Max,
    /// one gave 0.25 (spike T0.5, R3).
    pub threads: u16,
    /// Longest VAD segment kept; audio beyond it is dropped as overflow.
    pub max_utterance: Duration,
}

impl Default for ParakeetConfig {
    fn default() -> Self {
        Self {
            threads: 4,
            max_utterance: Duration::from_secs(60),
        }
    }
}

/// Builds [`ParakeetStt`] engines for the registry. The model is loaded on
/// the first build, not here, so a daemon that never selects this engine
/// never pays its 1.4 GB; every engine built afterwards shares that load.
#[derive(Debug)]
pub struct ParakeetFactory {
    files: ParakeetFiles,
    threads: u16,
    max_samples: usize,
    worker: OnceLock<Result<Worker<SttJob>, SpeechError>>,
}

impl ParakeetFactory {
    /// `model` is the catalog entry and `dir` its install directory from
    /// `ModelManager::path`. The files are validated here, before sherpa-onnx
    /// ever sees them.
    pub fn new(model: &Model, dir: &Path, config: ParakeetConfig) -> Result<Self, SpeechError> {
        check_threads(config.threads)?;
        let max_samples = max_utterance_samples(config.max_utterance)?;
        Ok(Self {
            files: ParakeetFiles::locate(model, dir)?,
            threads: config.threads,
            max_samples,
            worker: OnceLock::new(),
        })
    }

    /// A failed load is remembered: every later build fails at once with the
    /// same error, so the registry falls back without retrying a broken model
    /// on every utterance.
    pub fn build(&self, spec: &EngineSpec) -> Result<Box<dyn SttEngine>, SpeechError> {
        if spec
            .model
            .as_deref()
            .is_some_and(|m| m != PARAKEET_MODEL_ID)
        {
            return Err(SpeechError::unsupported("model is not available here"));
        }
        let worker = self
            .worker
            .get_or_init(|| {
                let (files, threads) = (self.files.clone(), self.threads);
                decoder("aulo-sherpa-stt", move || load(files, threads))
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

fn load(
    files: ParakeetFiles,
    threads: u16,
) -> Result<impl FnMut(&[f32]) -> Result<String, SpeechError>, SpeechError> {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(files.encoder),
        decoder: Some(files.decoder),
        joiner: Some(files.joiner),
    };
    config.model_config.tokens = Some(files.tokens);
    config.model_config.model_type = Some(MODEL_TYPE.into());
    config.model_config.num_threads = i32::from(threads);
    config.model_config.provider = Some("cpu".into());
    config.decoding_method = Some("greedy_search".into());
    let recognizer = OfflineRecognizer::create(&config)
        .ok_or_else(|| SpeechError::unavailable("sherpa-onnx refused the Parakeet model"))?;
    // The rate always fits: it is the pipeline constant.
    let rate = PIPELINE_SAMPLE_RATE_HZ as i32;
    Ok(move |samples: &[f32]| {
        let stream = recognizer.create_stream();
        stream.accept_waveform(rate, samples);
        recognizer.decode(&stream);
        stream
            .get_result()
            .map(|result| result.text)
            .ok_or_else(|| SpeechError::failed("Parakeet returned no result"))
    })
}

/// The engine description for the registry and pickers.
fn info(spec: &EngineSpec) -> EngineInfo {
    EngineInfo {
        id: spec.engine.clone(),
        kind: EngineKind::Stt,
        name: "NVIDIA Parakeet TDT 0.6B v3 (sherpa-onnx)".into(),
        capabilities: Capabilities {
            streaming: false,
            languages: LanguageSupport::Listed(
                LANGUAGES.iter().map(|&tag| tag.to_owned()).collect(),
            ),
            offline: true,
            needs_network: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use aulo_speech::{EngineId, SpeechRate};
    use aulo_speech_local::MAX_UTTERANCE_LIMIT;

    use super::*;

    fn spec(model: Option<&str>) -> EngineSpec {
        EngineSpec {
            engine: EngineId::new(PARAKEET_ENGINE_ID).unwrap(),
            model: model.map(str::to_owned),
            voice: None,
            rate: SpeechRate::NORMAL,
        }
    }

    #[test]
    fn the_engine_is_offline_and_not_streaming() {
        let info = info(&spec(None));
        assert!(!info.capabilities.streaming && info.capabilities.offline);
        assert!(info.capabilities.languages.supports("ru-RU"));
        assert!(!info.capabilities.languages.supports("ja"));
    }

    #[test]
    fn factory_validates_config_and_model_before_loading() {
        let catalog = aulo_models::Catalog::embedded().unwrap();
        let model = catalog.get(PARAKEET_MODEL_ID).unwrap();
        let missing = tempfile::tempdir().unwrap();
        let error = ParakeetFactory::new(model, missing.path(), ParakeetConfig::default());
        assert!(matches!(error, Err(SpeechError::Unavailable(_))));
        for config in [
            ParakeetConfig {
                threads: 0,
                ..ParakeetConfig::default()
            },
            ParakeetConfig {
                max_utterance: MAX_UTTERANCE_LIMIT + Duration::from_secs(1),
                ..ParakeetConfig::default()
            },
        ] {
            let error = ParakeetFactory::new(model, missing.path(), config).unwrap_err();
            assert!(matches!(error, SpeechError::Invalid { .. }));
        }
    }

    #[test]
    fn another_model_name_is_refused_without_loading() {
        let (dir, model) = crate::model::tests::bundle(crate::model::tests::TOKEN_TABLE);
        let factory = ParakeetFactory::new(&model, dir.path(), ParakeetConfig::default()).unwrap();
        let built = factory.build(&spec(Some("whisper-large")));
        assert!(matches!(built, Err(SpeechError::Unsupported(_))));
        assert!(factory.worker.get().is_none());
    }
}
