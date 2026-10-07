//! The Speech framework side of [`super::stt::SystemStt`]: the capability
//! probe, and the worker thread that owns `SFSpeechRecognizer` and feeds it
//! audio from the ring.
//!
//! # Why `SFSpeechRecognizer` and not `SpeechAnalyzer`
//!
//! `SpeechAnalyzer` and `SpeechTranscriber` (macOS 26+) are a Swift `actor`
//! and a Swift `final class` with no `@objc` exposure, so no Objective-C
//! binding can reach them; only a Swift bridge could. Checked 2026-10-07:
//! - `Speech.framework/Modules/Speech.swiftmodule/arm64e-apple-macos.swiftinterface`
//!   in the macOS 27.0 SDK (Xcode, Swift 6.4) declares both without `@objc`,
//!   and none of the SDK's Objective-C headers mention them.
//! - objc2-speech 0.3.2, the latest on <https://crates.io/crates/objc2-speech>,
//!   generates only the `SF*` headers.
//!
//! `SFSpeechRecognizer` with `requiresOnDeviceRecognition` keeps audio on
//! the Mac (SDK header `SFSpeechRecognitionRequest.h`, macOS 10.15+).
//!
//! # Threading
//!
//! Result handlers run on the recognizer's `queue`, which defaults to the
//! main queue (SDK header `SFSpeechRecognizer.h`). Each recognizer gets a
//! private serial queue instead, so recognition does not depend on the host
//! running the main run loop.

use std::ptr::NonNull;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use aulo_speech::{PIPELINE_SAMPLE_RATE_HZ, SpeechError};
use block2::RcBlock;
use objc2::AnyThread;
use objc2::rc::{Retained, autoreleasepool};
use objc2_avf_audio::{AVAudioFormat, AVAudioPCMBuffer};
use objc2_foundation::{NSError, NSLocale, NSOperationQueue, NSString};
use objc2_speech::{
    SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult, SFSpeechRecognitionTask,
    SFSpeechRecognitionTaskHint, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
};
use ringbuf::HeapCons;
use ringbuf::traits::{Consumer, Observer};

use super::stt::{Command, MAX_TRANSCRIPT_BYTES, Outcome, Sink};

/// How often a worker with a live utterance moves audio to the recognizer.
const FEED_TICK: Duration = Duration::from_millis(10);
/// How long an idle worker sleeps; any command wakes it.
const IDLE_WAIT: Duration = Duration::from_secs(1);
/// Most samples per appended buffer, 100 ms at the pipeline rate.
const MAX_CHUNK: usize = PIPELINE_SAMPLE_RATE_HZ as usize / 10;
/// Speech reports silence as this error rather than an empty result. Seen
/// in reports of the server and on-device recognizers; **unverified** here,
/// because authorization could not be granted to test it. Any other error
/// fails over, which is safe.
const NO_SPEECH: (&str, isize) = ("kAFAssistantErrorDomain", 1110);

/// Locales the recognizer can serve without the network, as BCP 47 tags.
/// Fails with Unavailable unless access is already granted: asking would
/// show a dialog, and a process without a usage description is killed by
/// TCC instead, so only the desktop app asks.
pub(super) fn on_device_locales() -> Result<Vec<String>, SpeechError> {
    // SAFETY: a class method with no arguments; it never prompts.
    authorization(unsafe { SFSpeechRecognizer::authorizationStatus() })?;
    // SAFETY: a class method with no arguments returning an owned set.
    let supported = unsafe { SFSpeechRecognizer::supportedLocales() };
    let mut tags: Vec<String> = supported
        .iter()
        .filter(|locale| recognizer(locale).is_some())
        .map(|locale| bcp47(&locale.localeIdentifier().to_string()))
        .collect();
    tags.sort();
    let none = || SpeechError::unavailable("no on-device speech recognition model is installed");
    (!tags.is_empty()).then_some(tags).ok_or_else(none)
}

