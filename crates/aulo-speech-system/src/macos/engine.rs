//! [`SystemTts`]: the push/poll engine the pipeline drives. It never touches
//! AVFoundation objects itself: text goes to the worker over a bounded
//! channel, audio comes back through a fixed-size lock-free ring, so every
//! method returns at once.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use aulo_speech::{
    AudioFormat, Capabilities, EngineId, EngineInfo, EngineKind, EngineSpec, LanguageSupport,
    SpeechError, SpeechRate, TtsEngine, TtsPoll, TtsRequest, Voice,
};
use objc2_avf_audio::AVSpeechSynthesisVoice;
use ringbuf::traits::{Consumer, Split};
use ringbuf::{HeapCons, HeapRb};

use super::voices::{self, FALLBACK_RATE_HZ, VoiceMeta};
use super::worker::{self, Command, Shared, Utterance};

/// The id config names (`engine = "system"`). Each platform module registers
/// its own system voices under it, so one config works on every desktop.
pub const ENGINE_ID: &str = "system";
const ENGINE_NAME: &str = "macOS system voices";
const WORKER_NAME: &str = "aulo-avspeech";

/// About 47 s of 22.05 kHz mono (4 MiB), allocated once per engine: longer
/// than one sentence at the slowest rate, so a reply only overflows if the
/// pipeline stops polling.
const RING_CAPACITY_SAMPLES: usize = 1 << 20;
/// The next sentence starts only with this much room, about 23 s at 22.05 kHz.
const DISPATCH_MIN_VACANT: usize = RING_CAPACITY_SAMPLES / 2;
/// Sentences queued but not yet fully synthesized. The normalizer sends one
/// sentence per push, so this is several replies' worth.
const MAX_QUEUED_TEXTS: u64 = 64;
/// One push is one sentence (240 chars from the normalizer); anything far
/// longer is not from it and is refused.
const MAX_TEXT_BYTES: usize = 4 * 1024;
/// With no callback at all by then, the main run loop is not running.
const MAIN_LOOP_TIMEOUT: Duration = Duration::from_secs(5);
const MONO: u8 = 1;

/// The registry factory: `registry.register(EngineId::new(ENGINE_ID)?, factory)`.
/// `spec.voice` becomes the default voice; model and rate are per reply.
pub fn factory(spec: &EngineSpec) -> Result<Box<dyn TtsEngine>, SpeechError> {
    Ok(Box::new(SystemTts::new(spec.voice.clone())?))
}

/// Text-to-speech over the voices installed in macOS. Output is mono at the
/// chosen voice's own rate (16 or 22.05 kHz).
pub struct SystemTts {
    info: EngineInfo,
    voices: Vec<Voice>,
    meta: Vec<VoiceMeta>,
    default_voice: Option<String>,
    shared: Arc<Shared>,
    consumer: HeapCons<f32>,
    commands: SyncSender<Command>,
    reply: Option<Reply>,
    next_utterance: u64,
}

struct Reply {
    generation: u64,
    voice: String,
    rate: SpeechRate,
    pushed: u64,
    finished: bool,
    first_push: Option<Instant>,
}

impl SystemTts {
    /// Lists the voices and starts the worker. `default_voice` is a
    /// [`Voice::id`] used when a request names none; an unknown one makes
    /// `begin` report it unsupported, so the registry falls back.
    pub fn new(default_voice: Option<String>) -> Result<Self, SpeechError> {
        let (voices, meta) = voices::installed();
        if voices.is_empty() {
            return Err(SpeechError::unavailable("no system voices are installed"));
        }
        let (producer, consumer) = HeapRb::<f32>::new(RING_CAPACITY_SAMPLES).split();
        let shared = Arc::new(Shared::new(producer, FALLBACK_RATE_HZ));
        // +1 so a Stop still fits behind a full queue of sentences.
        let (commands, receiver) = mpsc::sync_channel(MAX_QUEUED_TEXTS as usize + 1);
        let (worker_shared, ring) = (Arc::clone(&shared), consumer.observe());
        thread::Builder::new()
            .name(WORKER_NAME.into())
            .spawn(move || worker::run(worker_shared, ring, receiver, DISPATCH_MIN_VACANT))
            .map_err(|e| SpeechError::unavailable(&format!("worker thread: {e}")))?;
        Ok(Self {
            info: info(&voices)?,
            voices,
            meta,
            default_voice,
            shared,
            consumer,
            commands,
            reply: None,
            next_utterance: 0,
        })
    }

