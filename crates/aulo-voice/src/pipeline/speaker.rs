//! The speaking task: reply sentences in, synthesized audio out.
//!
//! One speaker owns the TTS engine and the audio sink. It polls the engine
//! on a tick, hands what it produced to the sink (which takes as much as its
//! bounded buffer holds), and reports every accepted chunk to clients as a
//! [`aulo_types::TtsChunk`] event followed by the audio itself. A cancel
//! (barge-in, T8.5) silences the sink and drops the engine's queue at once.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use aulo_speech::{AudioFormat, AudioFrame, SpeechError, TtsPoll, TurnId};
use aulo_types::{AuloEvent, NoticeLevel, TtsChunk};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::pipeline::audio::AudioSink;
use crate::pipeline::{AudioChunk, Output, StateCell};
use crate::{ActiveTts, TtsRegistry};

/// How often the engine is polled while a reply is open. A local engine
/// produces a 100 ms chunk well inside this; the tick is not on the audio
/// path.
pub(crate) const SPEAKER_POLL: Duration = Duration::from_millis(10);

/// Engine polls per tick, so one tick cannot spin on a chatty engine.
const MAX_POLLS_PER_TICK: u32 = 8;

/// Longest chunk handed to the sink and to clients, in milliseconds. At the
/// pipeline rate this is 1600 samples, 6400 bytes of PCM16.
const CHUNK_MS: u64 = 100;

/// What the handle can ask of the speaker.
pub(crate) enum SpeakerCmd {
    /// Gain from 0.0 to 1.0.
    SetVolume(f32),
    /// Stop the audio now (barge-in, T8.5).
    Cancel,
}

/// What the turn driver sends the speaker.
pub(crate) enum SpeakerMsg {
    Speak {
        turn_id: TurnId,
        sentence: String,
    },
    /// The reply is complete; drain and go quiet.
    EndReply {
        turn_id: TurnId,
    },
    /// The turn was cancelled; drop the audio.
    Cancel {
        turn_id: TurnId,
    },
}

/// The reply being spoken.
struct Speaking {
    turn_id: TurnId,
    /// Position in the reply, so a client can detect a dropped chunk.
    seq: u32,
    format: AudioFormat,
}

pub(crate) struct Speaker {
    tts: ActiveTts,
    tts_registry: Arc<TtsRegistry>,
    sink: Box<dyn AudioSink>,
    /// BCP 47 tag of the reply, from the transcript's language.
    language: Option<String>,
    listening_open: Arc<AtomicBool>,
    state: Arc<StateCell>,
    outputs: mpsc::Sender<Output>,
    /// Signals the turn driver that the reply has drained.
    reply_done: mpsc::Sender<()>,
    speaking: Option<Speaking>,
    /// Engine output the sink has not taken yet, interleaved.
    pending: Vec<f32>,
    /// Reused poll buffer, sized when a reply begins.
    buffer: Vec<f32>,
}