/// The user's locale as a BCP 47 tag, the language used when none is given.
pub(super) fn current_tag() -> String {
    bcp47(&NSLocale::currentLocale().localeIdentifier().to_string())
}

pub(super) fn authorization(
    status: SFSpeechRecognizerAuthorizationStatus,
) -> Result<(), SpeechError> {
    match status {
        SFSpeechRecognizerAuthorizationStatus::Authorized => Ok(()),
        SFSpeechRecognizerAuthorizationStatus::NotDetermined => Err(SpeechError::unavailable(
            "speech recognition access was never granted; the desktop app asks for it",
        )),
        _ => Err(SpeechError::unavailable(
            "speech recognition access is denied or restricted in System Settings",
        )),
    }
}

/// `en_US@rg=gbzzzz` (NSLocale) to `en-US`.
pub(super) fn bcp47(identifier: &str) -> String {
    identifier
        .split('@')
        .next()
        .unwrap_or(identifier)
        .replace('_', "-")
}

/// A recognizer for `locale` only if it runs on this Mac without a server.
fn recognizer(locale: &NSLocale) -> Option<Retained<SFSpeechRecognizer>> {
    // SAFETY: designated initializer with a live locale; nil for unsupported.
    let recognizer =
        unsafe { SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), locale) }?;
    // SAFETY: a property getter on the new recognizer.
    unsafe { recognizer.supportsOnDeviceRecognition() }.then_some(recognizer)
}

/// Samples to drop and then to feed out of `available` queued ones, given
/// how many were consumed so far. Commands carry positions in the stream of
/// accepted samples, so a cancel or a new utterance fences off old audio
/// exactly, without the engine ever touching the consumer.
pub(super) fn split(
    consumed: u64,
    available: usize,
    discard_to: u64,
    end_at: Option<u64>,
) -> (usize, usize) {
    let available = available as u64;
    let skip = available.min(discard_to.saturating_sub(consumed));
    let room = end_at.map_or(u64::MAX, |end| end.saturating_sub(consumed + skip));
    // Both are at most `available`, which came from a usize.
    (skip as usize, (available - skip).min(room) as usize)
}

pub(super) fn error_outcome(domain: &str, code: isize, detail: &str) -> Outcome {
    if (domain, code) == NO_SPEECH {
        Ok(String::new())
    } else {
        Err(SpeechError::failed(&format!("{domain} {code}: {detail}")))
    }
}

/// Untrusted framework text, trimmed and held to the transcript cap.
pub(super) fn capped(text: &str) -> String {
    let text = text.trim();
    text[..text.floor_char_boundary(MAX_TRANSCRIPT_BYTES)].to_owned()
}

struct Live {
    /// Held for the task's lifetime; the docs do not say the task retains it.
    _recognizer: Retained<SFSpeechRecognizer>,
    request: Retained<SFSpeechAudioBufferRecognitionRequest>,
    task: Retained<SFSpeechRecognitionTask>,
    end_at: Option<u64>,
    ended: bool,
}

impl Drop for Live {
    fn drop(&mut self) {
        // SAFETY: plain messages to objects this thread owns; both are
        // no-ops on a task that already completed.
        unsafe {
            self.request.endAudio();
            self.task.cancel();
        }
    }
}

