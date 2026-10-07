//! What an engine says about itself: a stable id, its kind and its
//! capabilities. The registry selects and falls back on these without loading
//! or calling the engine, and clients show them in the engine picker.

use std::fmt;
use std::str::FromStr;

use crate::SpeechError;

/// Longest engine id, in bytes. Ids appear in config keys, CLI arguments and
/// notices, so they stay short.
pub const MAX_ENGINE_ID_BYTES: usize = 64;

const LANGUAGE_SUBTAG_SEPARATOR: char = '-';

/// The id config and CLI use to pick an engine (`engine = "sherpa-kokoro"`).
///
/// Plugins choose their own ids, so ids are untrusted: the alphabet is limited
/// to lowercase ASCII letters, digits, `-`, `_` and `.`, starting with a
/// letter, which keeps them safe to print and to use as TOML keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EngineId(String);

impl EngineId {
    pub fn new(id: &str) -> Result<Self, SpeechError> {
        const WHAT: &str = "engine id";
        if id.is_empty() || id.len() > MAX_ENGINE_ID_BYTES {
            return Err(SpeechError::invalid(WHAT, "must be 1 to 64 bytes"));
        }
        if !id.starts_with(|c: char| c.is_ascii_lowercase()) {
            return Err(SpeechError::invalid(WHAT, "must start with a-z"));
        }
        let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || "-_.".contains(c);
        if !id.chars().all(allowed) {
            return Err(SpeechError::invalid(
                WHAT,
                "only a-z, 0-9, '-', '_' and '.'",
            ));
        }
        Ok(Self(id.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EngineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for EngineId {
    type Err = SpeechError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// Which trait the engine implements; the registry keeps one list per kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EngineKind {
    Stt,
    Tts,
    Vad,
    KeywordSpotter,
    TurnDetector,
}

/// Languages an engine handles, as BCP 47 tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanguageSupport {
    /// Multilingual engines that detect or accept any language.
    Any,
    Listed(Vec<String>),
}

impl LanguageSupport {
    /// Compares primary subtags only, case-insensitively, so a request for
    /// `en-US` is served by an engine listing `en` and the other way round.
    /// Region-level choice is the voice's job, not the engine's.
    pub fn supports(&self, tag: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Listed(tags) => tags
                .iter()
                .any(|listed| primary_subtag(listed).eq_ignore_ascii_case(primary_subtag(tag))),
        }
    }
}

fn primary_subtag(tag: &str) -> &str {
    tag.split(LANGUAGE_SUBTAG_SEPARATOR).next().unwrap_or(tag)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    /// STT: partial transcripts arrive while audio is still coming in. TTS:
    /// the first audio arrives before all text is synthesized. Non-streaming
    /// engines only answer after `finish`.
    pub streaming: bool,
    pub languages: LanguageSupport,
    /// Works with no network connectivity at all once its models are on disk.
    pub offline: bool,
    /// Sends audio or text over a socket to another process or host. This is
    /// the privacy flag, and it differs from `!offline`: an OpenAI-compatible
    /// server on localhost (runa) needs the network stack but works offline.
    pub needs_network: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineInfo {
    pub id: EngineId,
    pub kind: EngineKind,
    /// Human-readable name for pickers; never used for selection.
    pub name: String,
    pub capabilities: Capabilities,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_id_accepts_the_config_alphabet() {
        for id in ["sherpa-kokoro", "openai.tts", "whisper_cpp", "eleven2"] {
            assert_eq!(EngineId::new(id).unwrap().as_str(), id);
        }
    }

    #[test]
    fn engine_id_rejects_unsafe_or_oversized_ids() {
        let too_long = "a".repeat(MAX_ENGINE_ID_BYTES + 1);
        for id in [
            "",
            "Kokoro",
            "1engine",
            "a b",
            "a/b",
            "tts\n",
            too_long.as_str(),
        ] {
            assert!(EngineId::new(id).is_err(), "{id:?} was accepted");
        }
    }

    #[test]
    fn languages_match_on_the_primary_subtag() {
        let listed = LanguageSupport::Listed(vec!["en".into(), "ru-RU".into()]);
        assert!(listed.supports("en-US"));
        assert!(listed.supports("RU"));
        assert!(!listed.supports("uk"));
        assert!(LanguageSupport::Any.supports("uk"));
    }
}