impl Speaker {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        tts: ActiveTts,
        tts_registry: Arc<TtsRegistry>,
        sink: Box<dyn AudioSink>,
        language: Option<String>,
        listening_open: Arc<AtomicBool>,
        state: Arc<StateCell>,
        outputs: mpsc::Sender<Output>,
        reply_done: mpsc::Sender<()>,
    ) -> Self {
        Self {
            tts,
            tts_registry,
            sink,
            language,
            listening_open,
            state,
            outputs,
            reply_done,
            speaking: None,
            pending: Vec::new(),
            buffer: Vec::new(),
        }
    }

    pub(crate) async fn run(
        mut self,
        mut commands: mpsc::Receiver<SpeakerCmd>,
        mut messages: mpsc::Receiver<SpeakerMsg>,
        cancel: CancellationToken,
    ) {
        while !cancel.is_cancelled() {
            while let Ok(command) = commands.try_recv() {
                match command {
                    SpeakerCmd::SetVolume(volume) => self.set_volume(volume),
                    SpeakerCmd::Cancel => self.cancel(None),
                }
            }
            while let Ok(message) = messages.try_recv() {
                self.on_message(message);
            }
            if self.speaking.is_some() {
                self.poll_tts();
            }
            tokio::time::sleep(SPEAKER_POLL).await;
        }
    }

    fn on_message(&mut self, message: SpeakerMsg) {
        match message {
            SpeakerMsg::Speak { turn_id, sentence } => self.speak(turn_id, sentence),
            SpeakerMsg::EndReply { turn_id } => self.end_reply(turn_id),
            SpeakerMsg::Cancel { turn_id } => self.cancel(Some(turn_id)),
        }
    }

    fn speak(&mut self, turn_id: TurnId, sentence: String) {
        if self
            .speaking
            .as_ref()
            .is_some_and(|speaking| speaking.turn_id != turn_id)
        {
            // A newer turn arrived before the older reply drained.
            self.cancel(None);
        }
        if self.speaking.is_none() && !self.begin_reply(turn_id) {
            return;
        }
        let error = match self.tts.engine_mut() {
            Some(engine) => engine.push_text(&sentence).err(),
            None => None,
        };
        if let Some(error) = error {
            self.report(&error);
        }
        self.poll_tts();
    }

    /// Starts a reply on the first candidate that builds. Returns false when
    /// no engine could start, so the caller drops the sentence.
    fn begin_reply(&mut self, turn_id: TurnId) -> bool {
        let mut notices = Vec::new();
        let begun = self.tts.begin(
            &self.tts_registry,
            &mut |event| notices.push(event),
            turn_id,
            self.language.as_deref(),
        );
        for notice in notices {
            self.emit(notice);
        }
        match begun {
            Ok(format) => {
                let channels = usize::from(format.channels()).max(1);
                let chunk = (u64::from(format.sample_rate_hz()) * CHUNK_MS / 1_000)
                    .saturating_mul(channels as u64);
                let chunk = usize::try_from(chunk).unwrap_or(usize::from(u16::MAX));
                self.buffer.resize(chunk, 0.0);
                self.speaking = Some(Speaking {
                    turn_id,
                    seq: 0,
                    format,
                });
                true
            }
            Err(error) => {
                self.emit(AuloEvent::Notice {
                    level: NoticeLevel::Warn,
                    message: format!("tts engine could not start: {error}"),
                    source: None,
                });
                false
            }
        }
    }

    fn end_reply(&mut self, turn_id: TurnId) {
        if let Some(speaking) = self.speaking.as_mut() {
            if speaking.turn_id != turn_id {
                return;
            }
            if let Some(engine) = self.tts.engine_mut() {
                let _ = engine.finish();
            }
            // Polling continues until the engine reports Done.
            self.poll_tts();
            return;
        }
        // No audio ever started: an empty reply, or every engine failed. The
        // turn is still open, so the driver waits for this signal.
        if self.state.is_active() {
            self.finish_turn();
        }
    }

    fn poll_tts(&mut self) {
        let Some(speaking) = self.speaking.as_ref() else {
            return;
        };
        let (turn_id, format) = (speaking.turn_id, speaking.format);
        for _ in 0..MAX_POLLS_PER_TICK {
            let polled = match self.tts.engine_mut() {
                Some(engine) => Some(engine.poll(&mut self.buffer)),
                None => None,
            };
            match polled {
                Some(Ok(TtsPoll::Audio { samples })) => {
                    let take = samples.min(self.buffer.len());
                    self.pending.extend_from_slice(&self.buffer[..take]);
                    self.flush_pending(turn_id, format);
                }
                Some(Ok(TtsPoll::Pending)) => break,
                // Done, or the engine is gone: the reply is over.
                _ => {
                    self.finish_speaking();
                    break;
                }
            }
        }
    }

    /// Hands the sink as much queued audio as it takes, in whole channel
    /// groups, and reports each accepted run to clients.
    fn flush_pending(&mut self, turn_id: TurnId, format: AudioFormat) {
        let channels = usize::from(format.channels()).max(1);
        while !self.pending.is_empty() {
            let frame = match AudioFrame::new(&self.pending, format, 0) {
                Ok(frame) => frame,
                Err(_) => {
                    // Cannot happen: only whole groups are ever queued.
                    self.pending.clear();
                    return;
                }
            };
            let accepted = self.sink.write(frame);
            if accepted == 0 {
                return;
            }
            let take = (accepted.saturating_mul(channels)).min(self.pending.len());
            let chunk: Vec<f32> = self.pending[..take].to_vec();
            self.pending.drain(..take);
            self.emit_audio(turn_id, format, chunk);
        }
    }

    fn emit_audio(&mut self, turn_id: TurnId, format: AudioFormat, samples: Vec<f32>) {
        let Some(speaking) = self.speaking.as_mut() else {
            return;
        };
        let seq = speaking.seq;
        speaking.seq = speaking.seq.wrapping_add(1);
        let channels = usize::from(format.channels()).max(1);
        let per_channel = u64::try_from(samples.len() / channels).unwrap_or(u64::MAX);
        let duration_ms = u32::try_from(format.position_to_ms(per_channel)).unwrap_or(u32::MAX);
        // The first audio the client hears is the moment the user knows the
        // agent is speaking.
        self.state.speaking(&self.outputs);
        self.emit(AuloEvent::TtsChunk(TtsChunk {
            turn_id,
            seq,
            sample_rate_hz: format.sample_rate_hz(),
            channels: format.channels(),
            duration_ms,
            last: false,
        }));
        self.emit_output(Output::Audio(AudioChunk {
            turn_id,
            seq,
            sample_rate_hz: format.sample_rate_hz(),
            channels: format.channels(),
            samples,
        }));
    }

    fn finish_speaking(&mut self) {
        let Some(speaking) = self.speaking.take() else {
            return;
        };
        self.flush_pending(speaking.turn_id, speaking.format);
        // A zero-length last chunk marks the end for clients that count.
        self.emit(AuloEvent::TtsChunk(TtsChunk {
            turn_id: speaking.turn_id,
            seq: speaking.seq,
            sample_rate_hz: speaking.format.sample_rate_hz(),
            channels: speaking.format.channels(),
            duration_ms: 0,
            last: true,
        }));
        self.pending.clear();
        self.sink.finish();
        self.finish_turn();
    }

    /// The reply is over: back to listening or idle, and the turn driver may
    /// take the next submission.
    fn finish_turn(&mut self) {
        let listening = self.listening_open.load(Ordering::Relaxed);
        self.state.end(listening, &self.outputs);
        let _ = self.reply_done.try_send(());
    }

    /// Barge-in and turn cancellation: silence now, drop what is queued.
    /// `turn_id` ignores a cancel for a turn that already ended.
    fn cancel(&mut self, turn_id: Option<TurnId>) {
        if let Some(turn_id) = turn_id
            && self
                .speaking
                .as_ref()
                .is_some_and(|speaking| speaking.turn_id != turn_id)
        {
            return;
        }
        let was_speaking = self.speaking.take().is_some();
        if was_speaking {
            if let Some(engine) = self.tts.engine_mut() {
                engine.cancel();
            }
            self.sink.stop();
        }
        self.pending.clear();
        // Only a turn that is actually open waits for this signal; a stray
        // cancel must not release the next turn early.
        if was_speaking || self.state.is_active() {
            self.finish_turn();
        }
    }

    fn set_volume(&mut self, volume: f32) {
        if let Err(error) = self.sink.set_volume(volume) {
            self.emit(AuloEvent::Notice {
                level: NoticeLevel::Warn,
                message: format!("could not set the volume: {error}"),
                source: None,
            });
        }
    }

    fn report(&mut self, error: &SpeechError) {
        let mut notices = Vec::new();
        self.tts.report(error, &mut |event| notices.push(event));
        for notice in notices {
            self.emit(notice);
        }
    }

    fn emit(&mut self, event: AuloEvent) {
        let _ = self.outputs.try_send(Output::Event(event));
    }

    fn emit_output(&mut self, output: Output) {
        let _ = self.outputs.try_send(output);
    }
}
