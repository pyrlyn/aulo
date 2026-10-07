//! What `espeak-ng --voices` lists, and how a request maps to the flags that
//! pick one. The listing is the only source of voice ids: a request's voice is
//! looked up in it, so text from a config or a remote client never reaches the
//! command line unchecked.

use aulo_speech::{SpeechRate, Voice};

/// espeak-ng's own default speed, in words per minute (`-s`).
const NORMAL_WPM: f32 = 175.0;
/// The range espeak-ng documents for `-s`.
const MIN_WPM: f32 = 80.0;
const MAX_WPM: f32 = 450.0;
/// A real install lists about 130; more is not from espeak-ng.
const MAX_VOICES: usize = 1024;
const MAX_ID_BYTES: usize = 128;
const LANGUAGE_SEPARATOR: char = '-';

/// Columns: `Pty Language Age/Gender VoiceName File [Other Languages]`.
/// `VoiceName` has underscores for spaces, so a row splits on whitespace.
pub(super) fn parse(listing: &str) -> Vec<Voice> {
    listing
        .lines()
        .filter_map(parse_row)
        .take(MAX_VOICES)
        .collect()
}

fn parse_row(line: &str) -> Option<Voice> {
    let mut columns = line.split_whitespace();
    // The header row's first column is not a number, which drops it.
    columns.next()?.parse::<u8>().ok()?;
    let language = columns.next()?;
    let _age_gender = columns.next()?;
    let name = columns.next()?;
    let file = columns.next()?;
    // MBROLA voices need a separate synthesizer and run at another rate.
    if file.starts_with("mb/") || !is_safe_id(file) || !is_language_tag(language) {
        return None;
    }
    Some(Voice {
        id: file.to_owned(),
        name: name.replace('_', " "),
        language: Some(language.to_owned()),
    })
}

/// The id goes to `-v`, so it must not look like another flag.
fn is_safe_id(id: &str) -> bool {
    id.len() <= MAX_ID_BYTES
        && id.starts_with(|c: char| c.is_ascii_alphanumeric())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_/.".contains(c))
}

fn is_language_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 32
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == LANGUAGE_SEPARATOR)
}

/// The listed voice for a BCP 47 tag: an exact tag first, then the first one
/// of the same language, since espeak-ng's regions (`en-gb`, `en-us`) are
/// accents of one language.
pub(super) fn choose<'a>(voices: &'a [Voice], tag: &str) -> Option<&'a Voice> {
    let language = |v: &'a Voice| v.language.as_deref().unwrap_or_default();
    let primary = |t: &'a str| t.split(LANGUAGE_SEPARATOR).next().unwrap_or(t);
    let exact = voices
        .iter()
        .find(|v| language(v).eq_ignore_ascii_case(tag));
    exact.or_else(|| {
        let want = tag.split(LANGUAGE_SEPARATOR).next().unwrap_or(tag);
        voices
            .iter()
            .find(|v| primary(language(v)).eq_ignore_ascii_case(want))
    })
}

/// `-s` for a rate multiple, clamped to what espeak-ng accepts.
pub(super) fn words_per_minute(rate: SpeechRate) -> u32 {
    (NORMAL_WPM * rate.get()).round().clamp(MIN_WPM, MAX_WPM) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "\
Pty Language       Age/Gender VoiceName          File                 Other Languages
 5  af              --/M      Afrikaans          gmw/af
 5  en-gb           --/M      English_(Great_Britain) gmw/en-GB        (en 2)
 2  en-us           --/M      English_(America)  gmw/en-US            (en 3)
 5  de              --/M      German             gmw/de
 5  en              --/M      mb-en1             mb/mb-en1
 5  xx              --/M      Evil               -v/../x
 not a voice row
 5  ru              --/M      Russian            zle/ru
";

    #[test]
    fn rows_become_voices_and_junk_is_skipped() {
        let voices = parse(LISTING);
        let ids: Vec<_> = voices.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(
            ids,
            ["gmw/af", "gmw/en-GB", "gmw/en-US", "gmw/de", "zle/ru"]
        );
        assert_eq!(voices[1].name, "English (Great Britain)");
        assert_eq!(voices[1].language.as_deref(), Some("en-gb"));
    }

    #[test]
    fn the_listing_is_capped() {
        let row = " 5  en  --/M  Name  gmw/en\n";
        assert_eq!(parse(&row.repeat(MAX_VOICES + 10)).len(), MAX_VOICES);
    }

    #[test]
    fn a_language_picks_an_exact_tag_before_a_sibling_region() {
        let voices = parse(LISTING);
        let id = |tag| choose(&voices, tag).map(|v| v.id.as_str());
        assert_eq!(id("en-US"), Some("gmw/en-US"));
        assert_eq!(id("en-AU"), Some("gmw/en-GB"));
        assert_eq!(id("ru"), Some("zle/ru"));
        assert_eq!(id("tlh"), None);
    }

    #[test]
    fn rates_map_to_the_documented_speed_range() {
        let wpm = |rate: f32| words_per_minute(SpeechRate::new(rate).unwrap());
        assert_eq!(wpm(1.0), 175);
        assert_eq!(wpm(0.5), 88);
        assert_eq!(wpm(2.0), 350);
        assert!((MIN_WPM..=MAX_WPM).contains(&(wpm(0.5) as f32)));
    }
}
