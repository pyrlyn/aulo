//! Linear-interpolation resampling for the engines whose synthesizer reports
//! its own sample rate only once audio arrives, after the engine has already
//! declared one. Public so the speech benchmarks can time it.

/// Rates closer than this are treated as equal and passed through untouched.
const SAME_RATE_EPSILON_HZ: f64 = 0.5;

/// Streaming linear-interpolation resampler. Voices are declared at their own
/// rate, so this only runs when a buffer disagrees with the declared format;
/// linear is enough for speech.
#[derive(Debug, Clone)]
pub struct Resampler {
    /// Input samples per output sample; `None` passes samples through.
    step: Option<f64>,
    /// Time of the next output sample, in input samples after `prev`.
    pos: f64,
    prev: Option<f32>,
}

impl Resampler {
    pub fn new(input_hz: f64, output_hz: u32) -> Self {
        let output_hz = f64::from(output_hz);
        // A non-positive input rate — a malformed buffer reporting 0 Hz,
        // which the macOS callback path feeds in straight from
        // `format.sampleRate()` — must pass through: a step of zero or less
        // never advances `pos`, and the emit loop in `push` would spin
        // forever on the main dispatch queue (T16.2).
        let differs = (input_hz - output_hz).abs() >= SAME_RATE_EPSILON_HZ
            && input_hz > 0.0
            && output_hz > 0.0;
        Self {
            step: differs.then(|| input_hz / output_hz),
            pos: 0.0,
            prev: None,
        }
    }

    pub fn push(&mut self, sample: f32, mut emit: impl FnMut(f32)) {
        let Some(step) = self.step else {
            return emit(sample);
        };
        if let Some(prev) = self.prev {
            while self.pos < 1.0 {
                emit(prev + (sample - prev) * self.pos as f32);
                self.pos += step;
            }
            self.pos -= 1.0;
        }
        self.prev = Some(sample);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(resampler: &mut Resampler, input: &[f32]) -> Vec<f32> {
        let mut out = Vec::new();
        for &sample in input {
            resampler.push(sample, |s| out.push(s));
        }
        out
    }

    /// T16.2: a non-positive input rate (a malformed buffer reporting 0 Hz
    /// from the macOS callback path) passes through instead of spinning in
    /// the emit loop forever — before the guard, `step <= 0` never advanced
    /// `pos` and `push` hung the main dispatch queue.
    #[test]
    fn a_non_positive_input_rate_passes_through() {
        for broken in [0.0, -16000.0] {
            let mut r = Resampler::new(broken, 48_000);
            let input = [0.1, -0.2, 0.3];
            let out = run(&mut r, &input);
            assert_eq!(out, input, "{broken} Hz must pass samples through");
        }
    }

    #[test]
    fn equal_rates_pass_samples_through() {
        let input = [0.1, -0.2, 0.3];
        assert_eq!(run(&mut Resampler::new(22_050.0, 22_050), &input), input);
    }

    #[test]
    fn upsampling_interpolates_between_input_samples() {
        let out = run(&mut Resampler::new(16_000.0, 32_000), &[0.0, 1.0, 2.0, 3.0]);
        assert_eq!(out, [0.0, 0.5, 1.0, 1.5, 2.0, 2.5]);
    }

    #[test]
    fn resampling_keeps_the_duration_and_the_level() {
        let input = vec![0.25; 16_000];
        let out = run(&mut Resampler::new(16_000.0, 22_050), &input);
        assert!(out.len().abs_diff(22_050) <= 2, "{} samples", out.len());
        assert!(out.iter().all(|&s| (s - 0.25).abs() < 1e-6));
    }
}
