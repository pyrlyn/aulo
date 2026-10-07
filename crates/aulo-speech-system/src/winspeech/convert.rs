//! Mapping aulo's speech rate onto WinRT's, over plain numbers.

use aulo_speech::SpeechRate;

/// `SpeechSynthesizerOptions.SpeakingRate` is a multiple of the voice's normal
/// pace like aulo's, but WinRT documents 0.5 to 6.0 and rejects anything
/// else, so a value is held inside it rather than trusted.
const MIN_SPEAKING_RATE: f64 = 0.5;
const MAX_SPEAKING_RATE: f64 = 6.0;

pub(super) fn speaking_rate(rate: SpeechRate) -> f64 {
    f64::from(rate.get()).clamp(MIN_SPEAKING_RATE, MAX_SPEAKING_RATE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_passes_through_inside_winrt_bounds() {
        let at = |r| speaking_rate(SpeechRate::new(r).unwrap());
        assert_eq!(at(1.0), 1.0);
        assert_eq!(at(0.5), 0.5);
        assert_eq!(at(2.0), 2.0);
    }
}
