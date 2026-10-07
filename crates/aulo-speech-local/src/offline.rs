//! A speech-to-text engine for models that decode a whole utterance at once.
//! Frames are buffered in storage sized once for the longest utterance, and
//! after `finish` the worker thread decodes the lot. There are no partials.
//! The engine knows nothing about the model: it is handed a decode function.

use std::mem;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::time::Duration;

use aulo_speech::{
    AudioFormat, AudioFrame, EngineInfo, PIPELINE_SAMPLE_RATE_HZ, SpeechError, SttEngine, SttPoll,
    Transcript, TranscriptKind, TurnId,
};

use crate::worker::{Job, Worker};

/// Longest transcript handed to the pipeline, in bytes. Model output is
/// untrusted text that ends up in a prompt; a minute of speech is about a
/// kilobyte, so this only bites on a misbehaving model.
pub const MAX_TRANSCRIPT_BYTES: usize = 16 * 1024;

/// Upper bound for the longest utterance an engine accepts. The audio buffer
/// is sized for it up front, and decoding memory grows with its length.
pub const MAX_UTTERANCE_LIMIT: Duration = Duration::from_secs(300);

/// Utterances waiting behind the one being decoded.
const JOB_QUEUE: usize = 4;

/// The audio buffer size, in samples, for an engine that keeps utterances up
/// to `max_utterance` long.
pub fn max_utterance_samples(max_utterance: Duration) -> Result<usize, SpeechError> {
    if max_utterance.is_zero() || max_utterance > MAX_UTTERANCE_LIMIT {
        return Err(SpeechError::invalid("max utterance", "0 to 5 minutes"));
    }
    Ok((max_utterance.as_secs_f64() * f64::from(PIPELINE_SAMPLE_RATE_HZ)) as usize)
}

/// One finished utterance to decode.
#[derive(Debug)]
pub struct SttJob {
    samples: Vec<f32>,
    cancelled: Arc<AtomicBool>,
    reply: SyncSender<Reply>,
}

impl Job for SttJob {
    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// The transcript, plus the audio buffer handed back so the engine reuses
/// its allocation for the next utterance.
#[derive(Debug)]
struct Reply {
    text: Result<String, SpeechError>,
    samples: Vec<f32>,
}

/// Starts the worker thread. `load` runs there and returns the decode
/// function, which then serves one utterance per job; a load error reaches
/// the caller.
pub fn decoder<L, D>(thread_name: &str, load: L) -> Result<Worker<SttJob>, SpeechError>
where
    L: FnOnce() -> Result<D, SpeechError> + Send + 'static,
    D: FnMut(&[f32]) -> Result<String, SpeechError>,
{
    Worker::spawn(thread_name, JOB_QUEUE, move || {
        let mut decode = load()?;
        Ok(move |job: SttJob| {
            let text = decode(&job.samples);
            // The engine may have moved on; its receiver is gone then.
            let _ = job.reply.try_send(Reply {
                text,
                samples: job.samples,
            });
        })
    })
}

#[derive(Debug)]
enum State {
    Idle,
    Recording {
        turn: TurnId,
        language: Option<String>,
    },
    Waiting {
        turn: TurnId,
        language: Option<String>,
        replies: Receiver<Reply>,
        cancelled: Arc<AtomicBool>,
    },
    Delivered,
}

/// One utterance at a time, decoded whole on the worker after `finish`.
#[derive(Debug)]
pub struct OfflineStt {
    info: EngineInfo,
    worker: Worker<SttJob>,
    samples: Vec<f32>,
    max_samples: usize,
    state: State,
}

impl OfflineStt {
    /// `max_samples` comes from [`max_utterance_samples`]; audio beyond it is
    /// dropped as overflow.
    pub fn new(info: EngineInfo, worker: Worker<SttJob>, max_samples: usize) -> Self {
        Self {
            info,
            worker,
            samples: Vec::new(),
            max_samples,
            state: State::Idle,
        }
    }
}

impl SttEngine for OfflineStt {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn begin(&mut self, turn_id: TurnId, language: Option<&str>) -> Result<(), SpeechError> {
        // The contract says `begin` implies `cancel` (aulo-speech `stt`):
        // cancel first, so a language this engine rejects still ends any
        // utterance left recording or waiting (T16.3).
        self.cancel();
        if let Some(tag) = language
            && !self.info.capabilities.languages.supports(tag)
        {
            return Err(SpeechError::unsupported(tag));
        }
        // Only allocates when a cancelled decode kept the previous buffer.
        self.samples.reserve_exact(self.max_samples);
        self.state = State::Recording {
            turn: turn_id,
            language: language.map(str::to_owned),
        };
        Ok(())
    }

