use std::mem;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use aulo_models::Model;
use aulo_speech::{
    AudioFormat, AudioFrame, Capabilities, EngineInfo, EngineKind, EngineSpec, LanguageSupport,
    PIPELINE_SAMPLE_RATE_HZ, SpeechError, SttEngine, SttPoll, Transcript, TranscriptKind, TurnId,
};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};

use crate::model::{PARAKEET_MODEL_ID, ParakeetFiles};
use crate::worker::{Job, Reply, Worker, reply_channel};

/// The id config uses to pick this engine (`[voice.stt] engine = ...`).
pub const PARAKEET_ENGINE_ID: &str = "sherpa-parakeet";

/// Longest transcript handed to the pipeline, in bytes. Model output is
/// untrusted text that ends up in a prompt; a minute of speech is about a
/// kilobyte, so this only bites on a misbehaving model.
pub const MAX_TRANSCRIPT_BYTES: usize = 16 * 1024;

/// Upper bound for [`ParakeetConfig::max_utterance`]. The audio buffer is
/// sized for it up front, and decoding memory grows with segment length.
pub const MAX_UTTERANCE_LIMIT: Duration = Duration::from_secs(300);

/// Most threads a decode may use; more than the cores only adds contention.
pub const MAX_THREADS: u16 = 64;

/// The 25 languages on the model card, checked 2026-10-07:
/// <https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3>. The model detects
/// which one it hears, so a language hint only selects the engine.
const LANGUAGES: [&str; 25] = [
    "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt", "lv", "mt",
    "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "uk",
];

/// sherpa-onnx's name for the NeMo TDT transducer layout.
const MODEL_TYPE: &str = "nemo_transducer";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParakeetConfig {
    /// Decoding threads. Four gave a real-time factor of 0.07 on an M3 Max,
    /// one gave 0.25 (spike T0.5, R3).
    pub threads: u16,
    /// Longest VAD segment kept; audio beyond it is dropped as overflow.
    pub max_utterance: Duration,
}

impl Default for ParakeetConfig {
    fn default() -> Self {
        Self {
            threads: 4,
            max_utterance: Duration::from_secs(60),
        }
    }
}

/// Builds [`ParakeetStt`] engines for the registry. The model is loaded on
/// the first build, not here, so a daemon that never selects this engine
/// never pays its 1.4 GB; every engine built afterwards shares that load.
#[derive(Debug)]
pub struct ParakeetFactory {
    files: ParakeetFiles,
    threads: u16,
    max_samples: usize,
    worker: OnceLock<Result<Worker, SpeechError>>,
}

impl ParakeetFactory {
    /// `model` is the catalog entry and `dir` its install directory from
    /// `ModelManager::path`. The files are validated here, before sherpa-onnx
    /// ever sees them.
    pub fn new(model: &Model, dir: &Path, config: ParakeetConfig) -> Result<Self, SpeechError> {
        if config.threads == 0 || config.threads > MAX_THREADS {
            return Err(SpeechError::invalid("threads", "1 to 64"));
        }
        if config.max_utterance.is_zero() || config.max_utterance > MAX_UTTERANCE_LIMIT {
            return Err(SpeechError::invalid("max utterance", "0 to 5 minutes"));
        }
        let max_samples =
            (config.max_utterance.as_secs_f64() * f64::from(PIPELINE_SAMPLE_RATE_HZ)) as usize;
        Ok(Self {
            files: ParakeetFiles::locate(model, dir)?,
            threads: config.threads,
            max_samples,
            worker: OnceLock::new(),
        })
    }

    /// A failed load is remembered: every later build fails at once with the
    /// same error, so the registry falls back without retrying a broken model
    /// on every utterance.
    pub fn build(&self, spec: &EngineSpec) -> Result<Box<dyn SttEngine>, SpeechError> {
        if spec
            .model
            .as_deref()
            .is_some_and(|m| m != PARAKEET_MODEL_ID)
        {
            return Err(SpeechError::unsupported("model is not available here"));
        }
        let worker = self
            .worker
            .get_or_init(|| {
                let (files, threads) = (self.files.clone(), self.threads);
                Worker::spawn(move || load(files, threads))
            })
            .clone()?;
        Ok(Box::new(ParakeetStt::new(spec, worker, self.max_samples)))
    }

    /// The closure form `EngineRegistry::register` takes.
    pub fn into_factory(
        self,
    ) -> impl Fn(&EngineSpec) -> Result<Box<dyn SttEngine>, SpeechError> + Send + Sync + 'static
    {
        move |spec| self.build(spec)
    }
}

