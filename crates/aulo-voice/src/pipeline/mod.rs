//! The voice conversation pipeline (spec §4.3, T8.1).
//!
//! One pipeline is one voice conversation: microphone frames in, agent turns
//! and speech out, as three tokio tasks joined by bounded channels.
//!
//! ```text
//!   frames ──► listener ──► utterances ──► turn driver ──► sentences ──► speaker
//!   (source    VAD, STT,   (bounded)      session,        (bounded)    TTS,
//!    owned)    end of turn                 normalizer                   sink
//! ```
//!
//! - The **listener** owns the frame source, the VAD, the end-of-turn
//!   detector and the STT engine. It is the one audio owner: nothing else
//!   reads the microphone while a pipeline lives (spec §5.3).
//! - The **turn driver** owns the agent session and the speakable-text
//!   normalizer. The session is the T4.8 seam: aulo-agent's chat session
//!   behind `SessionService.Converse` in production, a scripted session in
//!   tests.
//! - The **speaker** owns the TTS engine and the audio sink, and reports
//!   every chunk to clients as a [`aulo_types::TtsChunk`] plus the audio.
//!
//! Every channel is bounded and every send on the audio-adjacent paths is
//! non-blocking: a slow client loses audio with a notice instead of stalling
//! the microphone. States are [`aulo_types::VoiceState`]: Idle, Listening,
//! Thinking, Speaking.
//!
//! The daemon implements [`FrameSource`] and [`AudioSink`] over `aulo-audio`
//! (`Capture` and `Playback`); remote `VoiceService.Talk` clients use a sink
//! that forwards to the stream instead.

mod agent;
mod audio;
mod eot;
mod listener;
mod session;
mod speaker;

pub use audio::{AudioSink, FrameSource};
pub use eot::{DEFAULT_SILENCE_MS, SilenceEndOfTurn};
pub use session::{SessionError, SessionEvent, Submission, VoiceSession};

use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use aulo_speech::{EngineSpec, Vad};
use aulo_types::{AuloEvent, ChatId, TurnId, VoiceState};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{ActiveStt, ActiveTts, Language, SttRegistry, TtsRegistry};

/// Client-facing output: events and the audio that goes with them, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    Event(AuloEvent),
    /// Synthesized audio, interleaved `f32` at `sample_rate_hz`. The gRPC
    /// layer converts it to the PCM16 the proto carries.
    Audio(AudioChunk),
}

/// One run of synthesized speech. The [`aulo_types::TtsChunk`] event with the
/// same `turn_id` and `seq` is always emitted first.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioChunk {
    pub turn_id: TurnId,
    pub seq: u32,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub samples: Vec<f32>,
}

/// How the microphone is armed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListenMode {
    /// Live only between `start_listening` and `stop_listening`: the desktop
    /// mic button, the global hotkey and `VoiceService` push-to-talk.
    #[default]
    PushToTalk,
    /// Always live: speech opens an utterance, end of turn closes it. This is
    /// the mode after a wake word (T8.6) arms the pipeline.
    AlwaysListening,
}

/// What speaking over the agent does to the turn in progress (T8.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BargeInMode {
    /// Stop the playback and cancel the turn.
    #[default]
    Interrupt,
    /// Stop the playback; the turn keeps running.
    Continue,
}

/// Pipeline settings that are not engine choices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineConfig {
    pub listen_mode: ListenMode,
    pub barge_in: BargeInMode,
    /// Silence after the user stops before the turn ends (spec §5.1).
    pub silence_ms: u32,
    /// Default spoken language: the normalizer and TTS follow it unless the
    /// transcript says otherwise.
    pub language: Language,
    /// Cap on the transcript handed to the agent, in characters. STT output
    /// is untrusted input.
    pub max_transcript_chars: usize,
    /// Pause between frame polls when the source has nothing ready.
    pub frame_poll: Duration,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            listen_mode: ListenMode::default(),
            barge_in: BargeInMode::default(),
            silence_ms: DEFAULT_SILENCE_MS,
            language: Language::En,
            max_transcript_chars: MAX_TRANSCRIPT_CHARS,
            frame_poll: listener::FRAME_POLL,
        }
    }
}

/// Longest transcript handed to the agent, in characters. Comfortably longer
/// than any spoken sentence; it exists so a broken or hostile engine cannot
/// submit a page of text.
pub const MAX_TRANSCRIPT_CHARS: usize = 8_192;

/// Everything the pipeline needs from the outside world.
pub struct PipelineDeps {
    /// The chat the transcripts go to.
    pub chat_id: ChatId,
    /// The agent seam (T4.8).
    pub session: Arc<dyn VoiceSession>,
    pub stt_registry: Arc<SttRegistry>,
    pub tts_registry: Arc<TtsRegistry>,
    /// Engine candidates, best first, from `crate::candidates`.
    pub stt: Vec<EngineSpec>,
    pub tts: Vec<EngineSpec>,
    /// The voice activity detector (T8.2 picks the backend).
    pub vad: Box<dyn Vad>,
    /// The microphone.
    pub source: Box<dyn FrameSource>,
    /// The speaker, or the channel of a remote client.
    pub sink: Box<dyn AudioSink>,
    /// BCP 47 hint for STT; `None` asks the engine to detect the language.
    pub stt_language: Option<String>,
}

