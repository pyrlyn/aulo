//! Contract tests: one fake engine per trait, driven only through `Box<dyn …>`
//! from outside the crate. They prove the traits are object safe and `Send`,
//! and pin the push/poll protocol the real engines and the pipeline rely on.

// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(clippy::unwrap_used)]

use aulo_speech::{
    AudioFormat, AudioFrame, EngineId, EngineInfo, EngineKind, KeywordHit, KeywordSpotter,
    SpeechError, SpeechRate, SttEngine, SttPoll, Transcript, TranscriptKind, TtsEngine, TtsPoll,
    TtsRequest, TurnDecision, TurnDetector, TurnId, Vad, VadEvent,
};

use aulo_speech::testkit::{
    FRAME_SAMPLES, FakeStt, FakeTts, STT_CAPACITY_SAMPLES, TTS_RATE_HZ, TTS_SAMPLES_PER_BYTE, info,
};

const LOUD: f32 = 0.5;
const SPEECH_THRESHOLD: f32 = 0.1;
const SILENCE_FOR_END_OF_TURN: u64 = 16_000 * 700 / 1_000;
const TEXT_CAPACITY: usize = 64;

fn id(id: &str) -> EngineId {
    EngineId::new(id).unwrap()
}

fn frame(samples: &[f32], index: u64) -> AudioFrame<'_> {
    AudioFrame::new(samples, AudioFormat::PIPELINE, index * FRAME_SAMPLES as u64).unwrap()
}

/// Peak-energy VAD.
struct FakeVad {
    info: EngineInfo,
    in_speech: bool,
}

impl Vad for FakeVad {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn push(&mut self, frame: AudioFrame<'_>) -> Result<Option<VadEvent>, SpeechError> {
        let loud = frame.samples().iter().any(|s| s.abs() > SPEECH_THRESHOLD);
        let position = frame.position();
        let event = match (self.in_speech, loud) {
            (false, true) => Some(VadEvent::SpeechStarted { position }),
            (true, false) => Some(VadEvent::SpeechEnded { position }),
            _ => None,
        };
        self.in_speech = loud;
        Ok(event)
    }

    fn reset(&mut self) {
        self.in_speech = false;
    }
}

/// Hears the first keyword in any loud frame.
struct FakeKws {
    info: EngineInfo,
    keywords: usize,
}

impl KeywordSpotter for FakeKws {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn set_keywords(&mut self, phrases: &[&str]) -> Result<(), SpeechError> {
        self.keywords = phrases.len();
        Ok(())
    }

    fn push(&mut self, frame: AudioFrame<'_>) -> Result<Option<KeywordHit>, SpeechError> {
        let loud = frame.samples().iter().any(|s| s.abs() > SPEECH_THRESHOLD);
        Ok((loud && self.keywords > 0).then(|| KeywordHit {
            keyword: 0,
            position: frame.end_position(),
        }))
    }

    fn reset(&mut self) {}
}

/// Ends the turn after a fixed silence that follows speech.
struct SilenceTurn {
    info: EngineInfo,
    heard_speech: bool,
    silent: u64,
}

impl TurnDetector for SilenceTurn {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn push(
        &mut self,
        frame: AudioFrame<'_>,
        in_speech: bool,
    ) -> Result<TurnDecision, SpeechError> {
        if in_speech {
            self.heard_speech = true;
            self.silent = 0;
            return Ok(TurnDecision::Continue);
        }
        self.silent += frame.len() as u64;
        if self.heard_speech && self.silent >= SILENCE_FOR_END_OF_TURN {
            return Ok(TurnDecision::EndOfTurn {
                position: frame.end_position(),
            });
        }
        Ok(TurnDecision::Continue)
    }

    fn reset(&mut self) {
        self.heard_speech = false;
        self.silent = 0;
    }
}

/// What the registry does: the first engine that starts wins, engine faults
/// are noted and skipped, anything else stops the search.
fn begin_with_fallback(
    engines: &mut [Box<dyn SttEngine>],
    turn: TurnId,
    notices: &mut Vec<String>,
) -> Result<usize, SpeechError> {
    let mut last = SpeechError::unavailable("no stt engine configured");
    for (index, engine) in engines.iter_mut().enumerate() {
        match engine.begin(turn, Some("en")) {
            Ok(()) => return Ok(index),
            Err(error) if error.should_fall_back() => {
                notices.push(format!("{}: {error}", engine.info().id));
                last = error;
            }
            Err(error) => return Err(error),
        }
    }
    Err(last)
}

