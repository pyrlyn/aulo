use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::sync::{Arc, LazyLock, OnceLock};

use aulo_models::Model;
use aulo_speech::{
    AudioFormat, Capabilities, EngineInfo, EngineKind, EngineSpec, LanguageSupport, SpeechError,
    TtsEngine, TtsPoll, TtsRequest, Voice,
};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsModelConfig,
};

use crate::model::{KOKORO_MODEL_ID, KokoroFiles};
use crate::worker::{Job, Worker, check_threads};

/// The id config uses to pick this engine (`[voice.tts] engine = ...`).
pub const KOKORO_ENGINE_ID: &str = "sherpa-kokoro";

/// Kokoro's output rate (`sample_rate` in the model metadata).
pub const KOKORO_SAMPLE_RATE_HZ: u32 = 24_000;

/// Longest text one `push_text` takes, in bytes. The text comes from a model,
/// so it is untrusted; a spoken sentence is far shorter, and sherpa-onnx keeps
/// all audio of one text in memory until it returns.
pub const MAX_SPEECH_TEXT_BYTES: usize = 2048;

/// 100 ms at 24 kHz. Small chunks keep `cancel` and first audio quick.
pub const CHUNK_SAMPLES: usize = 2_400;

/// Sentences waiting behind the one being spoken. A full queue is `Overflow`
/// rather than a block, because `push_text` must not wait.
const TEXT_QUEUE: usize = 64;

/// Chunks synthesis may run ahead of playback, 0.8 s; when they are not
/// polled, synthesis pauses instead of buffering the rest of the reply.
const AUDIO_QUEUE: usize = 8;

const DEFAULT_VOICE: &str = "af_heart";

/// Speaker ids in order, so the index is the id: `id2speaker` in
/// <https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.8/scripts/kokoro/v1.0/generate_voices_bin.py>,
/// checked 2026-10-07. The bundle's model metadata (`speaker2id`) agrees.
#[rustfmt::skip]
pub(crate) const SPEAKERS: [&str; 54] = [
    "af_alloy", "af_aoede", "af_bella", "af_heart", "af_jessica", "af_kore",
    "af_nicole", "af_nova", "af_river", "af_sarah", "af_sky",
    "am_adam", "am_echo", "am_eric", "am_fenrir", "am_liam", "am_michael",
    "am_onyx", "am_puck", "am_santa",
    "bf_alice", "bf_emma", "bf_isabella", "bf_lily",
    "bm_daniel", "bm_fable", "bm_george", "bm_lewis",
    "ef_dora", "em_alex",
    "ff_siwis",
    "hf_alpha", "hf_beta", "hm_omega", "hm_psi",
    "if_sara", "im_nicola",
    "jf_alpha", "jf_gongitsune", "jf_nezumi", "jf_tebukuro", "jm_kumo",
    "pf_dora", "pm_alex", "pm_santa",
    "zf_xiaobei", "zf_xiaoni", "zf_xiaoxiao", "zf_xiaoyi",
    "zm_yunjian", "zm_yunxi", "zm_yunxia", "zm_yunyang",
    "em_santa",
];

/// A Kokoro language, named by the first letter of its voice ids (the second
/// is the gender). Languages, espeak-ng voices and grades are from
/// <https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md>, checked
/// 2026-10-07; the default is the best-graded voice. There is no Russian.
#[derive(Debug)]
struct Language {
    code: char,
    tag: &'static str,
    name: &'static str,
    /// The phonemizer voice sherpa-onnx takes as `lang`.
    espeak: &'static str,
    default: &'static str,
}

const LANGUAGES: [Language; 9] = [
    lang('a', "en-US", "American English", "en-us", "af_heart"),
    lang('b', "en-GB", "British English", "en-gb", "bf_emma"),
    lang('e', "es", "Spanish", "es", "ef_dora"),
    lang('f', "fr", "French", "fr-fr", "ff_siwis"),
    lang('h', "hi", "Hindi", "hi", "hf_alpha"),
    lang('i', "it", "Italian", "it", "if_sara"),
    // VOICES.md names no espeak-ng voice for ja; espeak-ng's own is used.
    lang('j', "ja", "Japanese", "ja", "jf_alpha"),
    lang('p', "pt-BR", "Brazilian Portuguese", "pt-br", "pf_dora"),
    // Han characters go through the bundle's Chinese lexicon whatever `lang`
    // says; it only phonemizes the Latin words between them.
    lang('z', "zh", "Mandarin Chinese", "en-us", "zf_xiaobei"),
];

