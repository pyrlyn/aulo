//! [`EspeakTts`]: the push/poll engine the pipeline drives. Text goes to the
//! worker over a bounded channel and audio comes back through a fixed-size
//! lock-free ring, so every method returns at once.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread;
use std::time::Duration;

use aulo_speech::{
    AudioFormat, Capabilities, EngineId, EngineInfo, EngineKind, EngineSpec, LanguageSupport,
    SpeechError, TtsEngine, TtsPoll, TtsRequest, Voice,
};
use ringbuf::traits::{Consumer, Split};
use ringbuf::{HeapCons, HeapRb};

use super::process;
use super::voices;
use super::worker::{Config, STALL_LIMIT, Shared, Utterance, run};

/// The id config names (`engine = "system"`), shared with the other
/// platforms' system voices so one config works on every desktop.
pub const ENGINE_ID: &str = "system";
const ENGINE_NAME: &str = "espeak-ng";
/// Resolved from `PATH` when no path is configured.
const DEFAULT_BINARY: &str = "espeak-ng";
const WORKER_NAME: &str = "aulo-espeak";
/// About 12 s of 22.05 kHz mono (1 MiB), allocated once per engine. The
/// worker waits when it is full, so this only bounds the lead over playback.
const RING_CAPACITY_SAMPLES: usize = 1 << 18;
/// Sentences queued but not yet spoken; the normalizer sends one per push.
const MAX_QUEUED_TEXTS: u64 = 64;
/// One push is one sentence (240 chars from the normalizer); anything far
/// longer is not from it and is refused.
const MAX_TEXT_BYTES: usize = 4 * 1024;
const MONO: u8 = 1;

/// The registry factory with `espeak-ng` taken from `PATH`:
/// `registry.register(EngineId::new(ENGINE_ID)?, factory)`.
pub fn factory(spec: &EngineSpec) -> Result<Box<dyn TtsEngine>, SpeechError> {
    Ok(Box::new(EspeakTts::new(
        DEFAULT_BINARY,
        spec.voice.clone(),
    )?))
}

/// A factory for an `espeak-ng` at a configured path. The path comes from the
/// host's own settings, never from an [`EngineSpec`]: bot and chat overrides
/// can come from remote clients, and a path in them would run their program.
pub fn factory_with(
    binary: PathBuf,
) -> impl Fn(&EngineSpec) -> Result<Box<dyn TtsEngine>, SpeechError> + Send + Sync + 'static {
    move |spec| {
        Ok(Box::new(EspeakTts::new(
            binary.clone(),
            spec.voice.clone(),
        )?))
    }
}

/// Text-to-speech through an `espeak-ng` process. Output is mono 16-bit at the
/// rate of the installed espeak-ng, read from its WAV header.
pub struct EspeakTts {
    info: EngineInfo,
    voices: Vec<Voice>,
    default_voice: Option<String>,
    format: AudioFormat,
    shared: Arc<Shared>,
    consumer: HeapCons<f32>,
    utterances: SyncSender<Utterance>,
    reply: Option<Reply>,
}

struct Reply {
    generation: u64,
    voice: Option<String>,
    words_per_minute: u32,
    pushed: u64,
    finished: bool,
}

impl EspeakTts {
    /// Lists the voices and learns the output rate by running `binary`; a
    /// binary that is missing or broken is [`SpeechError::Unavailable`] or
    /// [`SpeechError::Failed`] so the registry falls back. `default_voice` is a
    /// [`Voice::id`] used when a request names none; an unknown one makes
    /// `begin` report it unsupported.
    pub fn new(
        binary: impl Into<PathBuf>,
        default_voice: Option<String>,
    ) -> Result<Self, SpeechError> {
        Self::with_limits(
            binary.into(),
            default_voice,
            RING_CAPACITY_SAMPLES,
            STALL_LIMIT,
        )
    }

    pub(super) fn with_limits(
        binary: PathBuf,
        default_voice: Option<String>,
        ring_samples: usize,
        stall_limit: Duration,
    ) -> Result<Self, SpeechError> {
        let voices = voices::parse(&process::list_voices(&binary)?);
        if voices.is_empty() {
            return Err(SpeechError::unavailable("espeak-ng lists no voices"));
        }
        let sample_rate_hz = process::probe_sample_rate(&binary)?;
        let format = AudioFormat::new(sample_rate_hz, MONO)?;
        let (producer, consumer) = HeapRb::<f32>::new(ring_samples).split();
        let shared = Arc::new(Shared::new());
        let (utterances, receiver) = mpsc::sync_channel(MAX_QUEUED_TEXTS as usize);
        let config = Config {
            binary,
            sample_rate_hz,
            stall_limit,
        };
        let worker_shared = Arc::clone(&shared);
        thread::Builder::new()
            .name(WORKER_NAME.into())
            .spawn(move || run(worker_shared, producer, receiver, config))
            .map_err(|e| SpeechError::unavailable(&format!("worker thread: {e}")))?;
        Ok(Self {
            info: info(&voices)?,
            voices,
            default_voice,
            format,
            shared,
            consumer,
            utterances,
            reply: None,
        })
    }

