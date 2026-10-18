//! Spotting a capture that runs but hears nothing. A refused microphone is
//! delivered as exact digital zeros, never as an error, and a probe can be
//! wrong or absent (Linux, a changed setting mid-session), so the frames
//! themselves are the last line of defence.

use aulo_speech::AudioFrame;

/// 3 s of frames: longer than any pause a live microphone's own noise floor
/// would leave at exactly zero, short enough to tell the user quickly.
pub const DEFAULT_SILENCE_FRAMES: u32 = 150;

/// Counts consecutive frames of digital silence.
#[derive(Debug, Clone)]
pub struct SilenceWatch {
    limit: u32,
    run: u32,
}

impl SilenceWatch {
    pub fn new(limit_frames: u32) -> Self {
        Self {
            limit: limit_frames.max(1),
            run: 0,
        }
    }

    /// True on the frame that completes a silent run, once per run: the caller
    /// raises one notice, not one per frame. Any signal restarts the count.
    pub fn observe(&mut self, frame: &AudioFrame<'_>) -> bool {
        if frame.samples().iter().any(|s| *s != 0.0) {
            self.run = 0;
            return false;
        }
        self.run = self.run.saturating_add(1);
        self.run == self.limit
    }
}

impl Default for SilenceWatch {
    fn default() -> Self {
        Self::new(DEFAULT_SILENCE_FRAMES)
    }
}

#[cfg(test)]
mod tests {
    use aulo_speech::AudioFormat;

    use super::*;
    use crate::FRAME_SAMPLES;

    fn observe(watch: &mut SilenceWatch, level: f32) -> bool {
        let samples = [level; FRAME_SAMPLES];
        let frame = AudioFrame::new(&samples, AudioFormat::PIPELINE, 0).unwrap();
        watch.observe(&frame)
    }

    #[test]
    fn a_silent_run_fires_once_when_it_reaches_the_limit() {
        let mut watch = SilenceWatch::new(3);
        let fired: Vec<bool> = (0..6).map(|_| observe(&mut watch, 0.0)).collect();
        assert_eq!(fired, [false, false, true, false, false, false]);
    }

    #[test]
    fn any_signal_restarts_the_run_and_a_later_run_fires_again() {
        let mut watch = SilenceWatch::new(2);
        assert!(!observe(&mut watch, 0.0));
        assert!(!observe(&mut watch, 0.001));
        assert!(!observe(&mut watch, 0.0));
        assert!(observe(&mut watch, 0.0));
    }

    #[test]
    fn a_zero_limit_still_needs_one_silent_frame() {
        let mut watch = SilenceWatch::new(0);
        assert!(!observe(&mut watch, 0.5));
        assert!(observe(&mut watch, 0.0));
    }
}
