//! Fake STT and TTS engines for tests, behind the `testkit` feature so the
//! contract tests here and the registry and pipeline tests elsewhere drive
//! the same fakes instead of each keeping a copy.

use crate::{
    AudioFormat, AudioFrame, Capabilities, EngineId, EngineInfo, EngineKind, LanguageSupport,
    SpeechError, SttEngine, SttPoll, Transcript, TranscriptKind, TtsEngine, TtsPoll, TtsRequest,
    TurnId, Voice,
};

/// 20 ms at 16 kHz, the frame size aulo-audio delivers.
pub const FRAME_SAMPLES: usize = 320;
/// What [`FakeStt`] "recognizes", a prefix growing with the audio heard.
pub const SPOKEN: &str = "open the browser and play some music";
pub const SAMPLES_PER_CHAR: usize = FRAME_SAMPLES / 2;
/// Fixed audio storage of [`FakeStt`]; a push beyond it overflows.
pub const STT_CAPACITY_SAMPLES: usize = FRAME_SAMPLES * 4;
pub const TTS_RATE_HZ: u32 = 24_000;
pub const TTS_SAMPLES_PER_BYTE: usize = 100;
/// The one voice [`FakeTts`] offers.
pub const FAKE_VOICE: &str = "anna";

/// An offline, streaming, English-only engine description.
pub fn info(id: EngineId, kind: EngineKind) -> EngineInfo {
    EngineInfo {
        name: id.as_str().to_owned(),
        id,
        kind,
        capabilities: Capabilities {
            streaming: true,
            languages: LanguageSupport::Listed(vec!["en".into()]),
            offline: true,
            needs_network: false,
        },
    }
}

/// Buffers audio in fixed storage and "recognizes" a prefix of [`SPOKEN`]
/// that grows with the amount of audio heard.
#[derive(Debug)]
pub struct FakeStt {
    info: EngineInfo,
    fail_begin: Option<SpeechError>,
    turn: Option<TurnId>,
    audio: Vec<f32>,
    fresh: bool,
    finished: bool,
    final_sent: bool,
}

impl FakeStt {
    pub fn new(id: EngineId) -> Self {
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

    /// An engine whose every `begin` returns `error`.
    pub fn failing(id: EngineId, error: SpeechError) -> Self {
        Self {
            fail_begin: Some(error),
            ..Self::new(id)
        }
    }

    /// Lets tests prove the audio storage never grows.
    pub fn buffer_capacity(&self) -> usize {
        self.audio.capacity()
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

/// Turns every pushed byte of text into [`TTS_SAMPLES_PER_BYTE`] silent
/// samples. Offers one voice, [`FAKE_VOICE`].
#[derive(Debug)]
pub struct FakeTts {
    info: EngineInfo,
    voices: Vec<Voice>,
    fail_begin: Option<SpeechError>,
    active: bool,
    pending: usize,
    finished: bool,
}

impl FakeTts {
    pub fn new(id: EngineId) -> Self {
        Self {
            info: info(id, EngineKind::Tts),
            voices: vec![Voice {
                id: FAKE_VOICE.into(),
                name: "Anna".into(),
                language: Some("en".into()),
            }],
            fail_begin: None,
            active: false,
            pending: 0,
            finished: false,
        }
    }

    /// An engine whose every `begin` returns `error`.
    pub fn failing(id: EngineId, error: SpeechError) -> Self {
        Self {
            fail_begin: Some(error),
            ..Self::new(id)
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
        if let Some(error) = &self.fail_begin {
            return Err(error.clone());
        }
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
