//! [`SystemStt`]: the push/poll engine the pipeline drives. It never touches
//! Speech framework objects itself: audio goes to the worker through a
//! fixed-size lock-free ring, commands over a small bounded channel, and
//! results come back over per-utterance channels, so every method returns at
//! once and results of a cancelled utterance have nowhere to go.

use std::mem;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use aulo_speech::{
    AudioFormat, AudioFrame, Capabilities, EngineId, EngineInfo, EngineKind, EngineSpec,
    LanguageSupport, SpeechError, SttEngine, SttPoll, Transcript, TranscriptKind, TurnId,
};
use ringbuf::traits::{Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

use super::engine::ENGINE_ID;
use super::recognizer;
use crate::language::best_match;

/// Longest transcript handed to the pipeline, in bytes. Recognizer output
/// is untrusted text that ends up in a prompt; a minute of speech is about a
/// kilobyte.
pub const MAX_TRANSCRIPT_BYTES: usize = 16 * 1024;

const ENGINE_NAME: &str = "macOS on-device speech recognition";
const WORKER_NAME: &str = "aulo-sfspeech";
/// 10 s at the pipeline rate (640 KiB), allocated once. The worker drains it
/// every 10 ms, so it only fills if the worker stalls.
const RING_CAPACITY_SAMPLES: usize = 160_000;
/// Begin, finish and cancel per utterance; this is many utterances' worth.
const MAX_QUEUED_COMMANDS: usize = 16;
/// Partials not yet polled. Each supersedes the last, so dropping is fine.
const MAX_QUEUED_PARTIALS: usize = 8;
/// On-device recognition finalizes well within this after the audio ends.
const FINAL_TIMEOUT: Duration = Duration::from_secs(10);

/// What a recognition task ends with: the final text or why it failed.
pub(super) type Outcome = Result<String, SpeechError>;

/// Where one utterance's results go. Created by `begin` and dropped by the
/// engine on cancel, which is what discards a stale task's late results.
pub(super) struct Sink {
    pub partials: SyncSender<String>,
    pub outcome: SyncSender<Outcome>,
}

/// Positions count samples accepted by `push` since the engine started.
pub(super) enum Command {
    /// Locale, the position the utterance starts at, where results go.
    Begin(String, u64, Sink),
    /// No more audio after this position.
    Finish(u64),
    /// Drop audio up to this position.
    Cancel(u64),
}

/// The registry factory: `stt.register(EngineId::new(ENGINE_ID)?, stt_factory)`.
/// Unavailable unless access was granted and an on-device model exists.
pub fn stt_factory(_spec: &EngineSpec) -> Result<Box<dyn SttEngine>, SpeechError> {
    Ok(Box::new(SystemStt::new()?))
}

struct Session {
    turn: TurnId,
    locale: usize,
    partials: Receiver<String>,
    outcome: Receiver<Outcome>,
    pushed: u64,
    finished_at: Option<Instant>,
    /// A final that arrived before `finish`, held so that it is delivered
    /// after it as the contract requires.
    held: Option<String>,
}

enum State {
    Idle,
    Live(Session),
    Delivered,
}

/// Speech-to-text over `SFSpeechRecognizer`, forced on-device. Streaming:
/// partials arrive while audio is pushed. Takes 16 kHz mono only.
pub struct SystemStt {
    info: EngineInfo,
    locales: Vec<String>,
    default_locale: Option<usize>,
    producer: HeapProd<f32>,
    commands: SyncSender<Command>,
    pushed: u64,
    dropped: u64,
    state: State,
}

impl SystemStt {
    /// Probes access and the on-device locales (about 0.3 s), then starts the
    /// worker. Never asks for access.
    pub fn new() -> Result<Self, SpeechError> {
        let locales = recognizer::on_device_locales()?;
        let default = recognizer::current_tag();
        Self::start(locales, &default, |ring, commands| {
            recognizer::run(ring, commands)
        })
    }

    fn start(
        locales: Vec<String>,
        default: &str,
        worker: impl FnOnce(HeapCons<f32>, Receiver<Command>) + Send + 'static,
    ) -> Result<Self, SpeechError> {
        let (producer, consumer) = HeapRb::<f32>::new(RING_CAPACITY_SAMPLES).split();
        let (commands, receiver) = mpsc::sync_channel(MAX_QUEUED_COMMANDS);
        thread::Builder::new()
            .name(WORKER_NAME.into())
            .spawn(move || worker(consumer, receiver))
            .map_err(|e| SpeechError::unavailable(&format!("worker thread: {e}")))?;
        Ok(Self {
            info: EngineInfo {
                id: EngineId::new(ENGINE_ID)?,
                kind: EngineKind::Stt,
                name: ENGINE_NAME.into(),
                capabilities: Capabilities {
                    streaming: true,
                    languages: LanguageSupport::Listed(locales.clone()),
                    offline: true,
                    needs_network: false,
                },
            },
            default_locale: pick(&locales, default),
            locales,
            producer,
            commands,
            pushed: 0,
            dropped: 0,
            state: State::Idle,
        })
    }

    /// Samples lost to a full ring since the engine loaded; the pipeline
    /// reports it with its other overflow counters.
    pub fn dropped_samples(&self) -> u64 {
        self.dropped
    }

    fn send(&self, command: Command) -> Result<(), SpeechError> {
        self.commands.try_send(command).map_err(|e| match e {
            TrySendError::Full(_) => SpeechError::failed("speech worker is not keeping up"),
            TrySendError::Disconnected(_) => SpeechError::failed("speech worker stopped"),
        })
    }

    fn deliver(&mut self, out: &mut Transcript, kind: TranscriptKind, text: &str) -> SttPoll {
        if let State::Live(session) = &self.state {
            let language = Some(self.locales[session.locale].as_str());
            out.set(session.turn, kind, text, language);
        }
        if kind == TranscriptKind::Final {
            self.state = State::Delivered;
        }
        SttPoll::Updated
    }
}

fn pick(locales: &[String], language: &str) -> Option<usize> {
    best_match(locales.iter().map(|tag| (Some(tag.as_str()), 0)), language)
}

impl SttEngine for SystemStt {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn begin(&mut self, turn_id: TurnId, language: Option<&str>) -> Result<(), SpeechError> {
        self.cancel();
        let locale = match language {
            Some(tag) => pick(&self.locales, tag).ok_or_else(|| SpeechError::unsupported(tag))?,
            None => self
                .default_locale
                .ok_or_else(|| SpeechError::unsupported("the system language"))?,
        };
        let (partials, partials_rx) = mpsc::sync_channel(MAX_QUEUED_PARTIALS);
        // One slot: a task ends with exactly one result or error.
        let (outcome, outcome_rx) = mpsc::sync_channel(1);
        let sink = Sink { partials, outcome };
        self.send(Command::Begin(
            self.locales[locale].clone(),
            self.pushed,
            sink,
        ))?;
        self.state = State::Live(Session {
            turn: turn_id,
            locale,
            partials: partials_rx,
            outcome: outcome_rx,
            pushed: 0,
            finished_at: None,
            held: None,
        });
        Ok(())
    }

    fn push(&mut self, frame: AudioFrame<'_>) -> Result<(), SpeechError> {
        let State::Live(session) = &mut self.state else {
            return Err(SpeechError::OutOfOrder("push outside an utterance"));
        };
        if session.finished_at.is_some() {
            return Err(SpeechError::OutOfOrder("push after finish"));
        }
        if frame.format() != AudioFormat::PIPELINE {
            return Err(SpeechError::invalid("audio format", "16 kHz mono only"));
        }
        let samples = frame.samples();
        if self.producer.vacant_len() < samples.len() {
            self.dropped += samples.len() as u64;
            return Err(SpeechError::Overflow {
                dropped: frame.len(),
            });
        }
        self.producer.push_slice(samples);
        self.pushed += samples.len() as u64;
        session.pushed += samples.len() as u64;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), SpeechError> {
        let State::Live(session) = &mut self.state else {
            return Err(SpeechError::OutOfOrder("finish before begin"));
        };
        if session.finished_at.is_some() {
            return Err(SpeechError::OutOfOrder("finish twice"));
        }
        session.finished_at = Some(Instant::now());
        if session.pushed == 0 {
            // Nothing was said; the contract still wants one final, and the
            // recognizer would only report an error for empty audio.
            session.held.get_or_insert_with(String::new);
            let _ = self.commands.try_send(Command::Cancel(self.pushed));
            return Ok(());
        }
        self.send(Command::Finish(self.pushed))
    }

    fn poll(&mut self, out: &mut Transcript) -> Result<SttPoll, SpeechError> {
        let session = match &mut self.state {
            State::Idle => return Ok(SttPoll::Pending),
            State::Delivered => return Ok(SttPoll::Done),
            State::Live(session) => session,
        };
        let finished = session.finished_at;
        if finished.is_some()
            && let Some(text) = session.held.take()
        {
            return Ok(self.deliver(out, TranscriptKind::Final, &text));
        }
        match session.outcome.try_recv() {
            Ok(Ok(text)) if finished.is_some() => {
                return Ok(self.deliver(out, TranscriptKind::Final, &text));
            }
            Ok(Ok(text)) => {
                let poll = self.deliver(out, TranscriptKind::Partial, &text);
                if let State::Live(session) = &mut self.state {
                    // The outcome outranks anything still queued as a
                    // partial: drop them, or the next poll would deliver an
                    // older partial over this held text (T16.4).
                    while session.partials.try_recv().is_ok() {}
                    session.held = Some(text);
                }
                return Ok(poll);
            }
            Ok(Err(error)) => {
                self.cancel();
                return Err(error);
            }
            Err(TryRecvError::Disconnected) if session.held.is_none() => {
                self.cancel();
                return Err(SpeechError::failed("recognition ended without a result"));
            }
            Err(_) => {}
        }
        let mut newest = None;
        while let Ok(text) = session.partials.try_recv() {
            newest = Some(text);
        }
        if let Some(text) = newest {
            return Ok(self.deliver(out, TranscriptKind::Partial, &text));
        }
        if finished.is_some_and(|at| at.elapsed() > FINAL_TIMEOUT) {
            self.cancel();
            return Err(SpeechError::failed("no final transcript within 10 s"));
        }
        Ok(SttPoll::Pending)
    }

    fn cancel(&mut self) {
        if let State::Live(_) = mem::replace(&mut self.state, State::Idle) {
            // Best effort: the next begin carries its own fence anyway.
            let _ = self.commands.try_send(Command::Cancel(self.pushed));
        }
    }
}

impl std::fmt::Debug for SystemStt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let live = matches!(self.state, State::Live(_));
        f.debug_struct("SystemStt")
            .field("live", &live)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use aulo_speech::SpeechRate;
    use aulo_voice::SttRegistry;
    use ringbuf::traits::Consumer;

    use super::*;

    const FRAME: usize = 320;
    const LOCALES: [&str; 3] = ["en-GB", "en-US", "ru-RU"];

    /// What the fake worker was told, in order, as plain values.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Seen {
        Begin(String, u64),
        Finish(u64),
        Cancel(u64),
    }

    type Log = Arc<Mutex<Vec<Seen>>>;
    type Sinks = Arc<Mutex<Vec<Sink>>>;

    /// An engine over a fake worker that logs commands and hands every sink
    /// to the test, which plays the recognizer.
    fn engine() -> (SystemStt, Log, Sinks) {
        let (log, sinks) = (Log::default(), Sinks::default());
        let (worker_log, worker_sinks) = (Arc::clone(&log), Arc::clone(&sinks));
        let locales = LOCALES.iter().map(|&t| t.to_owned()).collect();
        let stt = SystemStt::start(locales, "en-US", move |mut ring, commands| {
            while let Ok(command) = commands.recv() {
                let seen = match command {
                    Command::Begin(locale, start_at, sink) => {
                        worker_sinks.lock().unwrap().push(sink);
                        Seen::Begin(locale, start_at)
                    }
                    Command::Finish(end_at) => Seen::Finish(end_at),
                    Command::Cancel(discard_to) => Seen::Cancel(discard_to),
                };
                ring.clear();
                worker_log.lock().unwrap().push(seen);
            }
        })
        .unwrap();
        (stt, log, sinks)
    }

    fn push(stt: &mut SystemStt, frames: usize) -> Result<(), SpeechError> {
        let samples = [0.1_f32; FRAME];
        for _ in 0..frames {
            stt.push(AudioFrame::new(&samples, AudioFormat::PIPELINE, 0).unwrap())?;
        }
        Ok(())
    }

    /// Waits for the fake worker to have seen `count` commands.
    fn seen(log: &Log, count: usize) -> Vec<Seen> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while log.lock().unwrap().len() < count && Instant::now() < deadline {
            thread::yield_now();
        }
        log.lock().unwrap().clone()
    }

    fn sink(sinks: &Sinks, index: usize) -> (SyncSender<String>, SyncSender<Outcome>) {
        let sinks = sinks.lock().unwrap();
        (sinks[index].partials.clone(), sinks[index].outcome.clone())
    }

    #[test]
    fn partials_stream_and_the_final_comes_after_finish() {
        let (mut stt, log, sinks) = engine();
        let turn = TurnId::new();
        let mut out = Transcript::with_capacity(turn, 64);
        stt.begin(turn, Some("en")).unwrap();
        push(&mut stt, 2).unwrap();
        seen(&log, 1);
        let (partials, outcome) = sink(&sinks, 0);
        partials.try_send("open".into()).unwrap();
        partials.try_send("open the".into()).unwrap();
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Updated);
        assert_eq!(
            (out.kind, out.text.as_str()),
            (TranscriptKind::Partial, "open the")
        );
        assert_eq!(out.language.as_deref(), Some("en-GB"));
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Pending);
        stt.finish().unwrap();
        outcome.try_send(Ok("Open the door.".into())).unwrap();
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Updated);
        assert_eq!(
            (out.kind, out.text.as_str()),
            (TranscriptKind::Final, "Open the door.")
        );
        assert_eq!(out.turn_id, turn);
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Done);
        let fence = (2 * FRAME) as u64;
        assert_eq!(
            seen(&log, 2),
            [Seen::Begin("en-GB".into(), 0), Seen::Finish(fence)]
        );
    }

    /// T16.4: an outcome held before `finish` outranks the partials still
    /// queued behind it — they must be dropped, or the next poll would
    /// deliver an older partial over the held text.
    #[test]
    fn a_held_final_drops_the_partials_queued_behind_it() {
        let (mut stt, log, sinks) = engine();
        let turn = TurnId::new();
        let mut out = Transcript::with_capacity(turn, 64);
        stt.begin(turn, Some("en")).unwrap();
        push(&mut stt, 1).unwrap();
        seen(&log, 1);
        let (partials, outcome) = sink(&sinks, 0);
        // The recognizer's last partial never made it out before the final.
        partials.try_send("open".into()).unwrap();
        outcome.try_send(Ok("Open the door.".into())).unwrap();
        drop(outcome);
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Updated);
        assert_eq!(
            (out.kind, out.text.as_str()),
            (TranscriptKind::Partial, "Open the door.")
        );
        assert_eq!(
            stt.poll(&mut out).unwrap(),
            SttPoll::Pending,
            "the older queued partial must not overwrite the held outcome"
        );
        stt.finish().unwrap();
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Updated);
        assert_eq!(
            (out.kind, out.text.as_str()),
            (TranscriptKind::Final, "Open the door.")
        );
    }

    #[test]
    fn a_final_before_finish_is_held_until_finish() {
        let (mut stt, log, sinks) = engine();
        let mut out = Transcript::with_capacity(TurnId::new(), 64);
        stt.begin(TurnId::new(), Some("ru")).unwrap();
        push(&mut stt, 1).unwrap();
        seen(&log, 1);
        let (_, outcome) = sink(&sinks, 0);
        outcome.try_send(Ok("привет".into())).unwrap();
        drop(outcome);
        sinks.lock().unwrap().clear();
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Updated);
        assert_eq!(out.kind, TranscriptKind::Partial);
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Pending);
        stt.finish().unwrap();
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Updated);
        assert_eq!(
            (out.kind, out.text.as_str()),
            (TranscriptKind::Final, "привет")
        );
    }

    #[test]
    fn silence_gets_an_empty_final_without_the_recognizer() {
        let (mut stt, log, _) = engine();
        let mut out = Transcript::with_capacity(TurnId::new(), 8);
        stt.begin(TurnId::new(), None).unwrap();
        stt.finish().unwrap();
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Updated);
        assert_eq!((out.kind, out.text.as_str()), (TranscriptKind::Final, ""));
        assert_eq!(out.language.as_deref(), Some("en-US"));
        assert_eq!(seen(&log, 2)[1], Seen::Cancel(0));
    }

    #[test]
    fn cancel_is_immediate_and_fences_off_the_old_utterance() {
        let (mut stt, log, sinks) = engine();
        let mut out = Transcript::with_capacity(TurnId::new(), 8);
        stt.begin(TurnId::new(), None).unwrap();
        push(&mut stt, 3).unwrap();
        let started = Instant::now();
        stt.cancel();
        assert!(started.elapsed() < Duration::from_millis(150));
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Pending);
        let turn = TurnId::new();
        stt.begin(turn, None).unwrap();
        let fence = (3 * FRAME) as u64;
        let log = seen(&log, 3);
        assert_eq!(
            log[1..],
            [Seen::Cancel(fence), Seen::Begin("en-US".into(), fence)]
        );
        // The cancelled task's late result has nowhere to go.
        let (_, stale) = sink(&sinks, 0);
        assert!(stale.try_send(Ok("stale".into())).is_err());
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Pending);
    }

    #[test]
    fn recognizer_errors_fail_over_and_end_the_utterance() {
        let (mut stt, log, sinks) = engine();
        let mut out = Transcript::with_capacity(TurnId::new(), 8);
        stt.begin(TurnId::new(), None).unwrap();
        seen(&log, 1);
        let (_, outcome) = sink(&sinks, 0);
        outcome
            .try_send(Err(SpeechError::unavailable("denied")))
            .unwrap();
        let error = stt.poll(&mut out).unwrap_err();
        assert!(matches!(error, SpeechError::Unavailable(_)));
        assert!(matches!(push(&mut stt, 1), Err(SpeechError::OutOfOrder(_))));
        // A task that vanishes without a word fails over too.
        stt.begin(TurnId::new(), None).unwrap();
        seen(&log, 3);
        sinks.lock().unwrap().clear();
        assert!(stt.poll(&mut out).unwrap_err().should_fall_back());
    }

    #[test]
    fn a_full_ring_counts_the_overflow_instead_of_growing() {
        let (mut stt, _, _) = engine();
        // Hold the ring full: the fake worker only clears it on a command.
        stt.begin(TurnId::new(), None).unwrap();
        thread::sleep(Duration::from_millis(50));
        let fits = RING_CAPACITY_SAMPLES / FRAME;
        push(&mut stt, fits).unwrap();
        assert_eq!(
            push(&mut stt, 1).unwrap_err(),
            SpeechError::Overflow { dropped: FRAME }
        );
        assert_eq!(stt.dropped_samples(), FRAME as u64);
    }

    #[test]
    fn languages_without_an_on_device_model_and_foreign_audio_are_refused() {
        let (mut stt, _, _) = engine();
        assert!(matches!(stt.finish(), Err(SpeechError::OutOfOrder(_))));
        assert!(
            stt.begin(TurnId::new(), Some("ja-JP"))
                .unwrap_err()
                .should_fall_back()
        );
        stt.begin(TurnId::new(), Some("en-us")).unwrap();
        let stereo = AudioFormat::new(48_000, 2).unwrap();
        let frame = AudioFrame::new(&[0.0; 4], stereo, 0).unwrap();
        assert!(matches!(stt.push(frame), Err(SpeechError::Invalid { .. })));
        stt.finish().unwrap();
        assert!(matches!(stt.finish(), Err(SpeechError::OutOfOrder(_))));
        assert!(matches!(push(&mut stt, 1), Err(SpeechError::OutOfOrder(_))));
    }

    #[test]
    fn a_system_language_without_a_model_is_unsupported() {
        let stt = SystemStt::start(vec!["ru-RU".into()], "fr-FR", |_, _| {}).unwrap();
        let mut stt = stt;
        assert!(matches!(
            stt.begin(TurnId::new(), None),
            Err(SpeechError::Unsupported(_))
        ));
        assert_eq!(
            stt.info().capabilities.languages,
            LanguageSupport::Listed(vec!["ru-RU".into()])
        );
    }

    /// Passes with or without access: the factory either builds a streaming
    /// offline engine or reports Unavailable, so the registry falls back.
    #[test]
    fn registry_builds_the_engine_or_falls_back() {
        let mut registry = SttRegistry::new();
        let id = EngineId::new(ENGINE_ID).unwrap();
        registry.register(id.clone(), stt_factory).unwrap();
        let spec = EngineSpec {
            engine: id,
            model: None,
            voice: None,
            rate: SpeechRate::NORMAL,
        };
        match registry.build(&spec) {
            Ok(engine) => {
                let capabilities = &engine.info().capabilities;
                assert!(
                    capabilities.streaming && capabilities.offline && !capabilities.needs_network
                );
            }
            Err(error) => assert!(matches!(error, SpeechError::Unavailable(_)), "{error}"),
        }
    }
}
