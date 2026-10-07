//! The rule that picks a voice for a language, over plain [`Voice`] values.

use aulo_speech::Voice;

use crate::language::best_match;

/// The voice to use for `language`: an exact tag match before a match on the
/// primary subtag (`en` serves `en-GB`), then the first listed, so the choice
/// is stable across calls. WinRT reports no quality, so every voice ranks equal.
pub(super) fn choose(voices: &[Voice], language: &str) -> Option<usize> {
    best_match(voices.iter().map(|v| (v.language.as_deref(), 0)), language)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(id: &str, language: Option<&str>) -> Voice {
        Voice {
            id: id.into(),
            name: id.into(),
            language: language.map(Into::into),
        }
    }

    #[test]
    fn exact_tag_beats_order_and_primary_subtag_is_the_fallback() {
        let voices = [
            voice("gb", Some("en-GB")),
            voice("none", None),
            voice("us", Some("en-US")),
            voice("us2", Some("en-US")),
        ];
        assert_eq!(choose(&voices, "en-us"), Some(2));
        assert_eq!(choose(&voices, "en-AU"), Some(0));
        assert_eq!(choose(&voices, "en"), Some(0));
        assert_eq!(choose(&voices, "ru-RU"), None);
        assert_eq!(choose(&[], "en"), None);
    }
}
