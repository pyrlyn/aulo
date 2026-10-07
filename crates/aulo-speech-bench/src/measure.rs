//! Timing one engine on one input, and summing the runs into the numbers a
//! row reports. Engines are driven the way the pipeline drives them (push,
//! then poll), with audio fed faster than real time, so for a streaming engine
//! "first partial" is the time from `begin` to the first partial, not the
//! delay behind live speech.

use std::thread;
use std::time::{Duration, Instant};

use aulo_speech::{
    AudioFormat, AudioFrame, SpeechError, SpeechRate, SttEngine, SttPoll, Transcript,
    TranscriptKind, TtsEngine, TtsPoll, TtsRequest, TurnId,
};

use crate::clips::{CLIPS, duration_of};
use crate::wer::WordErrors;

/// 20 ms at 16 kHz, the frame size aulo-audio delivers.
const FRAME_SAMPLES: usize = 320;
const POLL_PAUSE: Duration = Duration::from_micros(500);
/// 100 ms of 48 kHz mono, a whole number of frames for mono and stereo.
const TTS_POLL_SAMPLES: usize = 4_800;
const TRANSCRIPT_CAPACITY: usize = 256;
/// Longest wait for one clip, cloud round trips included.
const RUN_LIMIT: Duration = Duration::from_secs(60);

/// A decoded clip to transcribe, or a sentence to speak.
#[derive(Debug, Clone)]
pub struct Workload {
    pub stt: Vec<SttInput>,
    pub tts: Vec<TtsInput>,
}

#[derive(Debug, Clone)]
pub struct SttInput {
    pub name: String,
    pub language: String,
    pub reference: String,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct TtsInput {
    pub language: String,
    pub text: String,
}

impl Workload {
    /// The embedded clips; TTS reads the English ones.
    pub fn builtin() -> Result<Self, String> {
        let stt = CLIPS
            .iter()
            .map(|c| {
                Ok(SttInput {
                    name: c.name.to_owned(),
                    language: c.language.to_owned(),
                    reference: c.text.to_owned(),
                    samples: c.samples()?,
                })
            })
            .collect::<Result<_, String>>()?;
        let tts = CLIPS
            .iter()
            .filter(|c| c.language == "en")
            .map(|c| TtsInput {
                language: c.language.to_owned(),
                text: c.text.to_owned(),
            })
            .collect();
        Ok(Self { stt, tts })
    }
}

/// What a row reports for one engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// STT only: word error rate over all clips.
    pub wer: Option<f64>,
    /// Processing time over audio time, summed over the clips. Below 1 is
    /// faster than real time.
    pub rtf: f64,
    /// Mean time to the first partial transcript (STT) or the first audio
    /// (TTS); `None` when the engine never produced one early, as with an
    /// offline recognizer that answers only after `finish`.
    pub first: Option<Duration>,
    pub clips: usize,
    /// Inputs in a language the engine does not handle.
    pub skipped: usize,
}

#[derive(Debug, Default)]
struct Sums {
    elapsed: Duration,
    audio: Duration,
    first: Vec<Duration>,
}

impl Sums {
    fn add(&mut self, elapsed: Duration, audio: Duration, first: Option<Duration>) {
        self.elapsed += elapsed;
        self.audio += audio;
        self.first.extend(first);
    }

    fn finish(self, wer: Option<WordErrors>, used: usize, total: usize) -> Result<Metrics, String> {
        if used == 0 {
            return Err("no input in a language this engine handles".to_owned());
        }
        let first = u32::try_from(self.first.len())
            .ok()
            .filter(|&n| n > 0)
            .map(|n| self.first.iter().sum::<Duration>() / n);
        Ok(Metrics {
            wer: wer.and_then(WordErrors::rate),
            rtf: self.elapsed.as_secs_f64() / self.audio.as_secs_f64().max(f64::EPSILON),
            first,
            clips: used,
            skipped: total - used,
        })
    }
}

/// Runs every clip through `engine`. A clip in a language the engine does not
/// handle (`Unsupported`) is skipped, not failed; any other error ends the run.
pub fn bench_stt(engine: &mut dyn SttEngine, inputs: &[SttInput]) -> Result<Metrics, String> {
    let (mut sums, mut wer, mut used) = (Sums::default(), WordErrors::default(), 0);
    for input in inputs {
        match transcribe(engine, &input.samples, &input.language) {
            Ok(run) => {
                wer += WordErrors::between(&run.text, &input.reference);
                sums.add(run.elapsed, run.audio, run.first_partial);
                used += 1;
            }
            Err(SpeechError::Unsupported(_)) => {}
            Err(e) => {
                engine.cancel();
                return Err(format!("{}: {e}", input.name));
            }
        }
    }
    sums.finish(Some(wer), used, inputs.len())
}

