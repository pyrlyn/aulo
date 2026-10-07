//! Turns a raw `[voice.stt]` / `[voice.tts]` section into the validated
//! [`EngineChoice`] the engine registry resolves. Validation lives in the
//! speech contract; this only maps its errors to config errors with the key
//! and, when known, the layer that set it.

use aulo_speech::{EngineChoice, EngineId, SpeechError, SpeechRate};

use crate::{ConfigError, EngineConfig, Provenance};

impl EngineConfig {
    /// `key` is the section's dotted path (`voice.tts`), used in errors and to
    /// look the offending value's layer up in `provenance`.
    pub fn choice(
        &self,
        key: &str,
        provenance: Option<&Provenance>,
    ) -> Result<EngineChoice, ConfigError> {
        let error = |field: &str, cause: SpeechError| {
            let path = format!("{key}.{field}");
            ConfigError {
                origin: provenance.and_then(|p| p.get(&path)).cloned(),
                message: format!("{cause} for key `{path}`"),
            }
        };
        let engine = self
            .engine
            .as_deref()
            .map(EngineId::new)
            .transpose()
            .map_err(|e| error("engine", e))?;
        let rate = self
            .rate
            .map(SpeechRate::new)
            .transpose()
            .map_err(|e| error("rate", e))?;
        let fallback = self
            .fallback
            .iter()
            .map(|id| EngineId::new(id))
            .collect::<Result<_, _>>()
            .map_err(|e| error("fallback", e))?;
        Ok(EngineChoice {
            engine,
            model: self.model.clone(),
            voice: self.voice.clone(),
            rate,
            fallback,
        })
    }
}