    /// Samples lost to a full ring since the engine loaded; the pipeline
    /// reports it with its other overflow counters.
    pub fn dropped_samples(&self) -> u64 {
        self.shared.dropped.load(Ordering::Relaxed)
    }

    fn pick_voice(&self, request: &TtsRequest<'_>) -> Result<usize, SpeechError> {
        if let Some(id) = request.voice.or(self.default_voice.as_deref()) {
            let found = self.voices.iter().position(|v| v.id == id);
            return found.ok_or_else(|| SpeechError::unsupported(id));
        }
        let language = match request.language {
            Some(tag) => tag.to_owned(),
            // SAFETY: a class method with no arguments returning an owned string.
            None => unsafe { AVSpeechSynthesisVoice::currentLanguageCode() }.to_string(),
        };
        voices::choose(&self.voices, &self.meta, &language)
            .ok_or_else(|| SpeechError::unsupported(&language))
    }
}

fn info(voices: &[Voice]) -> Result<EngineInfo, SpeechError> {
    let languages: BTreeSet<String> = voices.iter().filter_map(|v| v.language.clone()).collect();
    Ok(EngineInfo {
        id: EngineId::new(ENGINE_ID)?,
        kind: EngineKind::Tts,
        name: ENGINE_NAME.into(),
        capabilities: Capabilities {
            streaming: true,
            languages: LanguageSupport::Listed(languages.into_iter().collect()),
            offline: true,
            needs_network: false,
        },
    })
}

impl TtsEngine for SystemTts {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn voices(&self) -> &[Voice] {
        &self.voices
    }

    fn begin(&mut self, request: &TtsRequest<'_>) -> Result<AudioFormat, SpeechError> {
        self.cancel();
        let index = self.pick_voice(request)?;
        let output_hz = self.meta[index].sample_rate_hz;
        let format = AudioFormat::new(output_hz, MONO)?;
        self.shared.lock().output_hz = output_hz;
        let generation = self.shared.generation.load(Ordering::Acquire);
        self.reply = Some(Reply {
            generation,
            voice: self.voices[index].id.clone(),
            rate: request.rate,
            pushed: 0,
            finished: false,
            first_push: None,
        });
        Ok(format)
    }

    fn push_text(&mut self, text: &str) -> Result<(), SpeechError> {
        let Some(reply) = self.reply.as_mut().filter(|r| !r.finished) else {
            return Err(SpeechError::OutOfOrder("text outside a reply"));
        };
        if text.len() > MAX_TEXT_BYTES {
            return Err(SpeechError::invalid("tts text", "longer than 4096 bytes"));
        }
        // AVFoundation sends no end marker for empty text, so the reply
        // would never finish.
        if text.trim().is_empty() {
            return Ok(());
        }
        let ended = self.shared.ended.load(Ordering::Acquire);
        if reply.pushed.saturating_sub(ended) >= MAX_QUEUED_TEXTS {
            return Err(SpeechError::Overflow { dropped: 0 });
        }
        self.next_utterance += 1;
        let utterance = Utterance {
            generation: reply.generation,
            id: self.next_utterance,
            text: text.to_owned(),
            voice: reply.voice.clone(),
            rate: reply.rate,
        };
        self.commands
            .try_send(Command::Speak(utterance))
            .map_err(|e| match e {
                TrySendError::Full(_) => SpeechError::Overflow { dropped: 0 },
                TrySendError::Disconnected(_) => SpeechError::failed("speech worker stopped"),
            })?;
        reply.pushed += 1;
        reply.first_push.get_or_insert_with(Instant::now);
        Ok(())
    }

    fn finish(&mut self) -> Result<(), SpeechError> {
        let order = SpeechError::OutOfOrder("finish before begin");
        self.reply.as_mut().ok_or(order)?.finished = true;
        Ok(())
    }

    fn poll(&mut self, out: &mut [f32]) -> Result<TtsPoll, SpeechError> {
        let Some(reply) = &self.reply else {
            return Ok(TtsPoll::Done);
        };
        // Read before popping: samples are stored before their end marker,
        // so an empty ring after this load really is the end.
        let ended = self.shared.ended.load(Ordering::Acquire);
        let samples = self.consumer.pop_slice(out);
        if samples > 0 {
            return Ok(TtsPoll::Audio { samples });
        }
        if reply.finished && ended >= reply.pushed {
            self.reply = None;
            return Ok(TtsPoll::Done);
        }
        let stalled = reply
            .first_push
            .is_some_and(|at| at.elapsed() > MAIN_LOOP_TIMEOUT);
        if stalled && !self.shared.heard.load(Ordering::Relaxed) {
            self.cancel();
            return Err(SpeechError::unavailable(
                "AVSpeechSynthesizer delivers audio on the main run loop, which is not running",
            ));
        }
        Ok(TtsPoll::Pending)
    }