/// Runs until the engine drops its sender.
pub(super) fn run(mut ring: HeapCons<f32>, commands: Receiver<Command>) {
    let queue = NSOperationQueue::new();
    queue.setMaxConcurrentOperationCount(1);
    let rate = f64::from(PIPELINE_SAMPLE_RATE_HZ);
    // SAFETY: the designated initializer with a valid rate and one channel.
    let format = unsafe {
        AVAudioFormat::initStandardFormatWithSampleRate_channels(AVAudioFormat::alloc(), rate, 1)
    };
    let Some(format) = format else { return };
    let (mut live, mut consumed, mut discard_to) = (None::<Live>, 0_u64, 0_u64);
    loop {
        let feeding = live.as_ref().is_some_and(|l| !l.ended);
        let wait = if feeding { FEED_TICK } else { IDLE_WAIT };
        let first = match commands.recv_timeout(wait) {
            Ok(command) => Some(command),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        // Taken before the remaining commands are read: the engine sends a
        // command before pushing the audio that follows it, so every sample
        // counted here is already covered by a command seen below.
        let available = ring.occupied_len();
        autoreleasepool(|_| {
            for command in first.into_iter().chain(commands.try_iter()) {
                match command {
                    Command::Begin(locale, start_at, sink) => {
                        // The old task is cancelled before the new one starts.
                        drop(live.take());
                        discard_to = start_at;
                        live = start(&locale, &queue, sink);
                    }
                    Command::Finish(end_at) => {
                        live.iter_mut().for_each(|l| l.end_at = Some(end_at))
                    }
                    Command::Cancel(to) => (live, discard_to) = (None, to),
                }
            }
        });
        let end_at = live.as_ref().map_or(Some(consumed), |l| l.end_at);
        let (skip, feed) = split(consumed, available, discard_to, end_at);
        consumed += ring.skip(skip) as u64;
        if let Some(live) = live.as_mut() {
            autoreleasepool(|_| consumed += feed_audio(&mut ring, &live.request, &format, feed));
            if !live.ended && live.end_at == Some(consumed) {
                // SAFETY: a plain message to the request this thread owns.
                unsafe { live.request.endAudio() };
                live.ended = true;
            }
        }
    }
}

/// Moves `count` samples into buffers the request takes ownership of, and
/// returns how many were moved.
fn feed_audio(
    ring: &mut HeapCons<f32>,
    request: &SFSpeechAudioBufferRecognitionRequest,
    format: &AVAudioFormat,
    count: usize,
) -> u64 {
    let mut moved = 0;
    while moved < count {
        let chunk = (count - moved).min(MAX_CHUNK);
        // SAFETY: designated initializer; the chunk fits u32 by MAX_CHUNK.
        let buffer = unsafe {
            AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(
                AVAudioPCMBuffer::alloc(),
                format,
                chunk as u32,
            )
        };
        let Some(buffer) = buffer else { break };
        // SAFETY: a standard format is deinterleaved float32, so the first
        // plane holds `chunk` writable samples for the buffer's lifetime.
        let popped = unsafe {
            let Some(plane) = NonNull::new(buffer.floatChannelData()) else {
                break;
            };
            let samples = std::slice::from_raw_parts_mut(plane.read().as_ptr(), chunk);
            ring.pop_slice(samples)
        };
        // SAFETY: setter and append on live objects; `popped <= chunk`.
        unsafe {
            buffer.setFrameLength(popped as u32);
            request.appendAudioPCMBuffer(&buffer);
        }
        moved += popped;
        if popped < chunk {
            break;
        }
    }
    moved as u64
}

/// Starts recognition for `locale`. A failure goes to the sink, which the
/// engine reports on its next poll.
fn start(locale: &str, queue: &NSOperationQueue, sink: Sink) -> Option<Live> {
    // SAFETY: a class method with no arguments; access can be revoked while
    // the daemon runs, so it is checked again per utterance.
    if let Err(error) = authorization(unsafe { SFSpeechRecognizer::authorizationStatus() }) {
        let _ = sink.outcome.try_send(Err(error));
        return None;
    }
    let ns_locale = NSLocale::localeWithLocaleIdentifier(&NSString::from_str(locale));
    let Some(recognizer) = recognizer(&ns_locale) else {
        let _ = sink.outcome.try_send(Err(SpeechError::unsupported(locale)));
        return None;
    };
    let handler = RcBlock::new(
        move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
            // SAFETY: each pointer is nil or live for the call.
            on_result(&sink, unsafe { result.as_ref() }, unsafe { error.as_ref() });
        },
    );
    // SAFETY: setters on the new recognizer and request, then starting the
    // task; the framework copies the handler block.
    unsafe {
        recognizer.setQueue(queue);
        let request = SFSpeechAudioBufferRecognitionRequest::new();
        request.setRequiresOnDeviceRecognition(true);
        request.setShouldReportPartialResults(true);
        request.setAddsPunctuation(true);
        request.setTaskHint(SFSpeechRecognitionTaskHint::Dictation);
        let task = recognizer.recognitionTaskWithRequest_resultHandler(&request, &handler);
        Some(Live {
            _recognizer: recognizer,
            request,
            task,
            end_at: None,
            ended: false,
        })
    }
}