    fn push(&mut self, frame: AudioFrame<'_>) -> Result<(), SpeechError> {
        if !matches!(self.state, State::Recording { .. }) {
            return Err(SpeechError::OutOfOrder("push outside an utterance"));
        }
        if frame.format() != AudioFormat::PIPELINE {
            return Err(SpeechError::invalid("audio format", "16 kHz mono only"));
        }
        if self.samples.len() + frame.len() > self.max_samples {
            return Err(SpeechError::Overflow {
                dropped: frame.len(),
            });
        }
        self.samples.extend_from_slice(frame.samples());
        Ok(())
    }

    fn finish(&mut self) -> Result<(), SpeechError> {
        let State::Recording { turn, language } = mem::replace(&mut self.state, State::Idle) else {
            return Err(SpeechError::OutOfOrder("finish before begin"));
        };
        // One slot: the worker sends exactly one reply per job.
        let (reply, replies) = sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let samples = mem::take(&mut self.samples);
        if samples.is_empty() {
            // Nothing was said; the contract still wants one final.
            let _ = reply.try_send(Reply {
                text: Ok(String::new()),
                samples,
            });
        } else {
            let job = SttJob {
                samples,
                cancelled: Arc::clone(&cancelled),
                reply,
            };
            self.worker.submit(job).map_err(|error| match error {
                TrySendError::Full(_) => SpeechError::failed("speech worker is busy"),
                TrySendError::Disconnected(_) => SpeechError::unavailable("speech worker stopped"),
            })?;
        }
        self.state = State::Waiting {
            turn,
            language,
            replies,
            cancelled,
        };
        Ok(())
    }

    fn poll(&mut self, out: &mut Transcript) -> Result<SttPoll, SpeechError> {
        let State::Waiting {
            turn,
            language,
            replies,
            ..
        } = &self.state
        else {
            return Ok(match self.state {
                State::Delivered => SttPoll::Done,
                _ => SttPoll::Pending,
            });
        };
        let reply = match replies.try_recv() {
            Ok(reply) => reply,
            Err(TryRecvError::Empty) => return Ok(SttPoll::Pending),
            Err(TryRecvError::Disconnected) => {
                self.state = State::Idle;
                return Err(SpeechError::failed("speech worker stopped"));
            }
        };
        let text = match reply.text {
            Ok(text) => text,
            Err(error) => {
                self.state = State::Idle;
                return Err(error);
            }
        };
        let text = text.trim();
        let text = &text[..text.floor_char_boundary(MAX_TRANSCRIPT_BYTES)];
        out.set(*turn, TranscriptKind::Final, text, language.as_deref());
        self.samples = reply.samples;
        self.samples.clear();
        self.state = State::Delivered;
        Ok(SttPoll::Updated)
    }