#[rustfmt::skip]
const fn lang(
    code: char, tag: &'static str, name: &'static str, espeak: &'static str, default: &'static str,
) -> Language {
    Language { code, tag, name, espeak, default }
}

static VOICES: LazyLock<Vec<Voice>> = LazyLock::new(|| {
    SPEAKERS
        .iter()
        .filter_map(|&id| {
            let language = language_of(id)?;
            Some(Voice {
                id: id.to_owned(),
                name: display_name(id, language),
                language: Some(language.tag.to_owned()),
            })
        })
        .collect()
});

/// `af_heart` becomes "Heart (American English, female)".
fn display_name(id: &str, language: &Language) -> String {
    let given = id.get(3..).unwrap_or(id);
    let (first, rest) = given.split_at(given.chars().next().map_or(0, char::len_utf8));
    let gender = if id.get(1..2) == Some("m") {
        "male"
    } else {
        "female"
    };
    format!(
        "{}{rest} ({}, {gender})",
        first.to_ascii_uppercase(),
        language.name
    )
}

fn language_of(voice: &str) -> Option<&'static Language> {
    let code = voice.chars().next()?;
    LANGUAGES.iter().find(|l| l.code == code)
}

fn primary(tag: &str) -> &str {
    tag.split('-').next().unwrap_or(tag)
}

/// The exact tag first, so `en-GB` gets British voices, then the primary
/// subtag, so `en` and `zh-CN` are served too.
fn language_for(tag: &str) -> Option<&'static Language> {
    LANGUAGES
        .iter()
        .find(|l| l.tag.eq_ignore_ascii_case(tag))
        .or_else(|| {
            LANGUAGES
                .iter()
                .find(|l| primary(l.tag).eq_ignore_ascii_case(primary(tag)))
        })
}

fn speaker(id: &str) -> Option<(i32, &'static Language)> {
    let sid = SPEAKERS.iter().position(|s| *s == id)?;
    Some((i32::try_from(sid).ok()?, language_of(id)?))
}

/// How one reply is spoken: who, in which phonemes, how fast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Voicing {
    pub(crate) sid: i32,
    pub(crate) espeak: &'static str,
    pub(crate) speed: f32,
}

fn voicing(configured: Option<&str>, request: &TtsRequest<'_>) -> Result<Voicing, SpeechError> {
    let language = request
        .language
        .map(|tag| language_for(tag).ok_or_else(|| SpeechError::unsupported(tag)))
        .transpose()?;
    let same = |voice: &str, l: &Language| {
        language_of(voice).is_some_and(|own| primary(own.tag) == primary(l.tag))
    };
    // A configured voice of another language would read this text with an
    // accent, so the language's default replaces it; a requested voice wins.
    let id = request
        .voice
        .or_else(|| configured.filter(|v| language.is_none_or(|l| same(v, l))))
        .unwrap_or(language.map_or(DEFAULT_VOICE, |l| l.default));
    let (sid, own) = speaker(id).ok_or_else(|| SpeechError::unsupported(id))?;
    let phonemes = language.filter(|l| !same(id, l)).unwrap_or(own);
    Ok(Voicing {
        sid,
        espeak: phonemes.espeak,
        speed: request.rate.get(),
    })
}

/// Where one reply's audio goes, on the worker side.
#[derive(Debug, Clone)]
pub(crate) struct Sink {
    audio: SyncSender<Vec<f32>>,
    cancelled: Arc<AtomicBool>,
}