    fn cancel(&mut self) {
        {
            // Under the lock, so no callback of the old generation can be
            // halfway through a write when the ring is cleared below.
            let _writer = self.shared.lock();
            self.shared.generation.fetch_add(1, Ordering::AcqRel);
            self.shared.ended.store(0, Ordering::Release);
        }
        self.consumer.clear();
        if self.reply.take().is_some() {
            // Best effort: a full queue is dropped by generation anyway.
            let _ = self.commands.try_send(Command::Stop);
        }
    }
}

impl std::fmt::Debug for SystemTts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let replying = self.reply.is_some();
        let mut out = f.debug_struct("SystemTts");
        out.field("replying", &replying).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use aulo_speech::{SpeechRate, TurnId};
    use aulo_voice::TtsRegistry;

    use super::*;

    fn request<'a>(voice: Option<&'a str>, language: Option<&'a str>) -> TtsRequest<'a> {
        TtsRequest {
            turn_id: TurnId::new(),
            voice,
            language,
            rate: SpeechRate::NORMAL,
        }
    }

    #[test]
    fn registry_builds_the_engine_from_its_factory() {
        let mut registry = TtsRegistry::new();
        let id = EngineId::new(ENGINE_ID).unwrap();
        registry.register(id.clone(), factory).unwrap();
        let spec = EngineSpec {
            engine: id,
            model: None,
            voice: None,
            rate: SpeechRate::NORMAL,
        };
        let engine = registry.build(&spec).unwrap();
        assert!(!engine.voices().is_empty());
        assert_eq!(engine.info().kind, EngineKind::Tts);
    }

    #[test]
    fn unknown_voices_are_unsupported_so_the_registry_falls_back() {
        let mut engine = SystemTts::new(Some("no.such.voice".into())).unwrap();
        let error = engine.begin(&request(None, Some("en"))).unwrap_err();
        assert!(error.should_fall_back());
        let mut engine = SystemTts::new(None).unwrap();
        let error = engine
            .begin(&request(Some("no.such.voice"), None))
            .unwrap_err();
        assert!(error.should_fall_back());
        let error = engine.begin(&request(None, Some("tlh"))).unwrap_err();
        assert!(error.should_fall_back());
    }

    #[test]
    fn declared_format_is_mono_at_the_voice_rate() {
        let mut engine = SystemTts::new(None).unwrap();
        let format = engine.begin(&request(None, Some("en-US"))).unwrap();
        assert_eq!(format.channels(), 1);
        assert!([16_000, 22_050, 24_000, 48_000].contains(&format.sample_rate_hz()));
    }

    /// libtest parks the main thread, so this is the host that forgot to run
    /// the main run loop: the engine must give up and let the registry fall back.
    #[test]
    fn a_host_without_a_main_run_loop_gets_unavailable() {
        let mut engine = SystemTts::new(None).unwrap();
        engine.begin(&request(None, Some("en"))).unwrap();
        engine.push_text("Nobody will hear this.").unwrap();
        engine.finish().unwrap();
        let started = Instant::now();
        let error = loop {
            match engine.poll(&mut [0.0; 64]) {
                Ok(TtsPoll::Pending) => thread::sleep(Duration::from_millis(20)),
                Ok(other) => panic!("got {other:?} without a main run loop"),
                Err(error) => break error,
            }
            assert!(started.elapsed() < MAIN_LOOP_TIMEOUT * 2);
        };
        assert!(matches!(error, SpeechError::Unavailable(_)));
        assert_eq!(engine.poll(&mut [0.0; 64]).unwrap(), TtsPoll::Done);
    }

    #[test]
    fn call_order_and_text_caps_are_enforced() {
        let mut engine = SystemTts::new(None).unwrap();
        assert!(matches!(
            engine.push_text("hi"),
            Err(SpeechError::OutOfOrder(_))
        ));
        assert_eq!(engine.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
        engine.begin(&request(None, Some("en"))).unwrap();
        let long = "a".repeat(MAX_TEXT_BYTES + 1);
        assert!(matches!(
            engine.push_text(&long),
            Err(SpeechError::Invalid { .. })
        ));
        engine.push_text("   ").unwrap();
        engine.finish().unwrap();
        assert!(matches!(
            engine.push_text("late"),
            Err(SpeechError::OutOfOrder(_))
        ));
        // Nothing was queued, so the reply is over at once.
        assert_eq!(engine.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
    }
}
