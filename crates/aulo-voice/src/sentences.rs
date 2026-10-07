//! Streaming sentence splitter: model deltas in, complete sentences out, so
//! TTS can start on the first sentence while the rest is still generated.

/// Abbreviations whose period does not end a sentence. Only ones that almost
/// never end one: "etc." and "т.д." are left out on purpose.
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "st", "vs", "e.g", "i.e", "г", "гг", "ул", "им", "см", "рис",
    "проф", "т.е",
];

/// Buffers text and releases a sentence once its end is certain: a `.`, `!`,
/// `?` or `…` (plus closing quotes and brackets) followed by whitespace. A
/// terminator at the very end of the buffer is not certain ("3." may become
/// "3.14"), so it waits for the next delta or [`finish`](Self::finish).
///
/// A run-on with no boundary is cut at the last space once it passes
/// `max_chunk` characters, so the buffer never grows past that bound and a
/// long sentence still starts playing.
#[derive(Debug, Clone)]
pub struct SentenceSplitter {
    buf: String,
    max_chunk: usize,
}

impl SentenceSplitter {
    /// `max_chunk` is in characters; values below 20 are raised to 20 so a
    /// chunk can always hold a word.
    pub fn new(max_chunk: usize) -> Self {
        Self {
            buf: String::new(),
            max_chunk: max_chunk.max(20),
        }
    }

    /// Adds a delta and returns the sentences it completed, in order.
    pub fn push(&mut self, delta: &str) -> Vec<String> {
        let mut out = Vec::new();
        for c in delta.chars() {
            // Bound the work per character instead of per delta: a single huge
            // delta must not build a buffer larger than `max_chunk`.
            self.buf.push(c);
            if c.is_whitespace() || self.buf.chars().count() > self.max_chunk {
                self.drain_ready(&mut out);
            }
        }
        out
    }

    /// Ends the text and returns whatever is left as the last sentence.
    pub fn finish(&mut self) -> Option<String> {
        let rest = std::mem::take(&mut self.buf);
        let rest = rest.trim();
        (!rest.is_empty()).then(|| rest.to_owned())
    }

    fn drain_ready(&mut self, out: &mut Vec<String>) {
        while let Some(end) = sentence_end(&self.buf).or_else(|| self.overflow_cut()) {
            let sentence = self.buf[..end].trim();
            if !sentence.is_empty() {
                out.push(sentence.to_owned());
            }
            self.buf.drain(..end);
            let skipped = self.buf.len() - self.buf.trim_start().len();
            self.buf.drain(..skipped);
        }
    }

    /// Where to cut a buffer that passed `max_chunk` without a boundary.
    fn overflow_cut(&self) -> Option<usize> {
        let (limit, _) = self.buf.char_indices().nth(self.max_chunk)?;
        let window = &self.buf[..limit];
        // No space at all (a very long token): hard cut, to keep the bound.
        Some(
            window
                .rfind(char::is_whitespace)
                .filter(|&i| i > 0)
                .unwrap_or(limit),
        )
    }
}

/// End (exclusive byte offset) of the first confirmed sentence in `text`.
fn sentence_end(text: &str) -> Option<usize> {
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if !matches!(c, '.' | '!' | '?' | '…') {
            continue;
        }
        let mut end = i + c.len_utf8();
        while let Some(&(j, q)) = chars.peek() {
            if matches!(q, '"' | '\'' | ')' | ']' | '»' | '”' | '’') {
                end = j + q.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        let confirmed = text[end..].chars().next().is_some_and(char::is_whitespace);
        if confirmed && !(c == '.' && is_abbreviation(&text[..i])) {
            return Some(end);
        }
    }
    None
}

/// Whether the word that ends just before a period is an abbreviation or an
/// initial ("J. Smith"). "I", "A" and "Я" are words that end sentences, so
/// they do not count as initials.
fn is_abbreviation(before: &str) -> bool {
    let word = before
        .rsplit(char::is_whitespace)
        .next()
        .unwrap_or("")
        .trim_start_matches(['(', '"', '\'', '«']);
    let mut letters = word.chars();
    let initial = matches!(
        (letters.next(), letters.next()),
        (Some(c), None) if c.is_uppercase() && !matches!(c, 'I' | 'A' | 'Я')
    );
    initial || ABBREVIATIONS.contains(&word.to_lowercase().as_str())
}