    /// Samples lost to a stalled consumer since the engine loaded; the
    /// pipeline reports it with its other overflow counters.
    pub fn dropped_samples(&self) -> u64 {
        self.shared.dropped.load(Ordering::Relaxed)
    }

    /// `None` takes espeak-ng's own default voice.
    fn pick_voice(&self, request: &TtsRequest<'_>) -> Result<Option<String>, SpeechError> {
        if let Some(id) = request.voice.or(self.default_voice.as_deref()) {
            let found = self.voices.iter().find(|v| v.id == id);
            return found
                .map(|v| Some(v.id.clone()))
                .ok_or_else(|| SpeechError::unsupported(id));
        }
        match request.language {
            Some(tag) => voices::choose(&self.voices, tag)
                .map(|v| Some(v.id.clone()))
                .ok_or_else(|| SpeechError::unsupported(tag)),
            None => Ok(None),
        }
    }
}

fn info(voices: &[Voice]) -> Result<EngineInfo, SpeechError> {
    let languages: BTreeSet<String> = voices.iter().filter_map(|v| v.language.clone()).collect();
    Ok(EngineInfo {
        id: EngineId::new(ENGINE_ID)?,
        kind: EngineKind::Tts,
        name: ENGINE_NAME.into(),
        capabilities: Capabilities {
            // Each sentence is its own process, so the first one plays while
            // the rest is still queued.
            streaming: true,
            languages: LanguageSupport::Listed(languages.into_iter().collect()),
            offline: true,
            needs_network: false,
        },
    })
}

impl TtsEngine for EspeakTts {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn voices(&self) -> &[Voice] {
        &self.voices
    }

    fn begin(&mut self, request: &TtsRequest<'_>) -> Result<AudioFormat, SpeechError> {
        self.cancel();
        let voice = self.pick_voice(request)?;
        self.reply = Some(Reply {
            generation: self.shared.generation.load(Ordering::Acquire),
            voice,
            words_per_minute: voices::words_per_minute(request.rate),
            pushed: 0,
            finished: false,
        });
        Ok(self.format)
    }

    fn push_text(&mut self, text: &str) -> Result<(), SpeechError> {
        let Some(reply) = self.reply.as_mut().filter(|r| !r.finished) else {
            return Err(SpeechError::OutOfOrder("text outside a reply"));
        };
        if text.len() > MAX_TEXT_BYTES {
            return Err(SpeechError::invalid("tts text", "longer than 4096 bytes"));
        }
        // espeak-ng would answer with no audio at all, which is not an error.
        if text.trim().is_empty() {
            return Ok(());
        }
        let ended = self.shared.ended.load(Ordering::Acquire);
        if reply.pushed.saturating_sub(ended) >= MAX_QUEUED_TEXTS {
            return Err(SpeechError::Overflow { dropped: 0 });
        }
        let utterance = Utterance {
            generation: reply.generation,
            text: text.to_owned(),
            voice: reply.voice.clone(),
            words_per_minute: reply.words_per_minute,
        };
        self.utterances.try_send(utterance).map_err(|e| match e {
            TrySendError::Full(_) => SpeechError::Overflow { dropped: 0 },
            TrySendError::Disconnected(_) => SpeechError::failed("speech worker stopped"),
        })?;
        reply.pushed += 1;
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
        let failure = self.shared.lock().take();
        if let Some(error) = failure {
            self.cancel();
            return Err(error);
        }
        if reply.finished && ended >= reply.pushed {
            self.reply = None;
            return Ok(TtsPoll::Done);
        }
        Ok(TtsPoll::Pending)
    }

    fn cancel(&mut self) {
        {
            // Under the lock, so no utterance of the old generation can be
            // halfway through a write when the ring is cleared below.
            let mut failure = self.shared.lock();
            self.shared.generation.fetch_add(1, Ordering::AcqRel);
            self.shared.ended.store(0, Ordering::Release);
            *failure = None;
            self.consumer.clear();
        }
        self.shared.kill_child();
        self.reply = None;
    }
}

impl Drop for EspeakTts {
    fn drop(&mut self) {
        // The worker reaps the killed child and then exits when the channel closes.
        self.cancel();
    }
}

impl std::fmt::Debug for EspeakTts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let replying = self.reply.is_some();
        let mut out = f.debug_struct("EspeakTts");
        out.field("replying", &replying).finish_non_exhaustive()
    }
}