impl Sink {
    /// Hands audio over in chunks of at most [`CHUNK_SAMPLES`]. Blocks while
    /// the reply's queue is full, which is the backpressure. `false` asks the
    /// synthesizer to stop: the reply was cancelled or dropped.
    pub(crate) fn push(&self, samples: &[f32]) -> bool {
        samples
            .chunks(CHUNK_SAMPLES)
            .all(|chunk| !self.is_cancelled() && self.audio.send(chunk.to_vec()).is_ok())
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// One pushed text to speak.
#[derive(Debug)]
pub(crate) struct SpeakJob {
    text: String,
    voicing: Voicing,
    sink: Sink,
}

impl Job for SpeakJob {
    fn cancelled(&self) -> bool {
        self.sink.is_cancelled()
    }
}

/// Runs `speak` on the worker thread, one pushed text per job, in order.
fn synthesizer<L, S>(load: L) -> Result<Worker<SpeakJob>, SpeechError>
where
    L: FnOnce() -> Result<S, SpeechError> + Send + 'static,
    S: FnMut(&str, Voicing, Sink),
{
    Worker::spawn("aulo-sherpa-tts", TEXT_QUEUE, move || {
        let mut speak = load()?;
        Ok(move |job: SpeakJob| speak(&job.text, job.voicing, job.sink))
    })
}

fn load(files: KokoroFiles, threads: u16) -> Result<impl FnMut(&str, Voicing, Sink), SpeechError> {
    let config = OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            kokoro: OfflineTtsKokoroModelConfig {
                model: Some(files.model),
                voices: Some(files.voices),
                tokens: Some(files.tokens),
                data_dir: Some(files.data_dir),
                dict_dir: Some(files.dict_dir),
                lexicon: Some(files.lexicon),
                ..OfflineTtsKokoroModelConfig::default()
            },
            num_threads: i32::from(threads),
            provider: Some("cpu".into()),
            ..OfflineTtsModelConfig::default()
        },
        // The bundle's Chinese number and date FSTs stay off: sherpa-onnx
        // would rewrite digits in text of every language with them.
        rule_fsts: None,
        // Kokoro only takes 1 and logs a warning on every text otherwise.
        max_num_sentences: 1,
        ..OfflineTtsConfig::default()
    };
    let tts = OfflineTts::create(&config)
        .ok_or_else(|| SpeechError::unavailable("sherpa-onnx refused the Kokoro model"))?;
    let rate = u32::try_from(tts.sample_rate()).ok();
    let speakers = usize::try_from(tts.num_speakers()).ok();
    if rate != Some(KOKORO_SAMPLE_RATE_HZ) || speakers != Some(SPEAKERS.len()) {
        return Err(SpeechError::unavailable(
            "the Kokoro model is not the v1.0 bundle",
        ));
    }
    // The first synthesis on a fresh model is the slowest (onnxruntime sets
    // up its buffers then), so it is paid here instead of by the first reply.
    let warm_up = Voicing {
        sid: 0,
        espeak: "en-us",
        speed: 1.0,
    };
    let _ = tts.generate_with_config("Hi.", &generation(warm_up), None::<fn(&[f32], f32) -> bool>);
    Ok(move |text: &str, voicing: Voicing, sink: Sink| {
        // Called once per sentence sherpa-onnx splits the text into.
        let callback = move |samples: &[f32], _progress: f32| sink.push(samples);
        // `None` means no audio: the text had nothing Kokoro can pronounce,
        // such as only an emoji. That is silence, not a broken engine, and
        // sherpa-onnx has no other way to fail that does not abort.
        let _ = tts.generate_with_config(text, &generation(voicing), Some(callback));
    })
}

