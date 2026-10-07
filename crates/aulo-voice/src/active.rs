use std::fmt;

use aulo_speech::{
    AudioFormat, EngineId, EngineSpec, SpeechError, SttEngine, TtsEngine, TtsRequest, TurnId,
};
use aulo_types::{AuloEvent, NoticeLevel};

use crate::{Engine, EngineRegistry, RegistryError};

/// The engine one bot or chat speaks or listens with, and the ordered
/// candidates behind it.
///
/// Engines are built lazily, when an utterance or reply begins, and stay in
/// use while they work. A failing engine is dropped for the next candidate;
/// once every candidate has failed, the next begin starts over from the
/// first, so an engine that recovers (network back, plugin restarted) is
/// picked up again.
pub struct ActiveEngine<E: ?Sized> {
    candidates: Vec<EngineSpec>,
    /// Index of the candidate to build when there is no engine.
    next: usize,
    engine: Option<(EngineSpec, Box<E>)>,
    /// A switch waiting for the next begin, so that an utterance or a
    /// sentence already being spoken finishes on the engine it started with.
    pending: Option<Vec<EngineSpec>>,
}

pub type ActiveStt = ActiveEngine<dyn SttEngine>;
pub type ActiveTts = ActiveEngine<dyn TtsEngine>;

impl<E: ?Sized + Engine> ActiveEngine<E> {
    /// `candidates` usually comes from [`crate::candidates`].
    pub fn new(candidates: Vec<EngineSpec>) -> Self {
        Self {
            candidates,
            next: 0,
            engine: None,
            pending: None,
        }
    }

    /// Replaces the candidates from the next begin on. Called by
    /// `VoiceService.SetVoice`, `aulo voice use` and voice commands at any
    /// time, including mid-utterance.
    pub fn switch(&mut self, candidates: Vec<EngineSpec>) {
        self.pending = Some(candidates);
    }

    /// The engine in use, for pushing and polling after a successful begin.
    pub fn engine_mut(&mut self) -> Option<&mut E> {
        self.engine.as_mut().map(|(_, engine)| engine.as_mut())
    }

    /// The registry id of the engine in use. Taken from the spec, not from
    /// the engine's own info, because a plugin reports whatever it likes.
    pub fn active(&self) -> Option<&EngineId> {
        self.engine.as_ref().map(|(spec, _)| &spec.engine)
    }

    /// Reports an error the engine returned mid-utterance. An engine fault
    /// retires the engine, so the next begin moves on to the next candidate;
    /// the utterance itself is not moved to another engine halfway through.
    /// Other errors leave the engine in place.
    pub fn report(&mut self, error: &SpeechError, notify: &mut impl FnMut(AuloEvent)) {
        if error.should_fall_back()
            && let Some((spec, _)) = self.engine.take()
        {
            notify(fallback_notice(E::KIND, &spec, error));
        }
    }

    /// Applies a pending switch, then starts the first candidate that builds
    /// and accepts `start`, noting every engine it skips.
    fn begin_with<T>(
        &mut self,
        registry: &EngineRegistry<E>,
        notify: &mut impl FnMut(AuloEvent),
        mut start: impl FnMut(&mut E, &EngineSpec) -> Result<T, SpeechError>,
    ) -> Result<T, RegistryError> {
        if let Some(candidates) = self.pending.take() {
            self.candidates = candidates;
            self.next = 0;
            self.engine = None;
        }
        // Every pass either returns or uses up one candidate, so this ends.
        loop {
            if self.engine.is_none() {
                let Some(spec) = self.candidates.get(self.next).cloned() else {
                    self.next = 0;
                    return Err(RegistryError::NoEngine { kind: E::KIND });
                };
                self.next += 1;
                // Any build error falls back: building is the engine's own
                // business, so no build error can be a caller bug.
                match registry.build(&spec) {
                    Ok(engine) => self.engine = Some((spec, engine)),
                    Err(error) => {
                        notify(fallback_notice(E::KIND, &spec, &error));
                        continue;
                    }
                }
            }
            let Some((spec, engine)) = self.engine.as_mut() else {
                continue;
            };
            match start(engine.as_mut(), spec) {
                Ok(started) => return Ok(started),
                Err(error) if error.should_fall_back() => self.report(&error, notify),
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl ActiveStt {
    /// Starts an utterance; see [`SttEngine::begin`].
    pub fn begin(
        &mut self,
        registry: &EngineRegistry<dyn SttEngine>,
        notify: &mut impl FnMut(AuloEvent),
        turn_id: TurnId,
        language: Option<&str>,
    ) -> Result<(), RegistryError> {
        self.begin_with(registry, notify, |engine, _| {
            engine.begin(turn_id, language)
        })
    }
}

impl ActiveTts {
    /// Starts a reply with the candidate's voice and rate; see
    /// [`TtsEngine::begin`].
    pub fn begin(
        &mut self,
        registry: &EngineRegistry<dyn TtsEngine>,
        notify: &mut impl FnMut(AuloEvent),
        turn_id: TurnId,
        language: Option<&str>,
    ) -> Result<AudioFormat, RegistryError> {
        self.begin_with(registry, notify, |engine, spec| {
            engine.begin(&TtsRequest {
                turn_id,
                voice: spec.voice.as_deref(),
                language,
                rate: spec.rate,
            })
        })
    }
}

/// The id and the error detail are both bounded (id alphabet and length,
/// [`aulo_speech::ErrorDetail`] cap), so a plugin cannot flood clients here.
fn fallback_notice(kind: &str, spec: &EngineSpec, error: &SpeechError) -> AuloEvent {
    AuloEvent::Notice {
        level: NoticeLevel::Warn,
        message: format!(
            "{kind} engine `{}` failed, trying the next one: {error}",
            spec.engine
        ),
        source: Some(spec.engine.to_string()),
    }
}

impl<E: ?Sized + Engine> fmt::Debug for ActiveEngine<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActiveEngine")
            .field("kind", &E::KIND)
            .field("active", &self.active())
            .field("candidates", &self.candidates)
            .field("next", &self.next)
            .field("pending", &self.pending)
            .finish()
    }
}