/// As [`bench_stt`], for speech synthesis.
pub fn bench_tts(engine: &mut dyn TtsEngine, inputs: &[TtsInput]) -> Result<Metrics, String> {
    let (mut sums, mut used) = (Sums::default(), 0);
    for input in inputs {
        match synthesize(engine, &input.text, &input.language) {
            Ok(run) => {
                sums.add(run.elapsed, run.audio, Some(run.first_audio));
                used += 1;
            }
            Err(SpeechError::Unsupported(_)) => {}
            Err(e) => {
                engine.cancel();
                return Err(e.to_string());
            }
        }
    }
    sums.finish(None, used, inputs.len())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SttRun {
    pub text: String,
    pub audio: Duration,
    /// From `begin` to the final transcript.
    pub elapsed: Duration,
    pub first_partial: Option<Duration>,
}

pub fn transcribe(
    engine: &mut dyn SttEngine,
    samples: &[f32],
    language: &str,
) -> Result<SttRun, SpeechError> {
    let turn = TurnId::new();
    let mut out = Transcript::with_capacity(turn, TRANSCRIPT_CAPACITY);
    let mut first_partial = None;
    let start = Instant::now();
    let mut observe = |out: &Transcript| {
        if out.kind == TranscriptKind::Partial && first_partial.is_none() {
            first_partial = Some(start.elapsed());
        }
    };
    engine.begin(turn, Some(language))?;
    for (i, chunk) in samples.chunks(FRAME_SAMPLES).enumerate() {
        let position = (i * FRAME_SAMPLES) as u64;
        engine.push(AudioFrame::new(chunk, AudioFormat::PIPELINE, position)?)?;
        if engine.poll(&mut out)? == SttPoll::Updated {
            observe(&out);
        }
    }
    engine.finish()?;
    loop {
        match engine.poll(&mut out)? {
            SttPoll::Updated if out.kind == TranscriptKind::Final => break,
            SttPoll::Updated => observe(&out),
            SttPoll::Done => break,
            SttPoll::Pending if start.elapsed() > RUN_LIMIT => {
                return Err(SpeechError::failed("no final transcript in time"));
            }
            SttPoll::Pending => thread::sleep(POLL_PAUSE),
        }
    }
    let elapsed = start.elapsed();
    Ok(SttRun {
        text: out.text,
        audio: duration_of(samples),
        elapsed,
        first_partial,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TtsRun {
    pub audio: Duration,
    /// From `begin` to the last sample.
    pub elapsed: Duration,
    pub first_audio: Duration,
}

pub fn synthesize(
    engine: &mut dyn TtsEngine,
    text: &str,
    language: &str,
) -> Result<TtsRun, SpeechError> {
    let request = TtsRequest {
        turn_id: TurnId::new(),
        voice: None,
        language: Some(language),
        rate: SpeechRate::NORMAL,
    };
    let start = Instant::now();
    let format = engine.begin(&request)?;
    engine.push_text(text)?;
    engine.finish()?;
    let mut buffer = vec![0.0; TTS_POLL_SAMPLES];
    let (mut samples, mut first_audio) = (0_usize, None);
    loop {
        match engine.poll(&mut buffer)? {
            TtsPoll::Audio { samples: n } => {
                first_audio.get_or_insert_with(|| start.elapsed());
                samples += n;
            }
            TtsPoll::Done => break,
            TtsPoll::Pending if start.elapsed() > RUN_LIMIT => {
                return Err(SpeechError::failed("no audio in time"));
            }
            TtsPoll::Pending => thread::sleep(POLL_PAUSE),
        }
    }
    let elapsed = start.elapsed();
    let Some(first_audio) = first_audio else {
        return Err(SpeechError::failed("the engine produced no audio"));
    };
    let frames = samples / usize::from(format.channels());
    Ok(TtsRun {
        audio: Duration::from_secs_f64(frames as f64 / f64::from(format.sample_rate_hz())),
        elapsed,
        first_audio,
    })
}