fn generation(voicing: Voicing) -> GenerationConfig {
    GenerationConfig {
        sid: voicing.sid,
        speed: voicing.speed,
        // Per text, so one loaded model speaks every language.
        extra: Some(HashMap::from([("lang".to_owned(), voicing.espeak.into())])),
        ..GenerationConfig::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KokoroConfig {
    /// Synthesis threads. Four gave a real-time factor of 0.23 and first
    /// audio in 0.24-0.30 s on an M3 Max, one gave 0.78 (spike T0.5, R3).
    pub threads: u16,
}

impl Default for KokoroConfig {
    fn default() -> Self {
        Self { threads: 4 }
    }
}

/// Builds [`KokoroTts`] engines for the registry. The model is loaded on the
/// first build, not here, so a daemon that never speaks with Kokoro never
/// pays its 0.9 GB; every engine built afterwards shares that load.
#[derive(Debug)]
pub struct KokoroFactory {
    files: KokoroFiles,
    threads: u16,
    worker: OnceLock<Result<Worker<SpeakJob>, SpeechError>>,
}

impl KokoroFactory {
    /// `model` is the catalog entry and `dir` its install directory from
    /// `ModelManager::path`. The files are validated here, before sherpa-onnx
    /// ever sees them.
    pub fn new(model: &Model, dir: &Path, config: KokoroConfig) -> Result<Self, SpeechError> {
        check_threads(config.threads)?;
        Ok(Self {
            files: KokoroFiles::locate(model, dir, SPEAKERS.len())?,
            threads: config.threads,
            worker: OnceLock::new(),
        })
    }

    /// An unknown model or voice is `Unsupported`, so the registry falls back
    /// without loading anything. A failed load is remembered: later builds
    /// fail at once with the same error instead of retrying a broken model.
    pub fn build(&self, spec: &EngineSpec) -> Result<Box<dyn TtsEngine>, SpeechError> {
        if spec.model.as_deref().is_some_and(|m| m != KOKORO_MODEL_ID) {
            return Err(SpeechError::unsupported("model is not available here"));
        }
        if let Some(voice) = spec.voice.as_deref()
            && speaker(voice).is_none()
        {
            return Err(SpeechError::unsupported(voice));
        }
        let worker = self
            .worker
            .get_or_init(|| {
                let (files, threads) = (self.files.clone(), self.threads);
                synthesizer(move || load(files, threads))
            })
            .clone()?;
        Ok(Box::new(KokoroTts::new(spec, worker)))
    }

    /// The closure form `EngineRegistry::register` takes.
    pub fn into_factory(
        self,
    ) -> impl Fn(&EngineSpec) -> Result<Box<dyn TtsEngine>, SpeechError> + Send + Sync + 'static
    {
        move |spec| self.build(spec)
    }
}

#[derive(Debug)]
enum State {
    Idle,
    Speaking(Reply),
}

/// One reply. It gets its own audio channel, so output a cancelled reply
/// produces late can never reach the next one.
#[derive(Debug)]
struct Reply {
    voicing: Voicing,
    /// `None` after `finish`. Once the queued jobs drop their clones too, the
    /// channel disconnects, which is how `poll` knows the reply is complete.
    sink: Option<Sink>,
    cancelled: Arc<AtomicBool>,
    audio: Receiver<Vec<f32>>,
    chunk: Vec<f32>,
    offset: usize,
    /// Every job is done, held back until the audio before it is delivered.
    complete: bool,
}

/// Kokoro 82M v1.0: each pushed text is one job on the worker, spoken in
/// order, so the first sentence plays while later ones are generated.
#[derive(Debug)]
pub struct KokoroTts {
    info: EngineInfo,
    voice: Option<String>,
    worker: Worker<SpeakJob>,
    state: State,
}

impl KokoroTts {
    fn new(spec: &EngineSpec, worker: Worker<SpeakJob>) -> Self {
        Self {
            info: EngineInfo {
                id: spec.engine.clone(),
                kind: EngineKind::Tts,
                name: "Kokoro 82M v1.0 (sherpa-onnx)".into(),
                capabilities: Capabilities {
                    streaming: true,
                    languages: LanguageSupport::Listed(
                        LANGUAGES.iter().map(|l| l.tag.to_owned()).collect(),
                    ),
                    offline: true,
                    needs_network: false,
                },
            },
            voice: spec.voice.clone(),
            worker,
            state: State::Idle,
        }
    }
}

impl TtsEngine for KokoroTts {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn voices(&self) -> &[Voice] {
        &VOICES
    }

    fn begin(&mut self, request: &TtsRequest<'_>) -> Result<AudioFormat, SpeechError> {
        self.cancel();
        let voicing = voicing(self.voice.as_deref(), request)?;
        let format = AudioFormat::new(KOKORO_SAMPLE_RATE_HZ, 1)?;
        let (audio_sender, audio) = sync_channel(AUDIO_QUEUE);
        let cancelled = Arc::new(AtomicBool::new(false));
        self.state = State::Speaking(Reply {
            voicing,
            sink: Some(Sink {
                audio: audio_sender,
                cancelled: Arc::clone(&cancelled),
            }),
            cancelled,
            audio,
            chunk: Vec::new(),
            offset: 0,
            complete: false,
        });
        Ok(format)
    }

    fn push_text(&mut self, text: &str) -> Result<(), SpeechError> {
        let State::Speaking(Reply {
            sink: Some(sink),
            voicing,
            ..
        }) = &self.state
        else {
            return Err(SpeechError::OutOfOrder("text outside a reply"));
        };
        let text = text.trim_matches(|c: char| c.is_whitespace() || c.is_control());
        if text.is_empty() {
            return Ok(());
        }
        if text.len() > MAX_SPEECH_TEXT_BYTES {
            return Err(SpeechError::invalid(
                "text",
                "longer than the engine allows",
            ));
        }
        // Control characters are not speech, and a NUL would make the
        // sherpa-onnx crate panic in `CString::new`.
        let text = text
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let job = SpeakJob {
            text,
            voicing: *voicing,
            sink: sink.clone(),
        };
        self.worker.submit(job).map_err(|error| match error {
            TrySendError::Full(job) => SpeechError::Overflow {
                dropped: job.text.chars().count(),
            },
            TrySendError::Disconnected(_) => SpeechError::unavailable("speech worker stopped"),
        })
    }

    fn finish(&mut self) -> Result<(), SpeechError> {
        let State::Speaking(reply) = &mut self.state else {
            return Err(SpeechError::OutOfOrder("finish before begin"));
        };
        reply.sink = None;
        Ok(())
    }

    fn poll(&mut self, out: &mut [f32]) -> Result<TtsPoll, SpeechError> {
        let State::Speaking(reply) = &mut self.state else {
            return Ok(TtsPoll::Done);
        };
        let mut written = 0;
        while written < out.len() {
            if reply.offset < reply.chunk.len() {
                let n = (reply.chunk.len() - reply.offset).min(out.len() - written);
                out[written..written + n]
                    .copy_from_slice(&reply.chunk[reply.offset..reply.offset + n]);
                written += n;
                reply.offset += n;
                continue;
            }
            match reply.audio.try_recv() {
                Ok(samples) => {
                    reply.chunk = samples;
                    reply.offset = 0;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    reply.complete = true;
                    break;
                }
            }
        }
        if written > 0 {
            return Ok(TtsPoll::Audio { samples: written });
        }
        if !reply.complete {
            return Ok(TtsPoll::Pending);
        }
        self.state = State::Idle;
        Ok(TtsPoll::Done)
    }

    fn cancel(&mut self) {
        if let State::Speaking(reply) = &self.state {
            // Queued texts are skipped, and dropping the receiver below stops
            // the text being spoken at its next chunk.
            reply.cancelled.store(true, Ordering::Release);
        }
        self.state = State::Idle;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use std::thread;
    use std::time::{Duration, Instant};

    use aulo_speech::{EngineId, SpeechRate, TurnId};

    use super::*;

    /// What the fake heard, in the order it spoke it.
    type Log = Arc<Mutex<Vec<(String, Voicing)>>>;

    fn spec(voice: Option<&str>) -> EngineSpec {
        EngineSpec {
            engine: EngineId::new(KOKORO_ENGINE_ID).unwrap(),
            model: None,
            voice: voice.map(str::to_owned),
            rate: SpeechRate::NORMAL,
        }
    }

    fn request<'a>(voice: Option<&'a str>, language: Option<&'a str>) -> TtsRequest<'a> {
        TtsRequest {
            turn_id: TurnId::new(),
            voice,
            language,
            rate: SpeechRate::NORMAL,
        }
    }

    /// Speaks text `"<n>x<len>"` as `len` samples of value `n`, handed over
    /// in pieces of 3000 the way sherpa-onnx hands over a sentence. Texts
    /// starting with `wait` hold `gate` first. `sent` counts chunks the sink
    /// accepted.
    struct Fake {
        gate: Arc<Mutex<()>>,
        log: Log,
        sent: Arc<AtomicUsize>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                gate: Arc::default(),
                log: Arc::default(),
                sent: Arc::default(),
            }
        }

        fn engine(&self, voice: Option<&str>) -> KokoroTts {
            let (gate, log, sent) = (
                Arc::clone(&self.gate),
                Arc::clone(&self.log),
                Arc::clone(&self.sent),
            );
            let worker = synthesizer(move || {
                Ok(move |text: &str, voicing: Voicing, sink: Sink| {
                    log.lock().unwrap().push((text.to_owned(), voicing));
                    if text.starts_with("wait") {
                        drop(gate.lock().unwrap());
                    }
                    let (n, len) = parse(text);
                    let samples = vec![n; len];
                    for piece in samples.chunks(3000) {
                        for chunk in piece.chunks(CHUNK_SAMPLES) {
                            if !sink.push(chunk) {
                                return;
                            }
                            sent.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                })
            })
            .unwrap();
            KokoroTts::new(&spec(voice), worker)
        }

        fn spoken(&self) -> Vec<String> {
            self.log
                .lock()
                .unwrap()
                .iter()
                .map(|(t, _)| t.clone())
                .collect()
        }
    }

    fn parse(text: &str) -> (f32, usize) {
        let digits = text.trim_start_matches(|c: char| !c.is_ascii_digit());
        let (n, len) = digits.split_once('x').unwrap_or(("0", "0"));
        let len = len.trim_end_matches(|c: char| !c.is_ascii_digit());
        (n.parse().unwrap_or(0.0), len.parse().unwrap_or(0))
    }

    /// Polls until `Done` and returns all audio of the reply.
    fn drain(tts: &mut KokoroTts, out: &mut [f32]) -> Vec<f32> {
        let mut audio = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match tts.poll(out).unwrap() {
                TtsPoll::Audio { samples } => audio.extend_from_slice(&out[..samples]),
                TtsPoll::Pending => thread::yield_now(),
                TtsPoll::Done => return audio,
            }
        }
        panic!("reply never finished");
    }

    #[test]
    fn sentences_are_spoken_in_order_and_complete() {
        let fake = Fake::new();
        let mut tts = fake.engine(None);
        let format = tts.begin(&request(None, Some("en-US"))).unwrap();
        assert_eq!(format, AudioFormat::new(24_000, 1).unwrap());
        for text in ["1x5000", "2x10", "3x2400"] {
            tts.push_text(text).unwrap();
        }
        tts.finish().unwrap();
        let audio = drain(&mut tts, &mut [0.0; 1000]);
        let mut expected = vec![1.0; 5000];
        expected.extend([2.0; 10]);
        expected.extend([3.0; 2400]);
        assert_eq!(audio, expected);
        assert_eq!(fake.spoken(), ["1x5000", "2x10", "3x2400"]);
        assert_eq!(tts.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
    }

    #[test]
    fn the_first_sentence_plays_while_the_next_is_generated() {
        let fake = Fake::new();
        let held = fake.gate.lock().unwrap();
        let mut tts = fake.engine(None);
        tts.begin(&request(None, None)).unwrap();
        tts.push_text("1x100").unwrap();
        tts.push_text("wait 2x100").unwrap();
        let mut out = [0.0; 400];
        let deadline = Instant::now() + Duration::from_secs(5);
        let first = loop {
            match tts.poll(&mut out).unwrap() {
                TtsPoll::Audio { samples } => break samples,
                _ if Instant::now() < deadline => thread::yield_now(),
                other => panic!("{other:?}"),
            }
        };
        assert_eq!(first, 100);
        assert_eq!(tts.poll(&mut out).unwrap(), TtsPoll::Pending);
        drop(held);
        tts.finish().unwrap();
        let audio = drain(&mut tts, &mut out);
        assert_eq!(audio, [2.0; 100]);
    }

    #[test]
    fn audio_is_handed_over_in_chunks_of_at_most_100_ms() {
        let (audio, chunks) = sync_channel(16);
        let sink = Sink {
            audio,
            cancelled: Arc::default(),
        };
        assert!(sink.push(&[0.5; CHUNK_SAMPLES * 2 + 7]));
        drop(sink);
        let sizes: Vec<usize> = chunks.iter().map(|chunk| chunk.len()).collect();
        assert_eq!(sizes, [CHUNK_SAMPLES, CHUNK_SAMPLES, 7]);
        assert_eq!(CHUNK_SAMPLES as u32 * 10, KOKORO_SAMPLE_RATE_HZ);
    }

    #[test]
    fn synthesis_pauses_when_audio_is_not_polled() {
        let fake = Fake::new();
        let mut tts = fake.engine(None);
        tts.begin(&request(None, None)).unwrap();
        tts.push_text(&format!("1x{}", CHUNK_SAMPLES * 40)).unwrap();
        tts.finish().unwrap();
        thread::sleep(Duration::from_millis(100));
        // The queue holds AUDIO_QUEUE chunks and the worker blocks on the next.
        assert_eq!(fake.sent.load(Ordering::SeqCst), AUDIO_QUEUE);
        let audio = drain(&mut tts, &mut [0.0; CHUNK_SAMPLES]);
        assert_eq!(audio.len(), CHUNK_SAMPLES * 40);
    }

    #[test]
    fn cancel_is_instant_skips_queued_text_and_fences_late_audio() {
        let fake = Fake::new();
        let mut tts = fake.engine(None);
        tts.begin(&request(None, None)).unwrap();
        // Fills the audio queue and blocks the worker inside the first text.
        tts.push_text(&format!("1x{}", CHUNK_SAMPLES * 40)).unwrap();
        tts.push_text("2x100").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while fake.sent.load(Ordering::SeqCst) < AUDIO_QUEUE && Instant::now() < deadline {
            thread::yield_now();
        }
        let started = Instant::now();
        tts.cancel();
        assert!(started.elapsed() < Duration::from_millis(150));
        assert_eq!(tts.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
        tts.begin(&request(None, None)).unwrap();
        tts.push_text("3x50").unwrap();
        tts.finish().unwrap();
        let audio = drain(&mut tts, &mut [0.0; 64]);
        assert_eq!(audio, [3.0; 50]);
        assert_eq!(fake.spoken(), ["1x96000", "3x50"]);
        assert!(fake.sent.load(Ordering::SeqCst) <= AUDIO_QUEUE + 1 + 1);
    }

    #[test]
    fn begin_cancels_the_reply_before_it() {
        let fake = Fake::new();
        let mut tts = fake.engine(None);
        tts.begin(&request(None, None)).unwrap();
        tts.push_text("1x10").unwrap();
        tts.begin(&request(None, None)).unwrap();
        tts.finish().unwrap();
        let audio = drain(&mut tts, &mut [0.0; 64]);
        assert!(audio.is_empty());
    }

    #[test]
    fn text_is_capped_cleaned_and_bounded() {
        let fake = Fake::new();
        let held = fake.gate.lock().unwrap();
        let mut tts = fake.engine(None);
        tts.begin(&request(None, None)).unwrap();
        let long = "a".repeat(MAX_SPEECH_TEXT_BYTES + 1);
        assert!(matches!(
            tts.push_text(&long),
            Err(SpeechError::Invalid { .. })
        ));
        tts.push_text("  \n ").unwrap();
        tts.push_text("wait\0 1x1\n\u{1b}").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while fake.spoken().is_empty() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert_eq!(fake.spoken(), ["wait  1x1"]);
        // The worker is held inside that text, so the queue fills up.
        for _ in 0..TEXT_QUEUE {
            tts.push_text("2x1").unwrap();
        }
        assert_eq!(
            tts.push_text("2x1"),
            Err(SpeechError::Overflow { dropped: 3 })
        );
        tts.cancel();
        drop(held);
        tts.begin(&request(None, None)).unwrap();
        // The cancelled texts hold their slots until the worker skips them.
        while tts.push_text("3x1").is_err() && Instant::now() < deadline {
            thread::yield_now();
        }
        tts.finish().unwrap();
        assert_eq!(drain(&mut tts, &mut [0.0; 8]), [3.0]);
        assert_eq!(fake.spoken(), ["wait  1x1", "3x1"]);
    }

    #[test]
    fn calls_out_of_order_are_refused() {
        let fake = Fake::new();
        let mut tts = fake.engine(None);
        assert_eq!(tts.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
        assert!(matches!(
            tts.push_text("1x1"),
            Err(SpeechError::OutOfOrder(_))
        ));
        assert!(matches!(tts.finish(), Err(SpeechError::OutOfOrder(_))));
        tts.begin(&request(None, None)).unwrap();
        tts.finish().unwrap();
        assert!(matches!(
            tts.push_text("1x1"),
            Err(SpeechError::OutOfOrder(_))
        ));
        assert!(drain(&mut tts, &mut [0.0; 8]).is_empty());
    }

    #[test]
    fn russian_and_unknown_voices_fall_back() {
        let fake = Fake::new();
        let mut tts = fake.engine(None);
        for request in [request(None, Some("ru-RU")), request(Some("ru_anna"), None)] {
            let error = tts.begin(&request).unwrap_err();
            assert!(matches!(error, SpeechError::Unsupported(_)), "{error}");
        }
        assert!(!tts.info().capabilities.languages.supports("ru"));
        assert!(tts.info().capabilities.languages.supports("pt-PT"));
    }

    #[test]
    fn the_voice_and_phonemes_follow_the_language() {
        let pick = |configured, voice, language| {
            let v = voicing(configured, &request(voice, language)).unwrap();
            (SPEAKERS[v.sid as usize], v.espeak)
        };
        assert_eq!(pick(None, None, None), ("af_heart", "en-us"));
        assert_eq!(pick(None, None, Some("en")), ("af_heart", "en-us"));
        assert_eq!(pick(None, None, Some("en-gb")), ("bf_emma", "en-gb"));
        assert_eq!(pick(None, None, Some("es-MX")), ("ef_dora", "es"));
        assert_eq!(pick(None, None, Some("zh-CN")), ("zf_xiaobei", "en-us"));
        assert_eq!(
            pick(Some("am_adam"), None, Some("en-GB")),
            ("am_adam", "en-us")
        );
        // A configured voice of another language gives way; a requested one does not.
        assert_eq!(
            pick(Some("am_adam"), None, Some("fr")),
            ("ff_siwis", "fr-fr")
        );
        assert_eq!(
            pick(None, Some("am_adam"), Some("fr")),
            ("am_adam", "fr-fr")
        );
        let fast = TtsRequest {
            rate: SpeechRate::new(1.5).unwrap(),
            ..request(None, None)
        };
        assert_eq!(voicing(None, &fast).unwrap().speed, 1.5);
    }

    #[test]
    fn every_speaker_is_a_named_voice_with_a_language() {
        assert_eq!(VOICES.len(), SPEAKERS.len());
        let heart = &VOICES[3];
        assert_eq!(heart.id, "af_heart");
        assert_eq!(heart.name, "Heart (American English, female)");
        assert_eq!(VOICES[53].name, "Santa (Spanish, male)");
        for language in &LANGUAGES {
            let (_, own) = speaker(language.default).unwrap();
            assert_eq!(own.tag, language.tag);
        }
        let mut ids: Vec<_> = SPEAKERS.to_vec();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SPEAKERS.len());
    }

    #[test]
    fn factory_checks_config_model_and_voice_before_loading() {
        let catalog = aulo_models::Catalog::embedded().unwrap();
        let model = catalog.get(KOKORO_MODEL_ID).unwrap();
        let missing = tempfile::tempdir().unwrap();
        let error = KokoroFactory::new(model, missing.path(), KokoroConfig::default());
        assert!(matches!(error, Err(SpeechError::Unavailable(_))));
        let error = KokoroFactory::new(model, missing.path(), KokoroConfig { threads: 0 });
        assert!(matches!(error, Err(SpeechError::Invalid { .. })));

        let (dir, model) = crate::model::tests::kokoro_bundle(SPEAKERS.len());
        let factory = KokoroFactory::new(&model, dir.path(), KokoroConfig::default()).unwrap();
        let other_model = EngineSpec {
            model: Some("piper-ru".into()),
            ..spec(None)
        };
        for spec in [other_model, spec(Some("ru_anna"))] {
            assert!(matches!(
                factory.build(&spec),
                Err(SpeechError::Unsupported(_))
            ));
        }
        assert!(factory.worker.get().is_none());
    }
}
