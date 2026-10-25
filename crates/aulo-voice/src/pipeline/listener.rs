//! The microphone task: frames in, utterances out.
//!
//! One listener owns the frame source, the VAD, the end-of-turn detector and
//! the STT engine, which is what "one audio owner at a time" (spec §5.3)
//! means in practice: nothing else reads the microphone while a pipeline
//! lives. The listener never blocks: a full agent queue or a full client
//! event channel drops with a notice instead of stalling the audio.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use aulo_speech::{
    AudioFrame, SpeechError, SttPoll, TranscriptKind, TurnDecision, TurnDetector, TurnId, Vad,
    VadEvent,
};
use aulo_types::{AuloEvent, NoticeLevel, VoiceState};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::pipeline::audio::FrameSource;
use crate::pipeline::eot::SilenceEndOfTurn;
use crate::pipeline::session::Submission;
use crate::pipeline::{ListenMode, Output, StateCell};
use crate::{ActiveStt, SttRegistry};

/// Pause between frame polls when the source has nothing ready. Frames arrive
/// every 20 ms, so this adds at most this much latency to a turn.
pub(crate) const FRAME_POLL: Duration = Duration::from_millis(5);

/// Engine polls per frame, per detector or engine. A streaming engine only
/// answers when it has something new, so this bounds the work one frame can
/// cause without dropping real partials.
const MAX_POLLS_PER_FRAME: u32 = 32;

/// What the handle can ask of the microphone.
pub(crate) enum ListenerCmd {
    /// Push-to-talk: the microphone is live from here.
    StartListening,
    /// Push-to-talk: end the utterance now.
    StopListening,
}

/// Where an utterance is in its life.
pub(crate) enum Utterance {
    /// Audio still flows in; the end-of-turn detector runs.
    Open { turn_id: TurnId },
    /// The turn ended; the final transcript is being polled for.
    Closing { turn_id: TurnId },
}

impl Utterance {
    fn turn_id(&self) -> TurnId {
        match self {
            Self::Open { turn_id } | Self::Closing { turn_id } => *turn_id,
        }
    }

    fn is_open(&self) -> bool {
        matches!(self, Self::Open { .. })
    }
}

/// The microphone half of the pipeline. The frame source is kept apart from
/// the core so a borrowed frame never pins the whole listener.
pub(crate) struct Listener {
    source: Box<dyn FrameSource>,
    core: ListenerCore,
}

pub(crate) struct ListenerCore {
    pub(crate) mode: ListenMode,
    pub(crate) vad: Box<dyn Vad>,
    pub(crate) eot: SilenceEndOfTurn,
    pub(crate) stt: ActiveStt,
    pub(crate) stt_registry: Arc<SttRegistry>,
    pub(crate) chat_id: aulo_types::ChatId,
    /// BCP 47 hint for STT; `None` asks the engine to detect the language.
    pub(crate) language: Option<String>,
    /// Cap on the transcript handed to the agent, in characters. STT output
    /// is untrusted: an engine or a plugin must not be able to submit a page.
    pub(crate) max_transcript_chars: usize,
    /// Pause between frame polls when the source has nothing ready.
    pub(crate) poll_interval: Duration,
    pub(crate) utterance: Option<Utterance>,
    pub(crate) in_speech: bool,
    /// True while the microphone is live for utterances: always in
    /// `AlwaysListening`, between start and stop in push-to-talk.
    pub(crate) listening_open: Arc<AtomicBool>,
    pub(crate) transcript: aulo_speech::Transcript,
    pub(crate) outputs: mpsc::Sender<Output>,
    pub(crate) submissions: mpsc::Sender<Submission>,
    pub(crate) state: Arc<StateCell>,
}

impl Listener {
    pub(crate) fn new(source: Box<dyn FrameSource>, core: ListenerCore) -> Self {
        Self { source, core }
    }

    /// Runs until the pipeline shuts down.
    pub(crate) async fn run(
        mut self,
        mut commands: mpsc::Receiver<ListenerCmd>,
        cancel: CancellationToken,
    ) {
        self.core.announce();
        while !cancel.is_cancelled() {
            while let Ok(command) = commands.try_recv() {
                self.core.command(command);
            }
            // The frame borrows the source, and the core is a different
            // field, so the two borrows never overlap.
            let frame = self.source.next_frame();
            if let Some(frame) = frame {
                self.core.on_frame(frame);
                continue;
            }
            tokio::time::sleep(self.core.frame_poll()).await;
        }
    }
}

