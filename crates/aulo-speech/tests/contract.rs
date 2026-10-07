//! Contract tests: one fake engine per trait, driven only through `Box<dyn …>`
//! from outside the crate. They prove the traits are object safe and `Send`,
//! and pin the push/poll protocol the real engines and the pipeline rely on.

// Helpers outside `#[test]` fns are not covered by clippy's allow-in-tests.
#![allow(clippy::unwrap_used)]

use aulo_speech::{
    AudioFormat, AudioFrame, Capabilities, EngineId, EngineInfo, EngineKind, KeywordHit,
    KeywordSpotter, LanguageSupport, SpeechError, SpeechRate, SttEngine, SttPoll, Transcript,
    TranscriptKind, TtsEngine, TtsPoll, TtsRequest, TurnDecision, TurnDetector, TurnId, Vad,
    VadEvent, Voice,
};

/// 20 ms at 16 kHz, the frame size aulo-audio delivers.
const FRAME_SAMPLES: usize = 320;
const LOUD: f32 = 0.5;
const SPEECH_THRESHOLD: f32 = 0.1;
const STT_CAPACITY_SAMPLES: usize = FRAME_SAMPLES * 4;
const SAMPLES_PER_CHAR: usize = FRAME_SAMPLES / 2;
const SPOKEN: &str = "open the browser and play some music";
const TTS_RATE_HZ: u32 = 24_000;
const TTS_SAMPLES_PER_BYTE: usize = 100;
const SILENCE_FOR_END_OF_TURN: u64 = 16_000 * 700 / 1_000;
const TEXT_CAPACITY: usize = 64;

fn info(id: &str, kind: EngineKind) -> EngineInfo {
    EngineInfo {
        id: EngineId::new(id).unwrap(),
        kind,
        name: id.to_owned(),
        capabilities: Capabilities {
            streaming: true,
            languages: LanguageSupport::Listed(vec!["en".into()]),
            offline: true,
            needs_network: false,
        },
    }
}

fn frame(samples: &[f32], index: u64) -> AudioFrame<'_> {
    AudioFrame::new(samples, AudioFormat::PIPELINE, index * FRAME_SAMPLES as u64).unwrap()
}

/// Buffers audio in fixed storage and "recognizes" a prefix of `SPOKEN` that
/// grows with the amount of audio heard.
struct FakeStt {
    info: EngineInfo,
    fail_begin: Option<SpeechError>,
    turn: Option<TurnId>,
    audio: Vec<f32>,
    fresh: bool,
    finished: bool,
    final_sent: bool,
}

impl FakeStt {
    fn new(id: &str) -> Self {
        Self {
            info: info(id, EngineKind::Stt),
            fail_begin: None,
            turn: None,
            audio: Vec::with_capacity(STT_CAPACITY_SAMPLES),
            fresh: false,
            finished: false,
            final_sent: false,
        }
    }

    fn failing(id: &str, error: SpeechError) -> Self {
        Self {
            fail_begin: Some(error),
            ..Self::new(id)
        }
    }
}

impl SttEngine for FakeStt {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn begin(&mut self, turn_id: TurnId, _language: Option<&str>) -> Result<(), SpeechError> {
        if let Some(error) = &self.fail_begin {
            return Err(error.clone());
        }
        self.cancel();
        self.turn = Some(turn_id);
        Ok(())
    }

    fn push(&mut self, frame: AudioFrame<'_>) -> Result<(), SpeechError> {
        if self.turn.is_none() || self.finished {
            return Err(SpeechError::OutOfOrder("push outside an utterance"));
        }
        if self.audio.len() + frame.len() > self.audio.capacity() {
            return Err(SpeechError::Overflow {
                dropped: frame.len(),
            });
        }
        self.audio.extend_from_slice(frame.samples());
        self.fresh = true;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), SpeechError> {
        if self.turn.is_none() {
            return Err(SpeechError::OutOfOrder("finish before begin"));
        }
        self.finished = true;
        Ok(())
    }

    fn poll(&mut self, out: &mut Transcript) -> Result<SttPoll, SpeechError> {
        let Some(turn) = self.turn else {
            return Ok(SttPoll::Pending);
        };
        if self.final_sent {
            return Ok(SttPoll::Done);
        }
        let kind = if self.finished {
            self.final_sent = true;
            TranscriptKind::Final
        } else if self.fresh {
            TranscriptKind::Partial
        } else {
            return Ok(SttPoll::Pending);
        };
        self.fresh = false;
        let heard = (self.audio.len() / SAMPLES_PER_CHAR).min(SPOKEN.len());
        out.set(turn, kind, &SPOKEN[..heard], Some("en"));
        Ok(SttPoll::Updated)
    }

    fn cancel(&mut self) {
        self.turn = None;
        self.audio.clear();
        self.fresh = false;
        self.finished = false;
        self.final_sent = false;
    }
}