#[test]
fn stt_streams_partials_then_exactly_one_final() {
    let mut stt: Box<dyn SttEngine> = Box::new(FakeStt::new(id("fake-stt")));
    let turn = TurnId::new();
    let mut out = Transcript::with_capacity(turn, TEXT_CAPACITY);
    let loud = [LOUD; FRAME_SAMPLES];
    stt.begin(turn, None).unwrap();
    for index in 0..3 {
        stt.push(frame(&loud, index)).unwrap();
        assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Updated);
        assert_eq!(out.kind, TranscriptKind::Partial);
    }
    assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Pending);
    stt.finish().unwrap();
    assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Updated);
    assert_eq!((out.kind, out.turn_id), (TranscriptKind::Final, turn));
    assert_eq!(out.text, "open t");
    assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Done);
}

#[test]
fn polling_reuses_the_callers_transcript_buffer() {
    let mut stt: Box<dyn SttEngine> = Box::new(FakeStt::new(id("fake-stt")));
    let turn = TurnId::new();
    let mut out = Transcript::with_capacity(turn, TEXT_CAPACITY);
    let buffer = out.text.as_ptr();
    let loud = [LOUD; FRAME_SAMPLES];
    stt.begin(turn, None).unwrap();
    for index in 0..4 {
        stt.push(frame(&loud, index)).unwrap();
        stt.poll(&mut out).unwrap();
    }
    assert_eq!(out.text.as_ptr(), buffer);
}

#[test]
fn full_stt_queue_drops_the_frame_without_growing() {
    let mut stt = FakeStt::new(id("fake-stt"));
    let loud = [LOUD; FRAME_SAMPLES];
    stt.begin(TurnId::new(), None).unwrap();
    for index in 0..4 {
        stt.push(frame(&loud, index)).unwrap();
    }
    let error = stt.push(frame(&loud, 4)).unwrap_err();
    assert_eq!(
        error,
        SpeechError::Overflow {
            dropped: FRAME_SAMPLES
        }
    );
    assert!(!error.should_fall_back());
    assert_eq!(stt.buffer_capacity(), STT_CAPACITY_SAMPLES);
}

#[test]
fn stt_rejects_audio_outside_an_utterance() {
    let mut stt: Box<dyn SttEngine> = Box::new(FakeStt::new(id("fake-stt")));
    let loud = [LOUD; FRAME_SAMPLES];
    assert!(matches!(
        stt.push(frame(&loud, 0)),
        Err(SpeechError::OutOfOrder(_))
    ));
}

#[test]
fn begin_again_drops_the_previous_utterance() {
    let mut stt: Box<dyn SttEngine> = Box::new(FakeStt::new(id("fake-stt")));
    let (old, new) = (TurnId::new(), TurnId::new());
    let mut out = Transcript::with_capacity(old, TEXT_CAPACITY);
    let loud = [LOUD; FRAME_SAMPLES];
    stt.begin(old, None).unwrap();
    stt.push(frame(&loud, 0)).unwrap();
    stt.begin(new, None).unwrap();
    assert_eq!(stt.poll(&mut out).unwrap(), SttPoll::Pending);
    stt.push(frame(&loud, 1)).unwrap();
    stt.poll(&mut out).unwrap();
    assert_eq!(out.turn_id, new);
}

#[test]
fn registry_falls_back_past_a_failing_engine() {
    let mut engines: Vec<Box<dyn SttEngine>> = vec![
        Box::new(FakeStt::failing(
            id("broken-plugin"),
            SpeechError::unavailable("plugin process exited"),
        )),
        Box::new(FakeStt::new(id("fake-stt"))),
    ];
    let mut notices = Vec::new();
    let chosen = begin_with_fallback(&mut engines, TurnId::new(), &mut notices).unwrap();
    assert_eq!(engines[chosen].info().id.as_str(), "fake-stt");
    assert_eq!(
        notices,
        ["broken-plugin: engine unavailable: plugin process exited"]
    );
}

#[test]
fn caller_bugs_do_not_trigger_fallback() {
    let mut engines: Vec<Box<dyn SttEngine>> = vec![
        Box::new(FakeStt::failing(
            id("fake-a"),
            SpeechError::OutOfOrder("finish before begin"),
        )),
        Box::new(FakeStt::new(id("fake-b"))),
    ];
    let mut notices = Vec::new();
    assert!(begin_with_fallback(&mut engines, TurnId::new(), &mut notices).is_err());
    assert!(notices.is_empty());
}

#[test]
fn boxed_engines_move_to_a_worker_thread() {
    let stt: Box<dyn SttEngine> = Box::new(FakeStt::new(id("fake-stt")));
    let tts: Box<dyn TtsEngine> = Box::new(FakeTts::new(id("fake-tts")));
    let ids = std::thread::spawn(move || (stt.info().id.clone(), tts.info().id.clone()))
        .join()
        .unwrap();
    assert_eq!((ids.0.as_str(), ids.1.as_str()), ("fake-stt", "fake-tts"));
}

