//! The engines the benchmark can try on this machine. Nothing is loaded or
//! contacted here: a [`Candidate`] builds its engine only when it is run, and
//! a missing model, a missing key or a platform without the API makes it
//! `Unavailable` instead of an error, so one broken engine never hides the
//! others (fail open).

use std::path::{Path, PathBuf};

use aulo_models::{Catalog, ModelManager};
use aulo_speech::{EngineId, EngineSpec, SpeechError, SpeechRate, SttEngine, TtsEngine, Voice};
use aulo_speech_cloud::{
    ApiKey, CloudSttConfig, CloudSttFactory, CloudTtsConfig, CloudTtsFactory, DEEPGRAM_BASE_URL,
    DeepgramSttConfig, DeepgramSttFactory, ElevenLabsTtsConfig, ElevenLabsTtsFactory,
    openai_voices,
};
use aulo_speech_sherpa::{
    KOKORO_ENGINE_ID, KOKORO_MODEL_ID, KokoroConfig, KokoroFactory, PARAKEET_ENGINE_ID,
    PARAKEET_MODEL_ID, ParakeetConfig, ParakeetFactory,
};
use aulo_speech_whisper::{WHISPER_ENGINE_ID, WHISPER_MODEL_ID, WhisperConfig, WhisperFactory};
use tokio::runtime::Handle;

use crate::report::Kind;

const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
// Model names checked 2026-10-07 in OpenAI's changelog,
// https://developers.openai.com/api/docs/changelog: `gpt-transcribe` replaces
// the transcription models shut down on 2027-02-26; `gpt-4o-mini-tts` is current.
const OPENAI_STT_MODEL: &str = "gpt-transcribe";
const OPENAI_TTS_MODEL: &str = "gpt-4o-mini-tts";
const OPENAI_TTS_VOICE: &str = "alloy";
// `eleven_flash_v2_5` and the premade voice "Rachel" are ElevenLabs' documented
// low-latency defaults (unverified here: their docs refuse automated fetches).
const ELEVENLABS_MODEL: &str = "eleven_flash_v2_5";
const ELEVENLABS_VOICE: &str = "21m00Tcm4TlvDq8ikWAM";
const DEEPGRAM_MODEL: &str = "nova-3";

pub enum Engine {
    Stt(Box<dyn SttEngine>),
    Tts(Box<dyn TtsEngine>),
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Stt(_) => "Engine::Stt",
            Self::Tts(_) => "Engine::Tts",
        })
    }
}

type Build = Box<dyn FnOnce() -> Result<Engine, String> + Send>;

/// One engine to benchmark. `Err` from the build is the reason it is unavailable.
pub struct Candidate {
    pub kind: Kind,
    pub id: String,
    pub build: Build,
}

impl std::fmt::Debug for Candidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Candidate")
            .field("kind", &self.kind)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl Candidate {
    pub fn stt(
        id: &str,
        build: impl FnOnce() -> Result<Box<dyn SttEngine>, String> + Send + 'static,
    ) -> Self {
        Self::new(Kind::Stt, id, Box::new(move || build().map(Engine::Stt)))
    }

    pub fn tts(
        id: &str,
        build: impl FnOnce() -> Result<Box<dyn TtsEngine>, String> + Send + 'static,
    ) -> Self {
        Self::new(Kind::Tts, id, Box::new(move || build().map(Engine::Tts)))
    }

    fn new(kind: Kind, id: &str, build: Build) -> Self {
        Self {
            kind,
            id: id.to_owned(),
            build,
        }
    }
}

/// Cloud keys. The names are the vendors' own, so a key already exported for
/// their tools works; a key that is unset or empty leaves the engine out of the
/// network entirely.
#[derive(Debug, Default, Clone)]
pub struct Keys {
    pub openai: Option<ApiKey>,
    pub elevenlabs: Option<ApiKey>,
    pub deepgram: Option<ApiKey>,
}

impl Keys {
    pub fn from_env() -> Self {
        let read = |name| {
            std::env::var(name)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .map(ApiKey::new)
        };
        Self {
            openai: read("OPENAI_API_KEY"),
            elevenlabs: read("ELEVENLABS_API_KEY"),
            deepgram: read("DEEPGRAM_API_KEY"),
        }
    }
}

/// "engine unavailable: x" under an "unavailable" label says it twice.
fn why(error: SpeechError) -> String {
    match error {
        SpeechError::Unavailable(detail) => detail.to_string(),
        other => other.to_string(),
    }
}

fn spec(id: &str) -> Result<EngineSpec, String> {
    Ok(EngineSpec {
        engine: EngineId::new(id).map_err(why)?,
        model: None,
        voice: None,
        rate: SpeechRate::NORMAL,
    })
}

/// The directory of an installed catalog model, or why there is none.
fn model_dir(home: &Path, id: &str) -> Result<(aulo_models::Model, PathBuf), String> {
    let catalog = Catalog::embedded().map_err(|e| e.to_string())?;
    let model = catalog
        .get(id)
        .cloned()
        .ok_or(format!("`{id}` is not in the catalog"))?;
    let manager = ModelManager::new(home, catalog).map_err(|e| e.to_string())?;
    let dir = manager
        .path(id)
        .ok_or(format!("model `{id}` is not installed"))?;
    Ok((model, dir))
}

fn no_key(variable: &str) -> String {
    format!("no key: set {variable}")
}

