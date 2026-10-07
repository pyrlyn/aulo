//! [`WindowsTts`]: the push/poll engine the pipeline drives. It never touches
//! the synthesizer itself: text goes to the worker over a bounded channel,
//! audio comes back through a fixed-size lock-free ring, so every method
//! returns at once.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use aulo_speech::{
    AudioFormat, Capabilities, EngineId, EngineInfo, EngineKind, LanguageSupport, SpeechError,
    SpeechRate, TtsEngine, TtsPoll, TtsRequest, Voice,
};
use ringbuf::traits::{Consumer, Split};
use ringbuf::{HeapCons, HeapRb};

use super::ENGINE_ID;
use super::voices;
use super::worker::{self, Backend, OUTPUT_HZ, Shared, Utterance};

const ENGINE_NAME: &str = "Windows system voices";
const WORKER_NAME: &str = "aulo-winspeech";
const MONO: u8 = 1;

/// About 24 s of 22.05 kHz mono (2 MiB), allocated once per engine. A full
/// ring only pauses the worker, so this bounds memory, not loss.
const RING_CAPACITY_SAMPLES: usize = 1 << 19;
/// Sentences queued but not yet fully synthesized. The normalizer sends one
/// sentence per push, so this is several replies' worth.
pub(super) const MAX_QUEUED_TEXTS: u64 = 64;
/// One push is one sentence (240 chars from the normalizer); anything far
/// longer is not from it and is refused.
pub(super) const MAX_TEXT_BYTES: usize = 4 * 1024;
/// Opening the synthesizer loads the voice list; far beyond that it is stuck.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);

/// Text-to-speech over the voices installed in Windows. Output is mono at
/// 22.05 kHz whatever the voice's own rate.
pub struct WindowsTts {
    info: EngineInfo,
    voices: Vec<Voice>,
    default: Option<usize>,
    default_voice: Option<String>,
    shared: Arc<Shared>,
    consumer: HeapCons<f32>,
    utterances: SyncSender<Utterance>,
    worker: JoinHandle<()>,
    reply: Option<Reply>,
}

struct Reply {
    generation: u64,
    voice: String,
    rate: SpeechRate,
    pushed: u64,
    finished: bool,
}

impl WindowsTts {
    /// Runs `open` on a new worker thread, which then owns the backend, and
    /// waits for its voice list. A backend that fails to open, or lists no
    /// voices, makes the engine unavailable so the registry falls back.
    pub(super) fn start<B: Backend>(
        open: impl FnOnce() -> Result<B, SpeechError> + Send + 'static,
        default_voice: Option<String>,
    ) -> Result<Self, SpeechError> {
        let (producer, consumer) = HeapRb::<f32>::new(RING_CAPACITY_SAMPLES).split();
        let shared = Arc::new(Shared::new(producer));
        let (utterances, receiver) = mpsc::sync_channel(MAX_QUEUED_TEXTS as usize);
        let (ready, listed) = mpsc::sync_channel(1);
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name(WORKER_NAME.into())
            .spawn(move || {
                let opened = open().and_then(|backend| Ok((backend.catalog()?, backend)));
                match opened {
                    Ok((catalog, backend)) => {
                        if ready.send(Ok(catalog)).is_ok() {
                            worker::run(backend, &worker_shared, &receiver);
                        }
                    }
                    Err(error) => drop(ready.send(Err(error))),
                }
            })
            .map_err(|e| SpeechError::unavailable(&format!("worker thread: {e}")))?;
        let catalog = listed
            .recv_timeout(STARTUP_TIMEOUT)
            .map_err(|_| SpeechError::unavailable("Windows speech did not start in time"))??;
        if catalog.voices.is_empty() {
            return Err(SpeechError::unavailable("no system voices are installed"));
        }
        Ok(Self {
            info: info(&catalog.voices)?,
            voices: catalog.voices,
            default: catalog.default,
            default_voice,
            shared,
            consumer,
            utterances,
            worker,
            reply: None,
        })
    }

    fn pick_voice(&self, request: &TtsRequest<'_>) -> Result<usize, SpeechError> {
        if let Some(id) = request.voice.or(self.default_voice.as_deref()) {
            let found = self.voices.iter().position(|v| v.id == id);
            return found.ok_or_else(|| SpeechError::unsupported(id));
        }
        match request.language {
            Some(tag) => voices::choose(&self.voices, tag),
            None => self.default,
        }
        .ok_or_else(|| SpeechError::unsupported(request.language.unwrap_or("default voice")))
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

impl TtsEngine for WindowsTts {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn voices(&self) -> &[Voice] {
        &self.voices
    }

    fn begin(&mut self, request: &TtsRequest<'_>) -> Result<AudioFormat, SpeechError> {
        self.cancel();
        let index = self.pick_voice(request)?;
        let format = AudioFormat::new(OUTPUT_HZ, MONO)?;
        self.reply = Some(Reply {
            generation: self.shared.generation.load(Ordering::Acquire),
            voice: self.voices[index].id.clone(),
            rate: request.rate,
            pushed: 0,
            finished: false,
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
        // Nothing to synthesize, and no sentence to wait for at the end.
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
            rate: reply.rate,
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
        if let Some(error) = self.shared.take_failure() {
            self.cancel();
            return Err(error);
        }
        if reply.finished && ended >= reply.pushed {
            self.reply = None;
            return Ok(TtsPoll::Done);
        }
        if self.worker.is_finished() {
            self.cancel();
            return Err(SpeechError::failed("speech worker stopped"));
        }
        Ok(TtsPoll::Pending)
    }

    fn cancel(&mut self) {
        self.shared.reset();
        self.consumer.clear();
        self.reply = None;
    }
}

impl Drop for WindowsTts {
    fn drop(&mut self) {
        // A worker waiting on a full ring would never see the channel close.
        self.cancel();
    }
}

impl std::fmt::Debug for WindowsTts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let replying = self.reply.is_some();
        let mut out = f.debug_struct("WindowsTts");
        out.field("replying", &replying).finish_non_exhaustive()
    }
}
