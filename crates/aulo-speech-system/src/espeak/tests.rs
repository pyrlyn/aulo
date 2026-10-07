//! Tests against a fake `espeak-ng`: a shell script in a temp directory that
//! the engine is pointed at, so the real `PATH` is never touched.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use aulo_speech::{EngineSpec, SpeechError, SpeechRate, TtsEngine, TtsPoll, TtsRequest, TurnId};
use tempfile::TempDir;

use super::engine::{ENGINE_ID, EspeakTts, factory_with};

const LISTING: &str = "\
Pty Language       Age/Gender VoiceName          File                 Other Languages
 5  en-gb           --/M      English_(Great_Britain) gmw/en-GB        (en 2)
 2  en-us           --/M      English_(America)  gmw/en-US            (en 3)
 5  ru              --/M      Russian            zle/ru
";
const COPY_WAV: &str = r#"cat "$dir/out.wav""#;
const SAMPLES: [i16; 4] = [0, 16384, -16384, 32767];
const DEADLINE: Duration = Duration::from_secs(10);

/// What `espeak-ng --stdout` writes to a pipe: a header whose lengths are
/// placeholders, because a pipe cannot be seeked back to fill them in.
fn wav(rate: u32, samples: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend(0x7fff_f024_u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(rate.to_le_bytes());
    bytes.extend((rate * 2).to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(0x7fff_f000_u32.to_le_bytes());
    for sample in samples {
        bytes.extend(sample.to_le_bytes());
    }
    bytes
}

struct Fake {
    dir: TempDir,
}

impl Fake {
    fn new(rate: u32, samples: &[i16]) -> Self {
        Self::with_tail(rate, samples, COPY_WAV)
    }

    /// `tail` is the shell that runs for a synthesis request, after the
    /// arguments and the text were recorded.
    fn with_tail(rate: u32, samples: &[i16], tail: &str) -> Self {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("voices.txt"), LISTING).unwrap();
        fs::write(dir.path().join("out.wav"), wav(rate, samples)).unwrap();
        let script = format!(
            "#!/bin/sh\ndir=$(dirname \"$0\")\n\
             if [ \"$1\" = --voices ]; then cat \"$dir/voices.txt\"; exit 0; fi\n\
             printf '%s\\n' \"$@\" > \"$dir/args.txt\"\n\
             cat > \"$dir/stdin.txt\"\n{tail}\n"
        );
        let path = dir.path().join("espeak-ng");
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn engine(&self) -> EspeakTts {
        EspeakTts::new(self.path("espeak-ng"), None).unwrap()
    }

    fn args(&self) -> Vec<String> {
        let text = fs::read_to_string(self.path("args.txt")).unwrap();
        text.lines().map(str::to_owned).collect()
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

fn drain(engine: &mut EspeakTts) -> Result<Vec<f32>, SpeechError> {
    let started = Instant::now();
    let mut all = Vec::new();
    let mut out = [0.0; 64];
    loop {
        match engine.poll(&mut out)? {
            TtsPoll::Audio { samples } => all.extend_from_slice(&out[..samples]),
            TtsPoll::Pending => thread::sleep(Duration::from_millis(2)),
            TtsPoll::Done => return Ok(all),
        }
        assert!(started.elapsed() < DEADLINE, "the reply never finished");
    }
}

fn speak(engine: &mut EspeakTts, text: &str) -> Result<Vec<f32>, SpeechError> {
    engine.begin(&request(None, None))?;
    engine.push_text(text)?;
    engine.finish()?;
    drain(engine)
}

fn alive(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .output()
        .unwrap()
        .status
        .success()
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let started = Instant::now();
    while !done() {
        assert!(started.elapsed() < DEADLINE, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn voices_and_languages_come_from_the_listing() {
    let engine = Fake::new(22_050, &SAMPLES).engine();
    let ids: Vec<_> = engine.voices().iter().map(|v| v.id.as_str()).collect();
    assert_eq!(ids, ["gmw/en-GB", "gmw/en-US", "zle/ru"]);
    assert!(engine.info().capabilities.languages.supports("ru-RU"));
    assert!(!engine.info().capabilities.languages.supports("uk"));
    assert!(engine.info().capabilities.offline);
}

#[test]
fn rate_and_voice_become_flags_and_text_goes_over_stdin() {
    let fake = Fake::new(22_050, &SAMPLES);
    let mut engine = fake.engine();
    let hostile = "--version -v evil $(rm -rf /); `id`";
    let rate = SpeechRate::new(1.5).unwrap();
    engine
        .begin(&TtsRequest {
            rate,
            ..request(Some("gmw/en-US"), None)
        })
        .unwrap();
    engine.push_text(hostile).unwrap();
    engine.finish().unwrap();
    drain(&mut engine).unwrap();
    let expected = [
        "--stdout",
        "--stdin",
        "-b",
        "1",
        "-z",
        "-s",
        "263",
        "-v",
        "gmw/en-US",
    ];
    assert_eq!(fake.args(), expected);
    assert_eq!(fs::read_to_string(fake.path("stdin.txt")).unwrap(), hostile);
}

#[test]
fn a_language_picks_a_voice() {
    let fake = Fake::new(22_050, &SAMPLES);
    let mut engine = fake.engine();
    engine.begin(&request(None, Some("ru-RU"))).unwrap();
    engine.push_text("Привет").unwrap();
    engine.finish().unwrap();
    drain(&mut engine).unwrap();
    assert_eq!(fake.args()[7..], ["-v", "zle/ru"]);
    assert_eq!(
        fs::read_to_string(fake.path("stdin.txt")).unwrap(),
        "Привет"
    );
}

#[test]
fn unknown_voices_and_languages_are_unsupported_so_the_registry_falls_back() {
    let fake = Fake::new(22_050, &SAMPLES);
    let mut engine = fake.engine();
    for (voice, language) in [(Some("no/such"), None), (None, Some("tlh"))] {
        let error = engine.begin(&request(voice, language)).unwrap_err();
        assert!(matches!(error, SpeechError::Unsupported(_)));
        assert!(error.should_fall_back());
    }
    let mut engine = EspeakTts::new(fake.path("espeak-ng"), Some("no/such".into())).unwrap();
    assert!(engine.begin(&request(None, None)).is_err());
}

#[test]
fn audio_comes_at_the_rate_in_the_header() {
    // Not espeak-ng's usual 22.05 kHz, to show the rate is read, not assumed.
    let fake = Fake::new(16_000, &SAMPLES);
    let mut engine = fake.engine();
    let format = engine.begin(&request(None, None)).unwrap();
    assert_eq!((format.sample_rate_hz(), format.channels()), (16_000, 1));
    engine.push_text("Hello.").unwrap();
    engine.finish().unwrap();
    let audio = drain(&mut engine).unwrap();
    assert_eq!(audio, [0.0, 0.5, -0.5, 32767.0 / 32768.0]);
    assert_eq!(engine.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
}

#[test]
fn a_rate_that_changes_after_the_probe_is_a_failure() {
    let fake = Fake::new(22_050, &SAMPLES);
    let mut engine = fake.engine();
    fs::write(fake.path("out.wav"), wav(44_100, &SAMPLES)).unwrap();
    let error = speak(&mut engine, "Hello.").unwrap_err();
    assert!(matches!(error, SpeechError::Failed(_)));
    assert!(error.should_fall_back());
    assert_eq!(engine.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
}

#[test]
fn a_bad_wav_header_is_failed() {
    let fake = Fake::new(22_050, &SAMPLES);
    let mut engine = fake.engine();
    fs::write(fake.path("out.wav"), b"this is not a wav file at all").unwrap();
    let error = speak(&mut engine, "Hello.").unwrap_err();
    assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");

    // The same at start-up, where the probe reads the header.
    let error = EspeakTts::new(fake.path("espeak-ng"), None).unwrap_err();
    assert!(matches!(error, SpeechError::Failed(_)), "{error:?}");
}

#[test]
fn audio_that_is_not_mono_16_bit_pcm_is_failed() {
    let fake = Fake::new(22_050, &SAMPLES);
    let mut engine = fake.engine();
    let mut stereo = wav(22_050, &SAMPLES);
    stereo[22] = 2;
    fs::write(fake.path("out.wav"), stereo).unwrap();
    assert!(matches!(
        speak(&mut engine, "Hello."),
        Err(SpeechError::Failed(_))
    ));
}

#[test]
fn a_crash_after_audio_is_failed_but_the_audio_is_still_delivered() {
    let fake = Fake::with_tail(22_050, &SAMPLES, &format!("{COPY_WAV}\nexit 3"));
    let mut engine = fake.engine();
    engine.begin(&request(None, None)).unwrap();
    engine.push_text("Hello.").unwrap();
    engine.finish().unwrap();
    let mut out = [0.0; 64];
    let started = Instant::now();
    let mut heard = 0;
    let error = loop {
        match engine.poll(&mut out) {
            Ok(TtsPoll::Audio { samples }) => heard += samples,
            Ok(TtsPoll::Pending) => thread::sleep(Duration::from_millis(2)),
            Ok(TtsPoll::Done) => panic!("a crashed run finished cleanly"),
            Err(error) => break error,
        }
        assert!(started.elapsed() < DEADLINE);
    };
    assert_eq!(heard, SAMPLES.len());
    assert!(matches!(error, SpeechError::Failed(_)));
}

#[test]
fn text_with_nothing_to_say_is_not_an_error() {
    let fake = Fake::new(22_050, &SAMPLES);
    let mut engine = fake.engine();
    fs::write(fake.path("out.wav"), wav(22_050, &[])).unwrap();
    assert!(speak(&mut engine, "...").unwrap().is_empty());
}

#[test]
fn a_missing_binary_is_unavailable_so_the_registry_falls_back() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("espeak-ng");
    let error = EspeakTts::new(&missing, None).unwrap_err();
    assert!(matches!(error, SpeechError::Unavailable(_)), "{error:?}");
    assert!(error.should_fall_back());

    let spec = EngineSpec {
        engine: aulo_speech::EngineId::new(ENGINE_ID).unwrap(),
        model: None,
        voice: None,
        rate: SpeechRate::NORMAL,
    };
    let error = factory_with(missing)(&spec).err().unwrap();
    assert!(matches!(error, SpeechError::Unavailable(_)));
}

#[test]
fn a_configured_path_builds_through_the_factory() {
    let fake = Fake::new(22_050, &SAMPLES);
    let spec = EngineSpec {
        engine: aulo_speech::EngineId::new(ENGINE_ID).unwrap(),
        model: Some("/bin/false".into()),
        voice: None,
        rate: SpeechRate::NORMAL,
    };
    let engine = factory_with(fake.path("espeak-ng"))(&spec).unwrap();
    assert_eq!(engine.voices().len(), 3);
}

#[test]
fn cancel_kills_the_child_and_reaps_it() {
    let tail = r#"echo $$ > "$dir/pid"
head -c 6044 "$dir/out.wav"
exec sleep 30"#;
    let fake = Fake::with_tail(22_050, &[1000; 3000], tail);
    let mut engine = fake.engine();
    engine.begin(&request(None, None)).unwrap();
    engine.push_text("A long sentence.").unwrap();
    let mut out = [0.0; 64];
    wait_until("first audio", || {
        matches!(engine.poll(&mut out), Ok(TtsPoll::Audio { .. }))
    });
    let pid = fs::read_to_string(fake.path("pid"))
        .unwrap()
        .trim()
        .to_owned();
    assert!(alive(&pid));

    engine.cancel();
    assert_eq!(engine.poll(&mut out).unwrap(), TtsPoll::Done);
    // A killed child that nobody waited for stays a zombie, which `kill -0`
    // still finds; it is gone only once reaped.
    wait_until("the child to be reaped", || !alive(&pid));
}

#[test]
fn dropping_the_engine_kills_the_child_too() {
    let tail = r#"echo $$ > "$dir/pid"
head -c 200 "$dir/out.wav"
exec sleep 30"#;
    let fake = Fake::with_tail(22_050, &[1000; 1000], tail);
    let mut engine = fake.engine();
    engine.begin(&request(None, None)).unwrap();
    engine.push_text("Hello.").unwrap();
    wait_until("the child to start", || fake.path("pid").exists());
    let pid = fs::read_to_string(fake.path("pid"))
        .unwrap()
        .trim()
        .to_owned();
    drop(engine);
    wait_until("the child to be reaped", || !alive(&pid));
}

#[test]
fn text_and_queue_caps_are_enforced() {
    let tail = "cat \"$dir/out.wav\"\nexec sleep 30";
    let fake = Fake::with_tail(22_050, &SAMPLES, tail);
    let mut engine = fake.engine();
    assert!(matches!(
        engine.push_text("hi"),
        Err(SpeechError::OutOfOrder(_))
    ));
    assert_eq!(engine.poll(&mut [0.0; 8]).unwrap(), TtsPoll::Done);
    engine.begin(&request(None, None)).unwrap();
    let long = "a".repeat(4 * 1024 + 1);
    assert!(matches!(
        engine.push_text(&long),
        Err(SpeechError::Invalid { .. })
    ));
    engine.push_text("   ").unwrap();
    // The first sentence never ends, so the rest queue up to the cap.
    for _ in 0..64 {
        engine.push_text("Sentence.").unwrap();
    }
    assert!(matches!(
        engine.push_text("One too many."),
        Err(SpeechError::Overflow { .. })
    ));
    engine.cancel();
    assert!(engine.push_text("late").is_err());
}

#[test]
fn a_stalled_consumer_drops_and_counts_instead_of_growing() {
    let samples = vec![500_i16; 5000];
    let fake = Fake::new(22_050, &samples);
    let mut engine =
        EspeakTts::with_limits(fake.path("espeak-ng"), None, 100, Duration::from_millis(50))
            .unwrap();
    engine.begin(&request(None, None)).unwrap();
    engine.push_text("Hello.").unwrap();
    engine.finish().unwrap();
    wait_until("the drop", || engine.dropped_samples() > 0);
    // The ring held 100 of the first 2048-sample chunk; the rest was dropped.
    assert_eq!(engine.dropped_samples(), 1948);
    assert_eq!(drain(&mut engine).unwrap().len(), 100);
}

#[test]
fn the_text_of_a_cancelled_reply_never_reaches_the_next_one() {
    let fake = Fake::new(22_050, &SAMPLES);
    let mut engine = fake.engine();
    engine.begin(&request(None, None)).unwrap();
    engine.push_text("First reply.").unwrap();
    engine.begin(&request(None, None)).unwrap();
    engine.push_text("Second reply.").unwrap();
    engine.finish().unwrap();
    let audio = drain(&mut engine).unwrap();
    assert_eq!(audio.len(), SAMPLES.len());
    assert_eq!(
        fs::read_to_string(fake.path("stdin.txt")).unwrap(),
        "Second reply."
    );
}