/// Every engine of this build, in table order.
pub fn builtin(home: &Path, runtime: &Handle, keys: &Keys) -> Vec<Candidate> {
    let mut all = vec![
        sherpa_parakeet(home.to_owned()),
        whisper_cpp(home.to_owned()),
        system_stt(),
        cloud_stt(
            "openai",
            OPENAI_API_KEY,
            keys.openai.clone(),
            runtime.clone(),
            openai_stt,
        ),
        cloud_stt(
            "deepgram",
            DEEPGRAM_API_KEY,
            keys.deepgram.clone(),
            runtime.clone(),
            deepgram,
        ),
        sherpa_kokoro(home.to_owned()),
        system_tts(),
    ];
    all.push(cloud_tts(
        "openai",
        OPENAI_API_KEY,
        keys.openai.clone(),
        runtime.clone(),
        openai_tts,
    ));
    all.push(cloud_tts(
        "elevenlabs",
        ELEVENLABS_API_KEY,
        keys.elevenlabs.clone(),
        runtime.clone(),
        elevenlabs,
    ));
    all
}

const OPENAI_API_KEY: &str = "OPENAI_API_KEY";
const DEEPGRAM_API_KEY: &str = "DEEPGRAM_API_KEY";
const ELEVENLABS_API_KEY: &str = "ELEVENLABS_API_KEY";

fn sherpa_parakeet(home: PathBuf) -> Candidate {
    Candidate::stt(PARAKEET_ENGINE_ID, move || {
        let (model, dir) = model_dir(&home, PARAKEET_MODEL_ID)?;
        let factory = ParakeetFactory::new(&model, &dir, ParakeetConfig::default()).map_err(why)?;
        factory.build(&spec(PARAKEET_ENGINE_ID)?).map_err(why)
    })
}

fn whisper_cpp(home: PathBuf) -> Candidate {
    Candidate::stt(WHISPER_ENGINE_ID, move || {
        let (model, dir) = model_dir(&home, WHISPER_MODEL_ID)?;
        let factory = WhisperFactory::new(&model, &dir, WhisperConfig::default()).map_err(why)?;
        factory.build(&spec(WHISPER_ENGINE_ID)?).map_err(why)
    })
}

fn sherpa_kokoro(home: PathBuf) -> Candidate {
    Candidate::tts(KOKORO_ENGINE_ID, move || {
        let (model, dir) = model_dir(&home, KOKORO_MODEL_ID)?;
        let factory = KokoroFactory::new(&model, &dir, KokoroConfig::default()).map_err(why)?;
        factory.build(&spec(KOKORO_ENGINE_ID)?).map_err(why)
    })
}

#[cfg(target_os = "macos")]
fn system_stt() -> Candidate {
    Candidate::stt(aulo_speech_system::ENGINE_ID, || {
        aulo_speech_system::stt_factory(&spec(aulo_speech_system::ENGINE_ID)?).map_err(why)
    })
}

#[cfg(not(target_os = "macos"))]
fn system_stt() -> Candidate {
    Candidate::stt("system", || {
        Err("the system recognizer is only wired on macOS".to_owned())
    })
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
fn system_tts() -> Candidate {
    Candidate::tts(aulo_speech_system::ENGINE_ID, || {
        aulo_speech_system::factory(&spec(aulo_speech_system::ENGINE_ID)?).map_err(why)
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn system_tts() -> Candidate {
    Candidate::tts("system", || {
        Err("no system voices on this platform".to_owned())
    })
}

fn cloud_stt(
    id: &str,
    variable: &'static str,
    key: Option<ApiKey>,
    runtime: Handle,
    build: fn(ApiKey, Handle) -> Result<Box<dyn SttEngine>, String>,
) -> Candidate {
    Candidate::stt(id, move || {
        build(key.ok_or_else(|| no_key(variable))?, runtime)
    })
}

fn cloud_tts(
    id: &str,
    variable: &'static str,
    key: Option<ApiKey>,
    runtime: Handle,
    build: fn(ApiKey, Handle) -> Result<Box<dyn TtsEngine>, String>,
) -> Candidate {
    Candidate::tts(id, move || {
        build(key.ok_or_else(|| no_key(variable))?, runtime)
    })
}

fn openai_stt(key: ApiKey, runtime: Handle) -> Result<Box<dyn SttEngine>, String> {
    let mut config = CloudSttConfig::new("OpenAI", OPENAI_BASE_URL, OPENAI_STT_MODEL);
    config.api_key = Some(key);
    let factory = CloudSttFactory::new(config, runtime).map_err(why)?;
    factory.build(&spec("openai")?).map_err(why)
}

fn openai_tts(key: ApiKey, runtime: Handle) -> Result<Box<dyn TtsEngine>, String> {
    let mut config = CloudTtsConfig::new(
        "OpenAI",
        OPENAI_BASE_URL,
        OPENAI_TTS_MODEL,
        openai_voices(),
        OPENAI_TTS_VOICE,
    );
    config.api_key = Some(key);
    let factory = CloudTtsFactory::new(config, runtime).map_err(why)?;
    factory.build(&spec("openai")?).map_err(why)
}

fn deepgram(key: ApiKey, runtime: Handle) -> Result<Box<dyn SttEngine>, String> {
    let mut config = DeepgramSttConfig::new("Deepgram", DEEPGRAM_BASE_URL, DEEPGRAM_MODEL);
    config.api_key = Some(key);
    let factory = DeepgramSttFactory::new(config, runtime).map_err(why)?;
    factory.build(&spec("deepgram")?).map_err(why)
}

fn elevenlabs(key: ApiKey, runtime: Handle) -> Result<Box<dyn TtsEngine>, String> {
    let voice = Voice {
        id: ELEVENLABS_VOICE.to_owned(),
        name: "Rachel".to_owned(),
        language: None,
    };
    let mut config = ElevenLabsTtsConfig::new(ELEVENLABS_MODEL, vec![voice], ELEVENLABS_VOICE);
    config.tts.api_key = Some(key);
    let factory = ElevenLabsTtsFactory::new(config, runtime).map_err(why)?;
    factory.build(&spec("elevenlabs")?).map_err(why)
}
