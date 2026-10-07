//! The installed voices, read once when the engine loads so `voices()` never
//! calls into AVFoundation, and the rule that picks one for a language.

use std::cmp::Reverse;

use aulo_speech::Voice;
use objc2_avf_audio::{AVSampleRateKey, AVSpeechSynthesisVoice, AVSpeechSynthesisVoiceQuality};
use objc2_foundation::NSNumber;

/// Rate of most macOS voices; used when a voice does not report its own.
pub(super) const FALLBACK_RATE_HZ: u32 = 22_050;
/// No speech voice runs faster; a larger reported rate is not trusted.
const MAX_RATE_HZ: f64 = 192_000.0;
const SUBTAG_SEPARATOR: char = '-';

/// What `Voice` does not carry but the engine needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VoiceMeta {
    pub quality: isize,
    pub sample_rate_hz: u32,
}

/// Lists `AVSpeechSynthesisVoice.speechVoices`. Quality goes into the
/// display name because `Voice` has no field for it and the desktop picker
/// should still tell a premium voice from a compact one.
pub(super) fn installed() -> (Vec<Voice>, Vec<VoiceMeta>) {
    // SAFETY: a class method with no arguments; it returns an owned array.
    let list = unsafe { AVSpeechSynthesisVoice::speechVoices() };
    list.iter()
        .map(|voice| {
            // SAFETY: plain property getters on a live voice object.
            let (id, name, language, quality) = unsafe {
                (
                    voice.identifier().to_string(),
                    voice.name().to_string(),
                    voice.language().to_string(),
                    voice.quality(),
                )
            };
            let voice_out = Voice {
                id,
                name: display_name(&name, quality),
                language: (!language.is_empty()).then_some(language),
            };
            let meta = VoiceMeta {
                quality: quality.0,
                sample_rate_hz: sample_rate(&voice),
            };
            (voice_out, meta)
        })
        .unzip()
}

fn display_name(name: &str, quality: AVSpeechSynthesisVoiceQuality) -> String {
    match quality {
        AVSpeechSynthesisVoiceQuality::Premium => format!("{name} (Premium)"),
        AVSpeechSynthesisVoiceQuality::Enhanced => format!("{name} (Enhanced)"),
        _ => name.to_owned(),
    }
}

/// The voice's output rate from its `audioFileSettings`, which matches the
/// buffers `writeUtterance` delivers (16 kHz for Eloquence voices, 22.05 kHz
/// for most others on macOS 27).
fn sample_rate(voice: &AVSpeechSynthesisVoice) -> u32 {
    // SAFETY: a property getter on a live voice; reading an immutable
    // framework constant.
    let (settings, key) = unsafe { (voice.audioFileSettings(), AVSampleRateKey) };
    key.and_then(|key| settings.objectForKey(key))
        .and_then(|value| value.downcast::<NSNumber>().ok())
        .map(|number| number.as_f64())
        .filter(|hz| (1.0..=MAX_RATE_HZ).contains(hz))
        .map_or(FALLBACK_RATE_HZ, |hz| hz.round() as u32)
}

/// The voice to use for `language`: an exact tag match before a match on the
/// primary subtag (`en` serves `en-GB`), then the highest quality, then the
/// first listed, so the choice is stable across calls.
pub(super) fn choose(voices: &[Voice], meta: &[VoiceMeta], language: &str) -> Option<usize> {
    let candidates = voices.iter().zip(meta);
    best_match(
        candidates.map(|(voice, meta)| (voice.language.as_deref(), meta.quality)),
        language,
    )
}

/// The same rule over any `(tag, rank)` list, so the recognizer picks its
/// locale exactly as the synthesizer picks its voice.
pub(super) fn best_match<'a>(
    candidates: impl Iterator<Item = (Option<&'a str>, isize)>,
    language: &str,
) -> Option<usize> {
    let wanted = primary(language);
    candidates
        .enumerate()
        .filter_map(|(index, (tag, rank))| {
            let tag = tag?;
            let exact = tag.eq_ignore_ascii_case(language);
            let close = exact || primary(tag).eq_ignore_ascii_case(wanted);
            // Lower index wins a tie, hence the reversed index in the key.
            close.then_some((exact, rank, Reverse(index)))
        })
        .max()
        .map(|(_, _, Reverse(index))| index)
}

fn primary(tag: &str) -> &str {
    tag.split(SUBTAG_SEPARATOR).next().unwrap_or(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(id: &str, language: &str) -> Voice {
        Voice {
            id: id.into(),
            name: id.into(),
            language: Some(language.into()),
        }
    }

    fn meta(quality: isize) -> VoiceMeta {
        VoiceMeta {
            quality,
            sample_rate_hz: FALLBACK_RATE_HZ,
        }
    }

    #[test]
    fn exact_tag_beats_quality_and_primary_subtag_is_the_fallback() {
        let voices = [
            voice("gb", "en-GB"),
            voice("us", "en-US"),
            voice("us-hq", "en-US"),
        ];
        let meta = [meta(3), meta(1), meta(2)];
        assert_eq!(choose(&voices, &meta, "en-us"), Some(2));
        assert_eq!(choose(&voices, &meta, "en-AU"), Some(0));
        assert_eq!(choose(&voices, &meta, "en"), Some(0));
        assert_eq!(choose(&voices, &meta, "ru-RU"), None);
    }

    #[test]
    fn equal_voices_resolve_to_the_first_listed() {
        let voices = [voice("a", "ru-RU"), voice("b", "ru-RU")];
        assert_eq!(choose(&voices, &[meta(1), meta(1)], "ru-RU"), Some(0));
    }

    #[test]
    fn installed_voices_have_ids_and_plausible_rates() {
        let (voices, meta) = installed();
        assert!(!voices.is_empty(), "macOS ships system voices");
        assert_eq!(voices.len(), meta.len());
        assert!(voices.iter().all(|v| !v.id.is_empty()));
        assert!(
            meta.iter()
                .all(|m| (8_000..=48_000).contains(&m.sample_rate_hz))
        );
    }
}
