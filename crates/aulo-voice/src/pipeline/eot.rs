//! End of turn by silence (spec §4.3 step 4): the default detector.
//!
//! The detector counts the quiet frames after the VAD reports the end of
//! speech and ends the turn once they pass `silence_ms`. A pause inside the
//! user's sentence resets the count, so a mid-sentence breath does not cut
//! the turn short. The semantic Smart Turn v3 model (T8.7) refines this
//! verdict on the audio when it is enabled; the silence timeout stays the
//! floor underneath it.

use aulo_speech::{
    AudioFrame, Capabilities, EngineId, EngineInfo, EngineKind, LanguageSupport, SpeechError,
    TurnDecision, TurnDetector,
};

/// Silence before the user's turn ends, in milliseconds (spec §5.1).
pub const DEFAULT_SILENCE_MS: u32 = 700;

const MS_PER_SECOND: u64 = 1_000;

/// Ends the turn after `silence_ms` of continuous silence.
#[derive(Debug)]
pub struct SilenceEndOfTurn {
    info: EngineInfo,
    silence_ms: u32,
    /// Consecutive non-speech samples since the last frame the VAD called
    /// speech.
    quiet_samples: u64,
}

impl SilenceEndOfTurn {
    /// `silence_ms` of zero ends the turn on the first quiet frame, which is
    /// what a strict push-to-talk setup wants.
    pub fn new(silence_ms: u32) -> Self {
        Self {
            info: EngineInfo {
                id: EngineId::builtin("silence"),
                kind: EngineKind::TurnDetector,
                name: "Silence end of turn".to_owned(),
                capabilities: Capabilities {
                    streaming: true,
                    languages: LanguageSupport::Any,
                    offline: true,
                    needs_network: false,
                },
            },
            silence_ms,
            quiet_samples: 0,
        }
    }

    /// The timeout in milliseconds.
    pub fn silence_ms(&self) -> u32 {
        self.silence_ms
    }
}

impl TurnDetector for SilenceEndOfTurn {
    fn info(&self) -> &EngineInfo {
        &self.info
    }

    fn push(
        &mut self,
        frame: AudioFrame<'_>,
        in_speech: bool,
    ) -> Result<TurnDecision, SpeechError> {
        if in_speech {
            self.quiet_samples = 0;
            return Ok(TurnDecision::Continue);
        }
        self.quiet_samples += u64::try_from(frame.len()).unwrap_or(u64::MAX);
        let quiet_ms = self.quiet_samples.saturating_mul(MS_PER_SECOND)
            / u64::from(frame.format().sample_rate_hz());
        if quiet_ms >= u64::from(self.silence_ms) {
            // The turn ends where this frame ends, so the silence the engine
            // heard is part of the utterance the STT decodes.
            Ok(TurnDecision::EndOfTurn {
                position: frame.end_position(),
            })
        } else {
            Ok(TurnDecision::Continue)
        }
    }

    fn reset(&mut self) {
        self.quiet_samples = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aulo_speech::{AudioFormat, AudioFrame};

    const FRAME: usize = 320; // 20 ms at 16 kHz.
    static SILENCE: [f32; FRAME] = [0.0; FRAME];

    fn frame(position: u64) -> AudioFrame<'static> {
        AudioFrame::new(&SILENCE, AudioFormat::PIPELINE, position).expect("frame")
    }

    #[test]
    fn silence_past_the_timeout_ends_the_turn() {
        let mut detector = SilenceEndOfTurn::new(700);
        for i in 0..10 {
            assert_eq!(
                detector.push(frame(i * FRAME as u64), true).expect("push"),
                TurnDecision::Continue
            );
        }
        // 34 quiet frames are 680 ms: not yet. The 35th crosses 700 ms.
        for i in 0..34 {
            let position = (10 + i) * FRAME as u64;
            assert_eq!(
                detector.push(frame(position), false).expect("push"),
                TurnDecision::Continue,
                "frame {i}"
            );
        }
        let decision = detector
            .push(frame((10 + 34) * FRAME as u64), false)
            .expect("push");
        assert_eq!(
            decision,
            TurnDecision::EndOfTurn {
                position: 45 * FRAME as u64
            }
        );
    }

    #[test]
    fn a_mid_sentence_pause_does_not_end_the_turn() {
        let mut detector = SilenceEndOfTurn::new(700);
        let mut position = 0;
        for _ in 0..50 {
            assert_eq!(
                detector.push(frame(position), true).expect("push"),
                TurnDecision::Continue
            );
            position += FRAME as u64;
        }
        // 400 ms of silence: well under the timeout.
        for _ in 0..20 {
            assert_eq!(
                detector.push(frame(position), false).expect("push"),
                TurnDecision::Continue
            );
            position += FRAME as u64;
        }
        // The user carries on; the count starts over.
        for _ in 0..50 {
            assert_eq!(
                detector.push(frame(position), true).expect("push"),
                TurnDecision::Continue
            );
            position += FRAME as u64;
        }
        // And a full timeout still ends the turn afterwards.
        for _ in 0..35 {
            detector.push(frame(position), false).expect("push");
            position += FRAME as u64;
        }
        assert_eq!(
            detector.push(frame(position), false).expect("push"),
            TurnDecision::EndOfTurn {
                position: position + FRAME as u64
            }
        );
    }

    #[test]
    fn reset_forgets_the_silence_so_far() {
        let mut detector = SilenceEndOfTurn::new(700);
        for i in 0..30 {
            detector.push(frame(i * FRAME as u64), false).expect("push");
        }
        detector.reset();
        for i in 0..34 {
            assert_eq!(
                detector.push(frame(i * FRAME as u64), false).expect("push"),
                TurnDecision::Continue
            );
        }
    }

    #[test]
    fn a_zero_timeout_ends_the_turn_on_the_first_quiet_frame() {
        let mut detector = SilenceEndOfTurn::new(0);
        assert_eq!(
            detector.push(frame(0), false).expect("push"),
            TurnDecision::EndOfTurn {
                position: FRAME as u64
            }
        );
    }
}