impl fmt::Debug for PipelineDeps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PipelineDeps")
            .field("chat_id", &self.chat_id)
            .field("stt", &self.stt)
            .field("tts", &self.tts)
            .field("stt_language", &self.stt_language)
            .finish_non_exhaustive()
    }
}

/// What the handle can ask of the pipeline.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// The pipeline tasks are gone: the handle was shut down or a task died.
    #[error("the voice pipeline has stopped")]
    Stopped,
    #[error("invalid {what}: {reason}")]
    Invalid {
        what: &'static str,
        reason: &'static str,
    },
}

/// Bounded channels: events and audio for one client, submissions for the
/// agent, and the small command channels of the two audio tasks.
const OUTPUT_CHANNEL: usize = 128;
const SUBMISSION_CHANNEL: usize = 8;
const COMMAND_CHANNEL: usize = 8;
const SENTENCE_CHANNEL: usize = 16;
const REPLY_DONE_CHANNEL: usize = 4;

/// Builds and runs the pipeline; the returned handle drives and reads it.
pub fn spawn(config: PipelineConfig, deps: PipelineDeps) -> VoiceHandle {
    let (outputs, output_rx) = mpsc::channel(OUTPUT_CHANNEL);
    let (submissions, submission_rx) = mpsc::channel(SUBMISSION_CHANNEL);
    let (listener_cmds, listener_cmd_rx) = mpsc::channel(COMMAND_CHANNEL);
    let (speaker_cmds, speaker_cmd_rx) = mpsc::channel(COMMAND_CHANNEL);
    let (sentences, sentence_rx) = mpsc::channel(SENTENCE_CHANNEL);
    let (reply_done, reply_done_rx) = mpsc::channel(REPLY_DONE_CHANNEL);

    let initial = match config.listen_mode {
        ListenMode::PushToTalk => VoiceState::Idle,
        ListenMode::AlwaysListening => VoiceState::Listening,
    };
    let state = Arc::new(StateCell::new(initial));
    let listening_open = Arc::new(AtomicBool::new(initial == VoiceState::Listening));
    let cancel = CancellationToken::new();

    let core = listener::ListenerCore {
        mode: config.listen_mode,
        vad: deps.vad,
        eot: SilenceEndOfTurn::new(config.silence_ms),
        stt: ActiveStt::new(deps.stt),
        stt_registry: Arc::clone(&deps.stt_registry),
        chat_id: deps.chat_id,
        language: deps.stt_language,
        max_transcript_chars: config.max_transcript_chars,
        poll_interval: config.frame_poll,
        utterance: None,
        in_speech: false,
        listening_open: Arc::clone(&listening_open),
        transcript: aulo_speech::Transcript::with_capacity(TurnId::new(), TRANSCRIPT_CAPACITY),
        outputs: outputs.clone(),
        submissions,
        state: Arc::clone(&state),
    };
    let listener = listener::Listener::new(deps.source, core);

    let speaker = speaker::Speaker::new(
        ActiveTts::new(deps.tts),
        deps.tts_registry,
        deps.sink,
        Some(config.language.tag().to_owned()),
        Arc::clone(&listening_open),
        Arc::clone(&state),
        outputs.clone(),
        reply_done,
    );

    let tasks = vec![
        tokio::spawn(listener.run(listener_cmd_rx, cancel.clone())),
        tokio::spawn(speaker.run(speaker_cmd_rx, sentence_rx, cancel.clone())),
        tokio::spawn(agent::run(
            deps.session,
            config.language,
            submission_rx,
            sentences,
            reply_done_rx,
            outputs,
            Arc::clone(&state),
            Arc::clone(&listening_open),
            cancel.clone(),
        )),
    ];

    VoiceHandle {
        listener_cmds,
        speaker_cmds,
        outputs: output_rx,
        state,
        listening_open,
        cancel,
        tasks,
    }
}

/// Room the transcript buffer starts with; engines overwrite it in place.
const TRANSCRIPT_CAPACITY: usize = 256;

