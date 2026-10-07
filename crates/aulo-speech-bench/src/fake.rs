//! A scripted speech-to-text engine for tests and benchmarks. The fake in
//! `aulo_speech::testkit` caps its audio at a few frames, far less than a clip.

use aulo_speech::{
    AudioFrame, Capabilities, EngineId, EngineInfo, EngineKind, LanguageSupport, SpeechError,
    SttEngine, SttPoll, Transcript, TranscriptKind, TurnId,
};

/// Answers `partial` (when set) once audio is flowing and `final_text` after
/// `finish`, whatever the audio is.
#[derive(Debug)]
pub struct ScriptedStt {
    info: EngineInfo,
    languages: Vec<String>,
    partial: Option<String>,
    final_text: String,
    turn: Option<TurnId>,
    heard: bool,
    partial_sent: bool,
    finished: bool,
    final_sent: bool,
}

impl ScriptedStt {
    pub fn new(id: &str, final_text: &str) -> Result<Self, SpeechError> {
        Ok(Self {
            info: EngineInfo {
                id: EngineId::new(id)?,
                kind: EngineKind::Stt,
                name: id.to_owned(),
                capabilities: Capabilities {
                    streaming: true,
                    languages: LanguageSupport::Any,
                    offline: true,
                    needs_network: false,
                },
            },
            languages: Vec::new(),
            partial: None,
            final_text: final_text.to_owned(),
            turn: None,
            heard: false,
            partial_sent: false,
            finished: false,
            final_sent: false,
        })
    }

    /// Emits this partial after the first frame.
    pub fn with_partial(mut self, text: &str) -> Self {
        self.partial = Some(text.to_owned());
        self
    }

    /// Refuses every language but these (primary subtags) with `Unsupported`.
    pub fn with_languages(mut self, languages: &[&str]) -> Self {
        self.languages = languages.iter().map(|l| (*l).to_owned()).collect();
        self
    }
}

impl SttEngine for ScriptedStt {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn begin(&mut self, turn_id: TurnId, language: Option<&str>) -> Result<(), SpeechError> {
        if let Some(tag) = language
            && !self.languages.is_empty()
            && !self.languages.iter().any(|l| l == tag)
        {
            return Err(SpeechError::unsupported(tag));
        }
        self.cancel();
        self.turn = Some(turn_id);
        Ok(())
    }

    fn push(&mut self, _frame: AudioFrame<'_>) -> Result<(), SpeechError> {
        if self.turn.is_none() || self.finished {
            return Err(SpeechError::OutOfOrder("push outside an utterance"));
        }
        self.heard = true;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), SpeechError> {
        self.finished = true;
        Ok(())
    }

    fn poll(&mut self, out: &mut Transcript) -> Result<SttPoll, SpeechError> {
        let Some(turn) = self.turn else {
            return Ok(SttPoll::Pending);
        };
        if self.final_sent {
            return Ok(SttPoll::Done);
        }
        if self.finished {
            self.final_sent = true;
            out.set(turn, TranscriptKind::Final, &self.final_text, None);
            return Ok(SttPoll::Updated);
        }
        if let Some(text) = &self.partial
            && self.heard
            && !self.partial_sent
        {
            self.partial_sent = true;
            out.set(turn, TranscriptKind::Partial, text, None);
            return Ok(SttPoll::Updated);
        }
        Ok(SttPoll::Pending)
    }

    fn cancel(&mut self) {
        self.turn = None;
        self.heard = false;
        self.partial_sent = false;
        self.finished = false;
        self.final_sent = false;
    }
}