/// Turns every pushed byte of text into a fixed number of silent samples.
struct FakeTts {
    info: EngineInfo,
    voices: Vec<Voice>,
    active: bool,
    pending: usize,
    finished: bool,
}

impl FakeTts {
    fn new() -> Self {
        Self {
            info: info("fake-tts", EngineKind::Tts),
            voices: vec![Voice {
                id: "anna".into(),
                name: "Anna".into(),
                language: Some("en".into()),
            }],
            active: false,
            pending: 0,
            finished: false,
        }
    }
}

impl TtsEngine for FakeTts {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn voices(&self) -> &[Voice] {
        &self.voices
    }

    fn begin(&mut self, request: &TtsRequest<'_>) -> Result<AudioFormat, SpeechError> {
        if let Some(voice) = request.voice
            && !self.voices.iter().any(|v| v.id == voice)
        {
            return Err(SpeechError::unsupported(voice));
        }
        self.cancel();
        self.active = true;
        AudioFormat::new(TTS_RATE_HZ, 1)
    }

    fn push_text(&mut self, text: &str) -> Result<(), SpeechError> {
        if !self.active || self.finished {
            return Err(SpeechError::OutOfOrder("text outside a reply"));
        }
        self.pending += text.len() * TTS_SAMPLES_PER_BYTE;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), SpeechError> {
        self.finished = true;
        Ok(())
    }

    fn poll(&mut self, out: &mut [f32]) -> Result<TtsPoll, SpeechError> {
        if !self.active {
            return Ok(TtsPoll::Done);
        }
        let samples = self.pending.min(out.len());
        if samples > 0 {
            out[..samples].fill(0.0);
            self.pending -= samples;
            return Ok(TtsPoll::Audio { samples });
        }
        if self.finished {
            self.active = false;
            return Ok(TtsPoll::Done);
        }
        Ok(TtsPoll::Pending)
    }

    fn cancel(&mut self) {
        self.active = false;
        self.pending = 0;
        self.finished = false;
    }
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
    let mut stt: Box<dyn SttEngine> = Box::new(FakeStt::new("fake-stt"));
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
    let mut stt: Box<dyn SttEngine> = Box::new(FakeStt::new("fake-stt"));
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
    let mut stt = FakeStt::new("fake-stt");
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
    assert_eq!(stt.audio.capacity(), STT_CAPACITY_SAMPLES);
}

#[test]
fn stt_rejects_audio_outside_an_utterance() {
    let mut stt: Box<dyn SttEngine> = Box::new(FakeStt::new("fake-stt"));
    let loud = [LOUD; FRAME_SAMPLES];
    assert!(matches!(
        stt.push(frame(&loud, 0)),
        Err(SpeechError::OutOfOrder(_))
    ));
}

#[test]
fn begin_again_drops_the_previous_utterance() {
    let mut stt: Box<dyn SttEngine> = Box::new(FakeStt::new("fake-stt"));
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
            "broken-plugin",
            SpeechError::unavailable("plugin process exited"),
        )),
        Box::new(FakeStt::new("fake-stt")),
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
            "fake-a",
            SpeechError::OutOfOrder("finish before begin"),
        )),
        Box::new(FakeStt::new("fake-b")),
    ];
    let mut notices = Vec::new();
    assert!(begin_with_fallback(&mut engines, TurnId::new(), &mut notices).is_err());
    assert!(notices.is_empty());
}

#[test]
fn boxed_engines_move_to_a_worker_thread() {
    let stt: Box<dyn SttEngine> = Box::new(FakeStt::new("fake-stt"));
    let tts: Box<dyn TtsEngine> = Box::new(FakeTts::new());
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
    let mut tts: Box<dyn TtsEngine> = Box::new(FakeTts::new());
    let mut out = [0.0; FRAME_SAMPLES];
    let format = tts.begin(&request(TurnId::new(), Some("anna"))).unwrap();
    assert_eq!(format.sample_rate_hz(), TTS_RATE_HZ);
    tts.push_text("Opening the browser.").unwrap();
    assert!(matches!(tts.poll(&mut out).unwrap(), TtsPoll::Audio { .. }));
}

#[test]
fn tts_fills_at_most_the_callers_slice_then_ends() {
    let mut tts: Box<dyn TtsEngine> = Box::new(FakeTts::new());
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
    let mut tts: Box<dyn TtsEngine> = Box::new(FakeTts::new());
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
    let mut tts: Box<dyn TtsEngine> = Box::new(FakeTts::new());
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
        info: info("fake-kws", EngineKind::KeywordSpotter),
        keywords: 0,
    });
    let mut vad: Box<dyn Vad> = Box::new(FakeVad {
        info: info("fake-vad", EngineKind::Vad),
        in_speech: false,
    });
    let mut turn: Box<dyn TurnDetector> = Box::new(SilenceTurn {
        info: info("silence", EngineKind::TurnDetector),
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
