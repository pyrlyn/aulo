//! The espeak-ng child process: start it, decode its WAV stream, reap it.
//! No shell is involved and the arguments are fixed flags plus a voice id
//! that came from espeak-ng's own listing; the text goes over stdin.

use std::io::{self, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use aulo_speech::SpeechError;
use hound::{SampleFormat, WavReader};

/// Samples handed to the sink at once, so a cancel is noticed within a few ms.
const CHUNK_SAMPLES: usize = 2048;
/// More than any real `--voices` listing.
const MAX_LISTING_BYTES: u64 = 256 * 1024;
/// How long a child that closed its output may take to exit before it is killed.
const REAP_GRACE: Duration = Duration::from_secs(1);
const REAP_POLL: Duration = Duration::from_millis(2);
const I16_SCALE: f32 = 32768.0;
const MONO: u16 = 1;
const PROBE_TEXT: &str = ".";

fn spawn_error(binary: &Path, error: &io::Error) -> SpeechError {
    let detail = format!("cannot start {}: {error}", binary.display());
    match error.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => {
            SpeechError::unavailable(&detail)
        }
        _ => SpeechError::failed(&detail),
    }
}

/// Starts `espeak-ng --stdout` for one utterance and returns its output pipe.
/// The text is at most a few KiB, which a pipe takes without blocking, so
/// writing it before reading cannot deadlock.
pub(super) fn spawn_synth(
    binary: &Path,
    voice: Option<&str>,
    words_per_minute: u32,
    text: &str,
) -> Result<(Child, ChildStdout), SpeechError> {
    let mut command = Command::new(binary);
    // -z: no pause after the last sentence, because every push is a sentence.
    command.args(["--stdout", "--stdin", "-b", "1", "-z", "-s"]);
    command.arg(words_per_minute.to_string());
    if let Some(voice) = voice {
        command.args(["-v", voice]);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| spawn_error(binary, &e))?;
    if let Some(mut stdin) = child.stdin.take() {
        // A broken pipe means the child already exited; its status says why.
        let _ = stdin.write_all(text.as_bytes());
    }
    match child.stdout.take() {
        Some(stdout) => Ok((child, stdout)),
        None => {
            reap(child, true);
            Err(SpeechError::failed("espeak-ng has no output pipe"))
        }
    }
}

/// Waits for the child so it never lingers as a zombie, and kills it if it
/// does not exit by itself soon.
pub(super) fn reap(mut child: Child, kill_first: bool) -> Option<ExitStatus> {
    if kill_first {
        // Fails only when the child already exited.
        let _ = child.kill();
    }
    let deadline = Instant::now() + REAP_GRACE;
    while let Ok(None) = child.try_wait() {
        if Instant::now() >= deadline {
            let _ = child.kill();
            break;
        }
        thread::sleep(REAP_POLL);
    }
    child.wait().ok()
}

/// The text of `espeak-ng --voices`.
pub(super) fn list_voices(binary: &Path) -> Result<String, SpeechError> {
    let mut child = Command::new(binary)
        .arg("--voices")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| spawn_error(binary, &e))?;
    let mut bytes = Vec::new();
    let read = child.stdout.take().map_or(Ok(0), |out| {
        out.take(MAX_LISTING_BYTES).read_to_end(&mut bytes)
    });
    let capped = bytes.len() as u64 >= MAX_LISTING_BYTES;
    let status = reap(child, capped);
    if read.is_err() || !status.is_some_and(|s| s.success()) {
        return Err(SpeechError::failed("espeak-ng --voices failed"));
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The sample rate espeak-ng writes, which is a property of the build and not
/// of a voice. A request has to name its audio format before any text is
/// synthesized, so it is read from the header of a throwaway run.
pub(super) fn probe_sample_rate(binary: &Path) -> Result<u32, SpeechError> {
    let (child, stdout) = spawn_synth(binary, None, 175, PROBE_TEXT)?;
    let header = open_wav(BufReader::new(stdout)).map(|wav| wav.spec().sample_rate);
    reap(child, true);
    header
}

fn damaged(what: &str) -> SpeechError {
    SpeechError::failed(&format!("espeak-ng wrote {what}"))
}

fn open_wav<R: Read>(reader: R) -> Result<WavReader<R>, SpeechError> {
    let wav = WavReader::new(reader).map_err(|_| damaged("no valid WAV header"))?;
    let spec = wav.spec();
    if spec.sample_format != SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err(damaged("audio that is not 16-bit PCM"));
    }
    if spec.channels != MONO {
        return Err(damaged("audio that is not mono"));
    }
    Ok(wav)
}

/// Decodes the child's WAV stream into `sink`, chunk by chunk, so memory use
/// is one chunk however long the utterance is. The sink returns false to stop.
/// Returns whether the stream ran to its end.
///
/// The header is checked against `expected_hz` on every utterance rather than
/// trusted from the probe: a mismatch would play at the wrong pitch.
pub(super) fn stream_samples(
    stdout: ChildStdout,
    expected_hz: u32,
    mut sink: impl FnMut(&[f32]) -> bool,
) -> Result<bool, SpeechError> {
    let mut wav = open_wav(BufReader::new(stdout))?;
    let rate = wav.spec().sample_rate;
    if rate != expected_hz {
        return Err(SpeechError::failed(&format!(
            "espeak-ng wrote {rate} Hz audio, expected {expected_hz} Hz"
        )));
    }
    let mut chunk = [0.0_f32; CHUNK_SAMPLES];
    let mut len = 0;
    for sample in wav.samples::<i16>() {
        match sample {
            Ok(value) => {
                chunk[len] = f32::from(value) / I16_SCALE;
                len += 1;
                if len == CHUNK_SAMPLES {
                    if !sink(&chunk) {
                        return Ok(false);
                    }
                    len = 0;
                }
            }
            // On a pipe the header cannot know the length, so the stream really
            // ends at end of file, which hound reports as an I/O error (a
            // short read is not `UnexpectedEof`). A child that died is told
            // apart from one that finished by its exit status.
            Err(hound::Error::IoError(_)) => break,
            Err(_) => return Err(damaged("a damaged sample stream")),
        }
    }
    Ok(len == 0 || sink(&chunk[..len]))
}