fn load(
    files: ParakeetFiles,
    threads: u16,
) -> Result<impl FnMut(&[f32]) -> Option<String>, SpeechError> {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(files.encoder),
        decoder: Some(files.decoder),
        joiner: Some(files.joiner),
    };
    config.model_config.tokens = Some(files.tokens);
    config.model_config.model_type = Some(MODEL_TYPE.into());
    config.model_config.num_threads = i32::from(threads);
    config.model_config.provider = Some("cpu".into());
    config.decoding_method = Some("greedy_search".into());
    let recognizer = OfflineRecognizer::create(&config)
        .ok_or_else(|| SpeechError::unavailable("sherpa-onnx refused the Parakeet model"))?;
    // The rate always fits: it is the pipeline constant.
    let rate = PIPELINE_SAMPLE_RATE_HZ as i32;
    Ok(move |samples: &[f32]| {
        let stream = recognizer.create_stream();
        stream.accept_waveform(rate, samples);
        recognizer.decode(&stream);
        stream.get_result().map(|result| result.text)
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

/// Parakeet TDT v3 on one VAD segment at a time: frames are buffered in
/// storage sized once for the longest segment, and the whole segment is
/// decoded on the worker after `finish`. Not streaming: no partials.
#[derive(Debug)]
pub struct ParakeetStt {
    info: EngineInfo,
    worker: Worker,
    samples: Vec<f32>,
    max_samples: usize,
    state: State,
}

impl ParakeetStt {
    fn new(spec: &EngineSpec, worker: Worker, max_samples: usize) -> Self {
        Self {
            info: EngineInfo {
                id: spec.engine.clone(),
                kind: EngineKind::Stt,
                name: "NVIDIA Parakeet TDT 0.6B v3 (sherpa-onnx)".into(),
                capabilities: Capabilities {
                    streaming: false,
                    languages: LanguageSupport::Listed(
                        LANGUAGES.iter().map(|&tag| tag.to_owned()).collect(),
                    ),
                    offline: true,
                    needs_network: false,
                },
            },
            worker,
            samples: Vec::new(),
            max_samples,
            state: State::Idle,
        }
    }
}

impl SttEngine for ParakeetStt {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn begin(&mut self, turn_id: TurnId, language: Option<&str>) -> Result<(), SpeechError> {
        if let Some(tag) = language
            && !self.info.capabilities.languages.supports(tag)
        {
            return Err(SpeechError::unsupported(tag));
        }
        self.cancel();
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
        let (reply, replies) = reply_channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let samples = mem::take(&mut self.samples);
        if samples.is_empty() {
            // Nothing was said; the contract still wants one final.
            let _ = reply.try_send(Reply {
                text: Some(String::new()),
                samples,
            });
        } else {
            self.worker.submit(Job {
                samples,
                cancelled: Arc::clone(&cancelled),
                reply,
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
        let Some(text) = reply.text else {
            self.state = State::Idle;
            return Err(SpeechError::failed("Parakeet returned no result"));
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

    use aulo_speech::{EngineId, SpeechRate};

    use super::*;

    const FRAME: usize = 320;
    const MAX_SAMPLES: usize = FRAME * 4;

    fn spec(model: Option<&str>) -> EngineSpec {
        EngineSpec {
            engine: EngineId::new(PARAKEET_ENGINE_ID).unwrap(),
            model: model.map(str::to_owned),
            voice: None,
            rate: SpeechRate::NORMAL,
        }
    }

    /// An engine over a fake decoder that "hears" the sample count, and
    /// waits on `gate` first so tests can hold a decode in flight.
    fn engine(gate: Arc<Mutex<()>>, text: Option<&'static str>) -> ParakeetStt {
        let worker = Worker::spawn(move || {
            Ok(move |samples: &[f32]| {
                let _held = gate.lock().unwrap();
                text.map(|t| format!(" {t} {} ", samples.len()))
            })
        })
        .unwrap();
        ParakeetStt::new(&spec(None), worker, MAX_SAMPLES)
    }

    fn push(stt: &mut ParakeetStt, frames: usize) -> Result<(), SpeechError> {
        let samples = [0.1_f32; FRAME];
        for i in 0..frames {
            let at = (i * FRAME) as u64;
            stt.push(AudioFrame::new(&samples, AudioFormat::PIPELINE, at).unwrap())?;
        }
        Ok(())
    }

    fn wait(stt: &mut ParakeetStt, out: &mut Transcript) -> Result<SttPoll, SpeechError> {
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
    fn a_decoder_without_a_result_fails_over() {
        let mut stt = engine(Arc::default(), None);
        let mut out = Transcript::with_capacity(TurnId::new(), 8);
        stt.begin(TurnId::new(), None).unwrap();
        push(&mut stt, 1).unwrap();
        stt.finish().unwrap();
        assert!(wait(&mut stt, &mut out).unwrap_err().should_fall_back());
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
    fn factory_validates_config_and_model_before_loading() {
        let catalog = aulo_models::Catalog::embedded().unwrap();
        let model = catalog.get(PARAKEET_MODEL_ID).unwrap();
        let missing = tempfile::tempdir().unwrap();
        let error = ParakeetFactory::new(model, missing.path(), ParakeetConfig::default());
        assert!(matches!(error, Err(SpeechError::Unavailable(_))));
        for config in [
            ParakeetConfig {
                threads: 0,
                ..ParakeetConfig::default()
            },
            ParakeetConfig {
                max_utterance: MAX_UTTERANCE_LIMIT + Duration::from_secs(1),
                ..ParakeetConfig::default()
            },
        ] {
            let error = ParakeetFactory::new(model, missing.path(), config).unwrap_err();
            assert!(matches!(error, SpeechError::Invalid { .. }));
        }
    }

    #[test]
    fn another_model_name_is_refused_without_loading() {
        let (dir, model) = crate::model::tests::bundle(crate::model::tests::TOKEN_TABLE);
        let factory = ParakeetFactory::new(&model, dir.path(), ParakeetConfig::default()).unwrap();
        let built = factory.build(&spec(Some("whisper-large")));
        assert!(matches!(built, Err(SpeechError::Unsupported(_))));
        assert!(factory.worker.get().is_none());
    }
}