impl ListenerCore {
    /// Announces the state the pipeline starts in, so a client attached at
    /// spawn sees it. The state itself was set when the pipeline was built.
    fn announce(&self) {
        self.state.announce(&self.outputs);
    }

    fn frame_poll(&self) -> Duration {
        self.poll_interval
    }

    fn command(&mut self, command: ListenerCmd) {
        // Start and stop are push-to-talk controls; in AlwaysListening the
        // microphone is live for the whole life of the pipeline.
        if self.mode != ListenMode::PushToTalk {
            return;
        }
        match command {
            ListenerCmd::StartListening => {
                let was_open = self.listening_open.swap(true, Ordering::Relaxed);
                if !was_open {
                    // Open an utterance straight away: audio flows while the
                    // button is held, and the silence timeout still ends it.
                    self.open_utterance();
                }
            }
            ListenerCmd::StopListening => {
                let was_open = self.listening_open.swap(false, Ordering::Relaxed);
                if was_open && self.utterance.as_ref().is_some_and(Utterance::is_open) {
                    self.close_utterance();
                }
            }
        }
    }

    fn on_frame(&mut self, frame: AudioFrame<'_>) {
        let format = frame.format();
        let event = match self.vad.push(frame) {
            Ok(event) => event,
            Err(error) => {
                self.notice_engine("vad", &error);
                None
            }
        };
        if let Some(event) = event {
            match event {
                VadEvent::SpeechStarted { position } => {
                    self.in_speech = true;
                    self.emit(AuloEvent::SpeechStarted {
                        at_ms: format.position_to_ms(position),
                    });
                    if self.utterance.is_none() && self.listening_open.load(Ordering::Relaxed) {
                        self.open_utterance();
                    }
                }
                VadEvent::SpeechEnded { position } => {
                    self.in_speech = false;
                    self.emit(AuloEvent::SpeechEnded {
                        at_ms: format.position_to_ms(position),
                    });
                }
            }
        }
        let (turn_id, open) = match &self.utterance {
            Some(utterance) => (Some(utterance.turn_id()), utterance.is_open()),
            None => (None, false),
        };
        let Some(turn_id) = turn_id else { return };
        if open {
            if self.in_speech {
                self.push_to_stt(frame);
            }
            let decision = match self.eot.push(frame, self.in_speech) {
                Ok(decision) => decision,
                Err(error) => {
                    self.notice_engine("end of turn", &error);
                    TurnDecision::Continue
                }
            };
            self.poll_partials(turn_id);
            if matches!(decision, TurnDecision::EndOfTurn { .. }) {
                self.close_utterance();
            }
        } else {
            self.poll_final();
        }
    }

    fn open_utterance(&mut self) {
        let turn_id = TurnId::new();
        let mut notices = Vec::new();
        let begun = self.stt.begin(
            &self.stt_registry,
            &mut |event| notices.push(event),
            turn_id,
            self.language.as_deref(),
        );
        for notice in notices {
            self.emit(notice);
        }
        match begun {
            Ok(()) => {
                self.utterance = Some(Utterance::Open { turn_id });
                self.eot.reset();
                self.state.enter(VoiceState::Listening, &self.outputs);
            }
            Err(error) => {
                // Fail open: nothing was heard, but the pipeline keeps
                // listening for the next utterance.
                self.emit(AuloEvent::Notice {
                    level: NoticeLevel::Warn,
                    message: format!("stt engine could not start: {error}"),
                    source: None,
                });
            }
        }
    }

    fn close_utterance(&mut self) {
        let Some(Utterance::Open { turn_id }) = &self.utterance else {
            return;
        };
        let turn_id = *turn_id;
        self.utterance = Some(Utterance::Closing { turn_id });
        let error = match self.stt.engine_mut() {
            Some(engine) => engine.finish().err(),
            None => None,
        };
        if let Some(error) = error {
            self.report(&error);
        }
        self.eot.reset();
        self.poll_final();
    }