/// The control and read side of a running pipeline. One handle, one owner:
/// the daemon's local voice session or one `VoiceService.Talk` stream.
pub struct VoiceHandle {
    listener_cmds: mpsc::Sender<listener::ListenerCmd>,
    speaker_cmds: mpsc::Sender<speaker::SpeakerCmd>,
    outputs: mpsc::Receiver<Output>,
    state: Arc<StateCell>,
    listening_open: Arc<AtomicBool>,
    cancel: CancellationToken,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl VoiceHandle {
    /// Where the pipeline is right now.
    pub fn state(&self) -> VoiceState {
        self.state.get()
    }

    /// True while the microphone is live for utterances.
    pub fn is_listening(&self) -> bool {
        self.listening_open.load(Ordering::Relaxed)
    }

    /// The next event or audio chunk, or `None` once the pipeline is gone.
    pub async fn next_output(&mut self) -> Option<Output> {
        self.outputs.recv().await
    }

    /// Arms the microphone (push-to-talk, the desktop mic button, the global
    /// hotkey). In `AlwaysListening` it is already live.
    pub async fn start_listening(&self) -> Result<(), PipelineError> {
        self.listener_cmds
            .send(listener::ListenerCmd::StartListening)
            .await
            .map_err(|_| PipelineError::Stopped)
    }

    /// Ends the utterance in progress and disarms push-to-talk.
    pub async fn stop_listening(&self) -> Result<(), PipelineError> {
        self.listener_cmds
            .send(listener::ListenerCmd::StopListening)
            .await
            .map_err(|_| PipelineError::Stopped)
    }

    /// Playback gain from 0.0 (mute) to 1.0.
    pub async fn set_volume(&self, volume: f32) -> Result<(), PipelineError> {
        if !(0.0..=1.0).contains(&volume) {
            return Err(PipelineError::Invalid {
                what: "volume",
                reason: "must be within 0.0..=1.0",
            });
        }
        self.speaker_cmds
            .send(speaker::SpeakerCmd::SetVolume(volume))
            .await
            .map_err(|_| PipelineError::Stopped)
    }

    /// Stops the playback now (barge-in, T8.5).
    pub async fn cancel_speech(&self) -> Result<(), PipelineError> {
        self.speaker_cmds
            .send(speaker::SpeakerCmd::Cancel)
            .await
            .map_err(|_| PipelineError::Stopped)
    }

    /// Stops every task and waits for it. Dropping the handle cancels too,
    /// but only this method joins the tasks, so the audio streams are known
    /// to be closed when it returns.
    pub async fn shutdown(mut self) {
        self.cancel.cancel();
        for task in self.tasks.drain(..) {
            let _ = task.await;
        }
    }
}

impl fmt::Debug for VoiceHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VoiceHandle")
            .field("state", &self.state.get())
            .field("tasks", &self.tasks.len())
            .finish_non_exhaustive()
    }
}

impl Drop for VoiceHandle {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// The pipeline's state, shared by the three tasks. A mutex (not an atomic)
/// because a state change and the event that announces it must not interleave
/// with another task's.
#[derive(Debug)]
pub(crate) struct StateCell {
    state: Mutex<VoiceState>,
}

impl StateCell {
    fn new(initial: VoiceState) -> Self {
        Self {
            state: Mutex::new(initial),
        }
    }

    pub(crate) fn get(&self) -> VoiceState {
        *lock(&self.state)
    }

    /// True while a turn holds the pipeline: the reply is being spoken or
    /// waited for.
    pub(crate) fn is_active(&self) -> bool {
        matches!(self.get(), VoiceState::Speaking | VoiceState::Thinking)
    }

    /// Announces the state the pipeline starts in. Unlike [`set`](Self::set)
    /// this always emits, so a client sees the initial state even when it
    /// equals the default.
    pub(crate) fn announce(&self, outputs: &mpsc::Sender<Output>) {
        announce(self.get(), outputs);
    }

    /// Unconditional: the spawn state and the start of a turn.
    pub(crate) fn set(&self, next: VoiceState, outputs: &mpsc::Sender<Output>) {
        {
            let mut state = lock(&self.state);
            if *state == next {
                return;
            }
            *state = next;
        }
        announce(next, outputs);
    }

    /// Only from Idle: an utterance opening never downgrades Thinking or
    /// Speaking.
    pub(crate) fn enter(&self, next: VoiceState, outputs: &mpsc::Sender<Output>) {
        {
            let mut state = lock(&self.state);
            if *state != VoiceState::Idle || *state == next {
                return;
            }
            *state = next;
        }
        announce(next, outputs);
    }

    /// The first audio of a reply: the user hears that the agent is speaking.
    pub(crate) fn speaking(&self, outputs: &mpsc::Sender<Output>) {
        {
            let mut state = lock(&self.state);
            if !matches!(*state, VoiceState::Thinking | VoiceState::Idle) {
                return;
            }
            *state = VoiceState::Speaking;
        }
        announce(VoiceState::Speaking, outputs);
    }

    /// From Speaking or Thinking: the reply is over.
    pub(crate) fn end(&self, listening: bool, outputs: &mpsc::Sender<Output>) {
        let next = if listening {
            VoiceState::Listening
        } else {
            VoiceState::Idle
        };
        {
            let mut state = lock(&self.state);
            if !matches!(*state, VoiceState::Speaking | VoiceState::Thinking) {
                return;
            }
            *state = next;
        }
        announce(next, outputs);
    }
}

fn announce(state: VoiceState, outputs: &mpsc::Sender<Output>) {
    let _ = outputs.try_send(Output::Event(AuloEvent::VoiceState { state }));
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
