//! Markdown reply in, plain speakable text out.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use url::Url;

use crate::expand::expand;

/// Largest reply the normalizer reads. Model text is untrusted; a runaway
/// reply must not turn into minutes of speech or unbounded work.
pub const MAX_INPUT_BYTES: usize = 64 * 1024;
/// Longest text spoken from one reply; the rest is replaced by the
/// "it is on the screen" phrase because nobody listens to a page of output.
pub const MAX_SPOKEN_CHARS: usize = 2_000;

/// Languages with spoken-form rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Ru,
}

impl Language {
    /// Matches the primary subtag of a BCP 47 tag, so `ru-RU` is Russian.
    pub fn from_tag(tag: &str) -> Option<Self> {
        match tag.split(['-', '_']).next()?.to_ascii_lowercase().as_str() {
            "en" => Some(Self::En),
            "ru" => Some(Self::Ru),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ru => "ru",
        }
    }

    /// Spoken in place of code, tables and output that was cut for length.
    pub fn on_screen(self) -> &'static str {
        match self {
            Self::En => "It's on the screen.",
            Self::Ru => "Это на экране.",
        }
    }

    fn link(self) -> &'static str {
        match self {
            Self::En => "link",
            Self::Ru => "ссылка",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SpeakOptions {
    pub language: Language,
    pub max_input_bytes: usize,
    pub max_spoken_chars: usize,
}

impl SpeakOptions {
    pub fn new(language: Language) -> Self {
        Self {
            language,
            max_input_bytes: MAX_INPUT_BYTES,
            max_spoken_chars: MAX_SPOKEN_CHARS,
        }
    }
}

/// Guesses the language of `text` from its letters, so the voice can follow
/// what the model wrote. `None` when there are no Latin or Cyrillic letters;
/// the caller then keeps the chat's configured language. Cyrillic needs only
/// half as many letters as Latin because Russian replies often quote English
/// commands and names.
pub fn detect_language(text: &str) -> Option<Language> {
    let (mut cyrillic, mut latin) = (0usize, 0usize);
    for c in text.chars() {
        if ('\u{0400}'..='\u{04FF}').contains(&c) {
            cyrillic += 1;
        } else if c.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    match (cyrillic, latin) {
        (0, 0) => None,
        (c, l) if c * 2 >= l => Some(Language::Ru),
        _ => Some(Language::En),
    }
}

/// Cuts `s` to at most `max` bytes without splitting a character.
pub(crate) fn cap_bytes(s: &str, max: usize) -> &str {
    let mut end = max.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Characters that make spoken text unsafe or confusing: controls (terminal
/// escapes), soft hyphens, zero-width and bidirectional marks. Bidi overrides
/// can make a reply read differently from what is shown, so they go before
/// the text is parsed. Ordinary whitespace is kept.
fn is_unspeakable(c: char) -> bool {
    (c.is_control() && !c.is_whitespace())
        || matches!(
            c,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
        )
}

/// Normalizes one markdown reply for speech: markdown stripped, code and
/// tables replaced by [`Language::on_screen`], URLs shortened to the host,
/// numbers and units spelled out, whitespace collapsed. Never panics and is
/// bounded by the options' caps. Returns an empty string when nothing is
/// speakable.
///
/// Limits: no number-to-words (engines read digits), no negative numbers,
/// dates, times or versions (left as written), only the single units in
/// `expand.rs` (no `km/h`), only `http(s)` URLs, no per-sentence language switching.
pub fn speakable(markdown: &str, options: SpeakOptions) -> String {
    let lang = options.language;
    let clean: String = cap_bytes(markdown, options.max_input_bytes)
        .chars()
        .filter(|&c| !is_unspeakable(c))
        .collect();
    let plain = strip_markdown(&clean, lang);
    let text = shorten_urls(&plain, lang);
    let text = expand(&text, lang);
    limit_spoken(text.trim(), options)
}

fn strip_markdown(src: &str, lang: Language) -> String {
    let mut out = String::new();
    // Contents of code blocks and tables are not spoken.
    let mut skipping = false;
    for event in Parser::new_ext(src, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH) {
        match event {
            Event::Start(Tag::CodeBlock(_) | Tag::Table(_)) if !skipping => {
                skipping = true;
                end_block(&mut out);
                // Adjacent blocks say it once.
                if !out.trim_end().ends_with(lang.on_screen()) {
                    out.push_str(lang.on_screen());
                    out.push(' ');
                }
            }
            Event::End(TagEnd::CodeBlock | TagEnd::Table) => skipping = false,
            _ if skipping => {}
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::Item) => {
                end_block(&mut out);
            }
            // Raw HTML, rules, footnotes and the like have no spoken form.
            _ => {}
        }
    }
    out
}

/// Closes a block with a sentence terminator so headings and list items are
/// not glued to the next block, and so the sentence splitter sees a boundary.
fn end_block(out: &mut String) {
    let trimmed = out.trim_end().len();
    out.truncate(trimmed);
    if out.is_empty() {
        return;
    }
    if !out.ends_with(['.', '!', '?', '…', ':', ';']) {
        out.push('.');
    }
    out.push(' ');
}

/// Replaces every `http(s)` URL with its host; one that does not parse is
/// spoken as "link". Trailing sentence punctuation stays outside the URL.
fn shorten_urls(text: &str, lang: Language) -> String {
    text.split_whitespace()
        .map(|word| {
            let Some(start) = word.find("http://").or_else(|| word.find("https://")) else {
                return word.to_owned();
            };
            let (before, tail) = word.split_at(start);
            let url = tail
                .trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\'']);
            let spoken = Url::parse(url)
                .ok()
                .and_then(|u| {
                    u.host_str()
                        .map(|h| h.strip_prefix("www.").unwrap_or(h).to_owned())
                })
                .unwrap_or_else(|| lang.link().to_owned());
            format!("{before}{spoken}{}", &tail[url.len()..])
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Cuts text over the spoken limit at the last sentence end (or word) and
/// says the rest is on the screen.
fn limit_spoken(text: &str, options: SpeakOptions) -> String {
    let Some((byte, _)) = text.char_indices().nth(options.max_spoken_chars) else {
        return text.to_owned();
    };
    let head = &text[..byte];
    let cut = head
        .rfind(['.', '!', '?', '…'])
        // `…` is three bytes; the cut must land after the whole character.
        .map(|i| i + head[i..].chars().next().map_or(1, char::len_utf8))
        .or_else(|| head.rfind(' '))
        .unwrap_or(0);
    let phrase = options.language.on_screen();
    match head[..cut].trim() {
        "" => phrase.to_owned(),
        spoken => format!("{spoken} {phrase}"),
    }
}