    fn cancel(&mut self) {
        if let State::Waiting { cancelled, .. } = &self.state {
            cancelled.store(true, Ordering::Release);
        }
        self.state = State::Idle;
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Instant;

    use aulo_speech::{Capabilities, EngineId, EngineKind, LanguageSupport};

    use super::*;

    const FRAME: usize = 320;
    const MAX_SAMPLES: usize = FRAME * 4;

    fn info() -> EngineInfo {
        EngineInfo {
            id: EngineId::new("test-stt").unwrap(),
            kind: EngineKind::Stt,
            name: "test".into(),
            capabilities: Capabilities {
                streaming: false,
                languages: LanguageSupport::Listed(vec!["en".into(), "ru".into(), "uk".into()]),
                offline: true,
                needs_network: false,
            },
        }
    }

    /// An engine over a fake decoder that "hears" the sample count, and
    /// waits on `gate` first so tests can hold a decode in flight.
    fn engine(gate: Arc<Mutex<()>>, text: Option<&'static str>) -> OfflineStt {
        let worker = decoder("test-stt", move || {
            Ok(move |samples: &[f32]| {
                let _held = gate.lock().unwrap();
                text.map(|t| format!(" {t} {} ", samples.len()))
                    .ok_or_else(|| SpeechError::failed("no result"))
            })
        })
        .unwrap();
        OfflineStt::new(info(), worker, MAX_SAMPLES)
    }

    /// T16.3: `begin` implies `cancel` even when it rejects the language —
    /// an utterance left recording must not keep accepting audio after a
    /// failed `begin`.
    #[test]
    fn a_rejected_language_still_cancels_the_utterance_in_progress() {
        let mut stt = engine(Arc::default(), None);
        stt.begin(TurnId::new(), None).unwrap();
        push(&mut stt, 1).unwrap();
        stt.begin(TurnId::new(), Some("zz"))
            .expect_err("zz is not en/ru/uk");
        assert!(
            matches!(push(&mut stt, 1), Err(SpeechError::OutOfOrder(_))),
            "the old utterance must have been cancelled"
        );
        stt.begin(TurnId::new(), Some("en")).unwrap();
        push(&mut stt, 1).unwrap();
    }

    fn push(stt: &mut OfflineStt, frames: usize) -> Result<(), SpeechError> {
        let samples = [0.1_f32; FRAME];
        for i in 0..frames {
            let at = (i * FRAME) as u64;
            stt.push(AudioFrame::new(&samples, AudioFormat::PIPELINE, at).unwrap())?;
        }
        Ok(())
    }

    fn wait(stt: &mut OfflineStt, out: &mut Transcript) -> Result<SttPoll, SpeechError> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match stt.poll(out)? {
                SttPoll::Pending if Instant::now() < deadline => std::thread::yield_now(),
                other => return Ok(other),
            }
        }
    }