/// Runs on the recognizer's queue. Never waits: a full partial queue drops
/// the partial, which the next one supersedes, and the outcome slot is sized
/// for the single outcome a task delivers.
fn on_result(sink: &Sink, result: Option<&SFSpeechRecognitionResult>, error: Option<&NSError>) {
    if let Some(result) = result {
        // SAFETY: property getters on a live result.
        let (text, last) = unsafe {
            (
                result.bestTranscription().formattedString(),
                result.isFinal(),
            )
        };
        let text = capped(&text.to_string());
        if last {
            let _ = sink.outcome.try_send(Ok(text));
        } else {
            let _ = sink.partials.try_send(text);
        }
    } else if let Some(error) = error {
        let detail = error.localizedDescription().to_string();
        let _ = sink.outcome.try_send(error_outcome(
            &error.domain().to_string(),
            error.code(),
            &detail,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_granted_access_is_available() {
        assert!(authorization(SFSpeechRecognizerAuthorizationStatus::Authorized).is_ok());
        for status in [
            SFSpeechRecognizerAuthorizationStatus::NotDetermined,
            SFSpeechRecognizerAuthorizationStatus::Denied,
            SFSpeechRecognizerAuthorizationStatus::Restricted,
        ] {
            assert!(matches!(
                authorization(status),
                Err(SpeechError::Unavailable(_))
            ));
        }
    }

    #[test]
    fn locale_identifiers_become_bcp47_tags() {
        assert_eq!(bcp47("en_US"), "en-US");
        assert_eq!(bcp47("en_GB@rg=uszzzz"), "en-GB");
        assert_eq!(bcp47("ru-RU"), "ru-RU");
    }

    #[test]
    fn fences_drop_old_audio_and_stop_at_the_end() {
        // A cancel at 100 with 150 queued: 100 dropped, 50 for the next utterance.
        assert_eq!(split(0, 150, 100, None), (100, 50));
        // Finished at 120: audio of the next utterance stays queued.
        assert_eq!(split(100, 50, 100, Some(120)), (0, 20));
        assert_eq!(split(120, 30, 100, Some(120)), (0, 0));
        // Nothing queued yet past the fence.
        assert_eq!(split(0, 40, 100, None), (40, 0));
    }

    #[test]
    fn silence_is_an_empty_final_and_other_errors_fail_over() {
        assert_eq!(
            error_outcome(NO_SPEECH.0, NO_SPEECH.1, "No speech detected"),
            Ok(String::new())
        );
        let failed = error_outcome("kLSRErrorDomain", 102, "assets\nmissing").unwrap_err();
        assert!(failed.should_fall_back());
        assert!(!failed.to_string().contains('\n'));
    }

    #[test]
    fn framework_text_is_trimmed_and_capped_on_a_char_boundary() {
        assert_eq!(capped("  open the door \n"), "open the door");
        let long = capped(&"я".repeat(MAX_TRANSCRIPT_BYTES));
        assert!(long.len() <= MAX_TRANSCRIPT_BYTES);
        assert!(long.chars().all(|c| c == 'я'));
    }

    /// Passes with or without access: whatever the probe says, it says it
    /// without prompting and in the engine's terms.
    #[test]
    fn the_probe_answers_without_prompting() {
        match on_device_locales() {
            Ok(tags) => assert!(tags.iter().all(|t| !t.is_empty() && !t.contains('_'))),
            Err(error) => assert!(matches!(error, SpeechError::Unavailable(_)), "{error}"),
        }
        assert!(!current_tag().contains('_'));
    }
}
