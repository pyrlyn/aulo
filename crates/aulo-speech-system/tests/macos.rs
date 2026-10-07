//! Synthesis through the real `AVSpeechSynthesizer`. AVFoundation delivers
//! buffers on the main dispatch queue, and libtest parks the main thread
//! while tests run on workers, so this binary (`harness = false`) runs the
//! checks on a worker and pumps the main run loop itself, the way a host
//! must.
//!
//! Set `AULO_SYSTEM_TTS_WAV=/path/out.wav` to also save the synthesized
//! phrase for listening; nothing is ever played through the speakers.
//!
//! Set `AULO_SYSTEM_STT=1` to also transcribe the sherpa fixture WAVs with
//! the on-device recognizer. It needs speech recognition access, which only
//! an app bundle with a usage description can ask for, so it is off by
//! default and fails, rather than skips, when asked for without access.

// Test code may unwrap and assert (the workspace rule), but without libtest's
// `#[test]` clippy cannot tell this binary is a test, so it is said here.
#![allow(clippy::unwrap_used)]

#[cfg(target_os = "macos")]
fn main() {
    macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    use aulo_speech::{
        AudioFormat, AudioFrame, SpeechRate, SttEngine, SttPoll, Transcript, TranscriptKind,
        TtsEngine, TtsPoll, TtsRequest, TurnId,
    };
    use aulo_speech_system::{SystemStt, SystemTts};
    use objc2_foundation::{NSDate, NSRunLoop};

    const TIMEOUT: Duration = Duration::from_secs(20);
    const PUMP_SECONDS: f64 = 0.02;
    const IDLE: Duration = Duration::from_millis(5);
    const POLL_SAMPLES: usize = 1_024;
    const FIRST: &str = "Hello from aulo.";
    const SECOND: &str = "This sentence comes from a system voice.";
    const LONG: &str = "This is a much longer sentence that is cancelled before it can finish, \
                        so none of its audio may leak into the next reply.";
    const AFTER_CANCEL: &str = "Done.";
    /// Quieter than this is silence, not speech.
    const MIN_PEAK: f32 = 0.05;
    /// "Done." takes well under this; the cancelled sentence takes far longer.
    const MAX_AFTER_CANCEL_SECONDS: f32 = 2.0;
    const WAV_ENV: &str = "AULO_SYSTEM_TTS_WAV";
    const STT_ENV: &str = "AULO_SYSTEM_STT";
    const FIXTURE_DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../aulo-speech-sherpa/tests/fixtures"
    );
    const FRAME: usize = 320;
    /// File, language hint, what was said (macOS `say`, 16 kHz mono 16-bit).
    const FIXTURES: &[(&str, &str, &str)] = &[
        (
            "en_browser.wav",
            "en-US",
            "Open the browser and play some music.",
        ),
        (
            "en_weather.wav",
            "en-US",
            "What is the weather like in London tomorrow?",
        ),
        (
            "en_email.wav",
            "en-US",
            "Please read my latest email out loud.",
        ),
        ("ru_browser.wav", "ru-RU", "Открой браузер и включи музыку."),
        ("ru_weather.wav", "ru-RU", "Какая завтра погода в Москве?"),
    ];

    pub fn main() {
        let done = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&done);
        let checks = thread::spawn(move || {
            run("voices_are_listed", voices_are_listed);
            run("phrase_yields_speech", phrase_yields_speech);
            run("cancel_ends_the_reply", cancel_ends_the_reply);
            if std::env::var_os(STT_ENV).is_some() {
                run("fixtures_are_transcribed", fixtures_are_transcribed);
            } else {
                println!("test fixtures_are_transcribed ... skipped (set {STT_ENV}=1)");
            }
            flag.store(true, Ordering::SeqCst);
        });
        while !done.load(Ordering::SeqCst) && !checks.is_finished() {
            let until = NSDate::dateWithTimeIntervalSinceNow(PUMP_SECONDS);
            NSRunLoop::currentRunLoop().runUntilDate(&until);
        }
        if checks.join().is_err() {
            std::process::exit(1);
        }
    }

    fn run(name: &str, check: fn()) {
        check();
        println!("test {name} ... ok");
    }

    fn request(language: &str) -> TtsRequest<'_> {
        TtsRequest {
            turn_id: TurnId::new(),
            voice: None,
            language: Some(language),
            rate: SpeechRate::NORMAL,
        }
    }

    /// Polls until Done, returning every sample.
    fn drain(engine: &mut SystemTts) -> Vec<f32> {
        let (mut all, mut out) = (Vec::new(), [0.0; POLL_SAMPLES]);
        let started = Instant::now();
        loop {
            match engine.poll(&mut out).unwrap() {
                TtsPoll::Audio { samples } => all.extend_from_slice(&out[..samples]),
                TtsPoll::Done => return all,
                TtsPoll::Pending => thread::sleep(IDLE),
            }
            assert!(started.elapsed() < TIMEOUT, "no Done within {TIMEOUT:?}");
        }
    }

    fn voices_are_listed() {
        let engine = SystemTts::new(None).unwrap();
        assert!(!engine.voices().is_empty());
        assert!(engine.voices().iter().any(|v| v.language.is_some()));
    }

    fn phrase_yields_speech() {
        let mut engine = SystemTts::new(None).unwrap();
        let format = engine.begin(&request("en-US")).unwrap();
        engine.push_text(FIRST).unwrap();
        engine.push_text(SECOND).unwrap();
        engine.finish().unwrap();
        let samples = drain(&mut engine);
        let seconds = samples.len() as f32 / format.sample_rate_hz() as f32;
        let peak = samples.iter().fold(0.0_f32, |p, s| p.max(s.abs()));
        println!(
            "  {} samples at {} Hz: {seconds:.2} s, peak {peak:.3}, dropped {}",
            samples.len(),
            format.sample_rate_hz(),
            engine.dropped_samples()
        );
        assert!(seconds > 1.0, "two sentences gave {seconds} s");
        assert!(peak > MIN_PEAK && peak <= 1.0, "peak {peak}");
        assert_eq!(engine.dropped_samples(), 0);
        if let Some(path) = std::env::var_os(WAV_ENV) {
            std::fs::write(&path, wav(&samples, format)).unwrap();
            println!("  wrote {}", path.to_string_lossy());
        }
    }

    fn cancel_ends_the_reply() {
        let mut engine = SystemTts::new(None).unwrap();
        engine.begin(&request("en-US")).unwrap();
        engine.push_text(LONG).unwrap();
        let mut out = [0.0; POLL_SAMPLES];
        let started = Instant::now();
        while !matches!(engine.poll(&mut out).unwrap(), TtsPoll::Audio { .. }) {
            assert!(started.elapsed() < TIMEOUT, "no audio within {TIMEOUT:?}");
            thread::sleep(IDLE);
        }
        engine.cancel();
        assert_eq!(engine.poll(&mut out).unwrap(), TtsPoll::Done);

        let format = engine.begin(&request("en-US")).unwrap();
        engine.push_text(AFTER_CANCEL).unwrap();
        engine.finish().unwrap();
        let samples = drain(&mut engine);
        let seconds = samples.len() as f32 / format.sample_rate_hz() as f32;
        assert!(seconds > 0.0, "the reply after a cancel is silent");
        assert!(
            seconds < MAX_AFTER_CANCEL_SECONDS,
            "{seconds} s: audio of the cancelled reply leaked"
        );
    }

    fn fixtures_are_transcribed() {
        let mut stt = SystemStt::new().unwrap();
        for (file, language, said) in FIXTURES {
            if !stt.info().capabilities.languages.supports(language) {
                println!("  {file}: no on-device model for {language}");
                continue;
            }
            let reader = hound::WavReader::open(format!("{FIXTURE_DIR}/{file}")).unwrap();
            let samples: Vec<f32> = reader
                .into_samples::<i16>()
                .map(|s| f32::from(s.unwrap()) / 32_768.0)
                .collect();
            let turn = TurnId::new();
            let mut out = Transcript::with_capacity(turn, 256);
            stt.begin(turn, Some(language)).unwrap();
            for (i, chunk) in samples.chunks(FRAME).enumerate() {
                let at = (i * FRAME) as u64;
                stt.push(AudioFrame::new(chunk, AudioFormat::PIPELINE, at).unwrap())
                    .unwrap();
            }
            stt.finish().unwrap();
            let started = Instant::now();
            while out.kind != TranscriptKind::Final {
                if stt.poll(&mut out).unwrap() == SttPoll::Pending {
                    thread::sleep(IDLE);
                }
                assert!(started.elapsed() < TIMEOUT, "no final within {TIMEOUT:?}");
            }
            println!("  {file}: said {said:?}, heard {:?}", out.text);
            assert!(!out.text.is_empty(), "{file} gave an empty transcript");
            assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Done);
        }
        assert_eq!(stt.dropped_samples(), 0);
    }

    /// 16-bit PCM WAV, enough for a listening check.
    fn wav(samples: &[f32], format: AudioFormat) -> Vec<u8> {
        const HEADER_BYTES: u32 = 44;
        const FMT_CHUNK_BYTES: u32 = 16;
        const PCM: u16 = 1;
        const BITS: u16 = 16;
        let channels = u16::from(format.channels());
        let rate = format.sample_rate_hz();
        let block = channels * BITS / 8;
        let data = (samples.len() * 2) as u32;
        let mut out = Vec::with_capacity((HEADER_BYTES + data) as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(HEADER_BYTES - 8 + data).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&FMT_CHUNK_BYTES.to_le_bytes());
        out.extend_from_slice(&PCM.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
        out.extend_from_slice(&block.to_le_bytes());
        out.extend_from_slice(&BITS.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data.to_le_bytes());
        for s in samples {
            let pcm = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
            out.extend_from_slice(&pcm.to_le_bytes());
        }
        out
    }
}