    #[test]
    fn one_trimmed_final_per_segment_and_the_buffer_is_reused() {
        let mut stt = engine(Arc::default(), Some("hello"));
        let turn = TurnId::new();
        let mut out = Transcript::with_capacity(turn, 64);
        stt.begin(turn, Some("ru-RU")).unwrap();
        let buffer = stt.samples.as_ptr();
        push(&mut stt, 2).unwrap();
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Pending);
        stt.finish().unwrap();
        assert_eq!(wait(&mut stt, &mut out).unwrap(), SttPoll::Updated);
        assert_eq!(out.kind, TranscriptKind::Final);
        assert_eq!(out.text, "hello 640");
        assert_eq!(out.language.as_deref(), Some("ru-RU"));
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Done);
        stt.begin(TurnId::new(), None).unwrap();
        assert_eq!(stt.samples.as_ptr(), buffer);
        assert_eq!(stt.samples.capacity(), MAX_SAMPLES);
    }

    #[test]
    fn silence_still_gets_an_empty_final() {
        let mut stt = engine(Arc::default(), Some("never"));
        let mut out = Transcript::with_capacity(TurnId::new(), 8);
        stt.begin(TurnId::new(), None).unwrap();
        stt.finish().unwrap();
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Updated);
        assert_eq!((out.kind, out.text.as_str()), (TranscriptKind::Final, ""));
    }

    #[test]
    fn audio_past_the_cap_overflows_without_growing() {
        let mut stt = engine(Arc::default(), Some("x"));
        stt.begin(TurnId::new(), None).unwrap();
        push(&mut stt, 4).unwrap();
        assert_eq!(
            push(&mut stt, 1).unwrap_err(),
            SpeechError::Overflow { dropped: FRAME }
        );
        assert_eq!(stt.samples.capacity(), MAX_SAMPLES);
    }

    #[test]
    fn calls_out_of_order_and_foreign_input_are_refused() {
        let mut stt = engine(Arc::default(), Some("x"));
        assert!(matches!(push(&mut stt, 1), Err(SpeechError::OutOfOrder(_))));
        assert!(matches!(stt.finish(), Err(SpeechError::OutOfOrder(_))));
        let error = stt.begin(TurnId::new(), Some("ja")).unwrap_err();
        assert!(error.should_fall_back());
        stt.begin(TurnId::new(), Some("uk")).unwrap();
        let stereo = AudioFormat::new(48_000, 2).unwrap();
        let frame = AudioFrame::new(&[0.0; 4], stereo, 0).unwrap();
        assert!(matches!(stt.push(frame), Err(SpeechError::Invalid { .. })));
    }

    #[test]
    fn cancel_returns_at_once_while_a_decode_is_in_flight() {
        let gate = Arc::new(Mutex::new(()));
        let held = gate.lock().unwrap();
        let mut stt = engine(Arc::clone(&gate), Some("late"));
        let mut out = Transcript::with_capacity(TurnId::new(), 8);
        stt.begin(TurnId::new(), None).unwrap();
        push(&mut stt, 1).unwrap();
        stt.finish().unwrap();
        let started = Instant::now();
        stt.cancel();
        assert!(started.elapsed() < Duration::from_millis(150));
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Pending);
        drop(held);
        // The next utterance gets its own reply, never the cancelled one.
        let turn = TurnId::new();
        stt.begin(turn, None).unwrap();
        push(&mut stt, 3).unwrap();
        stt.finish().unwrap();
        assert_eq!(wait(&mut stt, &mut out).unwrap(), SttPoll::Updated);
        assert_eq!((out.turn_id, out.text.as_str()), (turn, "late 960"));
    }

    #[test]
    fn a_decode_error_reaches_the_caller_and_fails_over() {
        let mut stt = engine(Arc::default(), None);
        let mut out = Transcript::with_capacity(TurnId::new(), 8);
        stt.begin(TurnId::new(), None).unwrap();
        push(&mut stt, 1).unwrap();
        stt.finish().unwrap();
        let error = wait(&mut stt, &mut out).unwrap_err();
        assert_eq!(error, SpeechError::failed("no result"));
        assert!(error.should_fall_back());
    }

    #[test]
    fn transcripts_are_capped_on_a_char_boundary() {
        let long: &'static str = "я".repeat(MAX_TRANSCRIPT_BYTES).leak();
        let mut stt = engine(Arc::default(), Some(long));
        let mut out = Transcript::with_capacity(TurnId::new(), 8);
        stt.begin(TurnId::new(), None).unwrap();
        push(&mut stt, 1).unwrap();
        stt.finish().unwrap();
        wait(&mut stt, &mut out).unwrap();
        assert!(out.text.len() <= MAX_TRANSCRIPT_BYTES);
        assert!(out.text.chars().all(|c| c == 'я'));
    }

    #[test]
    fn the_utterance_limit_is_checked() {
        assert_eq!(
            max_utterance_samples(Duration::from_secs(2)).unwrap(),
            32_000
        );
        assert!(max_utterance_samples(Duration::ZERO).is_err());
        assert!(max_utterance_samples(MAX_UTTERANCE_LIMIT + Duration::from_secs(1)).is_err());
    }
}