    fn push_to_stt(&mut self, frame: AudioFrame<'_>) {
        let error = match self.stt.engine_mut() {
            Some(engine) => engine.push(frame).err(),
            None => None,
        };
        if let Some(error) = error {
            // An overflow is the engine's own bound doing its job: it counted
            // the drop, and the turn goes on. Anything else retires the
            // engine for the next candidate (fail open).
            if !matches!(error, SpeechError::Overflow { .. }) {
                self.report(&error);
            }
        }
    }

    fn poll_partials(&mut self, turn_id: TurnId) {
        for _ in 0..MAX_POLLS_PER_FRAME {
            let polled = match self.stt.engine_mut() {
                Some(engine) => Some(engine.poll(&mut self.transcript)),
                None => None,
            };
            match polled {
                Some(Ok(SttPoll::Updated)) => {
                    if self.transcript.turn_id != turn_id {
                        // Stale: a network engine answered for an utterance
                        // that is already over.
                        break;
                    }
                    if self.transcript.kind == TranscriptKind::Partial {
                        self.emit_transcript(turn_id);
                    }
                }
                _ => break,
            }
        }
    }

    fn poll_final(&mut self) {
        let Some(Utterance::Closing { turn_id }) = &self.utterance else {
            return;
        };
        let turn_id = *turn_id;
        for _ in 0..MAX_POLLS_PER_FRAME {
            let polled = match self.stt.engine_mut() {
                Some(engine) => Some(engine.poll(&mut self.transcript)),
                None => None,
            };
            match polled {
                Some(Ok(SttPoll::Updated)) => {
                    if self.transcript.turn_id != turn_id {
                        break;
                    }
                    match self.transcript.kind {
                        TranscriptKind::Partial => self.emit_transcript(turn_id),
                        TranscriptKind::Final => self.deliver_final(turn_id),
                        // Newer kinds are ignored until the pipeline knows
                        // what to do with them.
                        _ => {}
                    }
                }
                // The engine is still working: the next frame polls again.
                Some(Ok(SttPoll::Pending)) => break,
                Some(Ok(SttPoll::Done)) => {
                    self.utterance = None;
                    break;
                }
                Some(Err(error)) => {
                    self.report(&error);
                    self.utterance = None;
                    break;
                }
                // No engine: nothing more will come for this utterance.
                None => {
                    self.utterance = None;
                    break;
                }
            }
        }
    }

    fn deliver_final(&mut self, turn_id: TurnId) {
        let language = self.transcript.language.clone();
        let capped: String = self
            .transcript
            .text
            .chars()
            .take(self.max_transcript_chars)
            .collect();
        self.emit(AuloEvent::Transcript {
            turn_id,
            kind: TranscriptKind::Final,
            text: capped.clone(),
            language: language.clone(),
        });
        let text = capped.trim();
        if text.is_empty() {
            return;
        }
        let submission = Submission {
            chat_id: self.chat_id,
            turn_id,
            text: text.to_owned(),
            language,
        };
        if self.submissions.try_send(submission).is_err() {
            self.emit(AuloEvent::Notice {
                level: NoticeLevel::Warn,
                message: "the agent is busy; the utterance was dropped".to_owned(),
                source: None,
            });
        }
    }

    fn emit_transcript(&mut self, turn_id: TurnId) {
        self.emit(AuloEvent::Transcript {
            turn_id,
            kind: self.transcript.kind,
            text: self.transcript.text.clone(),
            language: self.transcript.language.clone(),
        });
    }

    /// Retires an engine that failed mid-utterance and notes every engine
    /// the registry skipped on the way.
    fn report(&mut self, error: &SpeechError) {
        let mut notices = Vec::new();
        self.stt.report(error, &mut |event| notices.push(event));
        for notice in notices {
            self.emit(notice);
        }
    }

    fn notice_engine(&mut self, what: &str, error: &SpeechError) {
        self.emit(AuloEvent::Notice {
            level: NoticeLevel::Warn,
            message: format!("{what} failed: {error}"),
            source: None,
        });
    }

    fn emit(&mut self, event: AuloEvent) {
        let _ = self.outputs.try_send(Output::Event(event));
    }
}