fn request(turn_id: TurnId, voice: Option<&str>) -> TtsRequest<'_> {
    TtsRequest {
        turn_id,
        voice,
        language: Some("en"),
        rate: SpeechRate::NORMAL,
    }
}

#[test]
fn tts_plays_the_first_sentence_before_the_reply_is_finished() {
    let mut tts: Box<dyn TtsEngine> = Box::new(FakeTts::new(id("fake-tts")));
    let mut out = [0.0; FRAME_SAMPLES];
    let format = tts.begin(&request(TurnId::new(), Some("anna"))).unwrap();
    assert_eq!(format.sample_rate_hz(), TTS_RATE_HZ);
    tts.push_text("Opening the browser.").unwrap();
    assert!(matches!(tts.poll(&mut out).unwrap(), TtsPoll::Audio { .. }));
}

#[test]
fn tts_fills_at_most_the_callers_slice_then_ends() {
    let mut tts: Box<dyn TtsEngine> = Box::new(FakeTts::new(id("fake-tts")));
    let mut out = [0.0; FRAME_SAMPLES];
    let sentence = "Done.";
    tts.begin(&request(TurnId::new(), None)).unwrap();
    tts.push_text(sentence).unwrap();
    tts.finish().unwrap();
    let mut total = 0;
    loop {
        match tts.poll(&mut out).unwrap() {
            TtsPoll::Audio { samples } => {
                assert!(samples <= out.len());
                total += samples;
            }
            TtsPoll::Pending => {}
            TtsPoll::Done => break,
        }
    }
    assert_eq!(total, sentence.len() * TTS_SAMPLES_PER_BYTE);
}

#[test]
fn barge_in_cancel_ends_the_reply_on_the_next_poll() {
    let mut tts: Box<dyn TtsEngine> = Box::new(FakeTts::new(id("fake-tts")));
    let mut out = [0.0; FRAME_SAMPLES];
    tts.begin(&request(TurnId::new(), None)).unwrap();
    tts.push_text("A long answer that would take many seconds to speak.")
        .unwrap();
    assert!(matches!(tts.poll(&mut out).unwrap(), TtsPoll::Audio { .. }));
    tts.cancel();
    assert_eq!(tts.poll(&mut out).unwrap(), TtsPoll::Done);
}

#[test]
fn unknown_voice_is_unsupported_so_the_registry_falls_back() {
    let mut tts: Box<dyn TtsEngine> = Box::new(FakeTts::new(id("fake-tts")));
    let error = tts
        .begin(&request(TurnId::new(), Some("boris")))
        .unwrap_err();
    assert!(matches!(error, SpeechError::Unsupported(_)));
    assert!(error.should_fall_back());
    assert_eq!(tts.voices().len(), 1);
}

#[test]
fn detectors_find_wake_word_speech_and_end_of_turn() {
    let mut kws: Box<dyn KeywordSpotter> = Box::new(FakeKws {
        info: info(id("fake-kws"), EngineKind::KeywordSpotter),
        keywords: 0,
    });
    let mut vad: Box<dyn Vad> = Box::new(FakeVad {
        info: info(id("fake-vad"), EngineKind::Vad),
        in_speech: false,
    });
    let mut turn: Box<dyn TurnDetector> = Box::new(SilenceTurn {
        info: info(id("silence"), EngineKind::TurnDetector),
        heard_speech: false,
        silent: 0,
    });
    kws.set_keywords(&["hey aulo"]).unwrap();
    let (loud, quiet) = ([LOUD; FRAME_SAMPLES], [0.0; FRAME_SAMPLES]);
    let speech_frames = 5;
    let mut events = Vec::new();
    let mut end = None;
    let mut wake = None;
    for index in 0..100 {
        let samples = if index < speech_frames { &loud } else { &quiet };
        let frame = frame(samples, index);
        wake = wake.or(kws.push(frame).unwrap());
        events.extend(vad.push(frame).unwrap());
        turn.observe_transcript("open the");
        if let TurnDecision::EndOfTurn { position } =
            turn.push(frame, index < speech_frames).unwrap()
        {
            end = Some(position);
            break;
        }
    }
    let speech_end = speech_frames * FRAME_SAMPLES as u64;
    assert_eq!(wake.map(|hit| hit.keyword), Some(0));
    assert_eq!(
        events,
        [
            VadEvent::SpeechStarted { position: 0 },
            VadEvent::SpeechEnded {
                position: speech_end
            }
        ]
    );
    assert_eq!(end, Some(speech_end + SILENCE_FOR_END_OF_TURN));
}
