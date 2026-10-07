//! Linear-interpolation resampling for the engines whose synthesizer reports
//! its own sample rate only once audio arrives, after the engine has already
//! declared one.

#![cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]

/// Rates closer than this are treated as equal and passed through untouched.
const SAME_RATE_EPSILON_HZ: f64 = 0.5;

/// Streaming linear-interpolation resampler. Voices are declared at their own
/// rate, so this only runs when a buffer disagrees with the declared format;
/// linear is enough for speech.
#[derive(Debug, Clone)]
pub(crate) struct Resampler {
    /// Input samples per output sample; `None` passes samples through.
    step: Option<f64>,
    /// Time of the next output sample, in input samples after `prev`.
    pos: f64,
    prev: Option<f32>,
}

impl Resampler {
    pub(crate) fn new(input_hz: f64, output_hz: u32) -> Self {
        let output_hz = f64::from(output_hz);
        let differs = (input_hz - output_hz).abs() >= SAME_RATE_EPSILON_HZ && output_hz > 0.0;
        Self {
            step: differs.then(|| input_hz / output_hz),
            pos: 0.0,
            prev: None,
        }
    }

    pub(crate) fn push(&mut self, sample: f32, mut emit: impl FnMut(f32)) {
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
