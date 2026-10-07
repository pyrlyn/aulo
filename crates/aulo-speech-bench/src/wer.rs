//! Word error rate. Written here, not taken from crates.io: the one maintained
//! candidate checked (`rwer` 0.2.2, 215 downloads, five weeks of history on
//! 2026-10-07) pulls in clap, serde_json and a Chinese converter for a
//! 20-line edit distance, and the normalisation below (ё, apostrophes) is
//! ours either way. runa-media has its own `word_error_rate`; the two should
//! become one shared crate (see ideas.md).

use std::ops::{Add, AddAssign};

/// Word-level edit counts of one or more transcripts against their
/// references. Add them up to get the WER of a whole set, which weights long
/// clips more than averaging per-clip rates would.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WordErrors {
    /// Substitutions, insertions and deletions.
    pub errors: usize,
    /// Words in the reference.
    pub words: usize,
}

impl WordErrors {
    /// Case, punctuation and `ё` do not count as errors.
    pub fn between(hypothesis: &str, reference: &str) -> Self {
        let (hyp, reference) = (normalize(hypothesis), normalize(reference));
        let mut row: Vec<usize> = (0..=hyp.len()).collect();
        for (i, want) in reference.iter().enumerate() {
            let mut diagonal = row[0];
            row[0] = i + 1;
            for (j, got) in hyp.iter().enumerate() {
                let substitute = diagonal + usize::from(want != got);
                diagonal = row[j + 1];
                row[j + 1] = substitute.min(row[j] + 1).min(diagonal + 1);
            }
        }
        Self {
            errors: row[hyp.len()],
            words: reference.len(),
        }
    }

    /// `None` for an empty reference, where a rate is undefined.
    pub fn rate(self) -> Option<f64> {
        (self.words > 0).then(|| self.errors as f64 / self.words as f64)
    }
}

impl Add for WordErrors {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            errors: self.errors + other.errors,
            words: self.words + other.words,
        }
    }
}

impl AddAssign for WordErrors {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

fn normalize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .replace('ё', "е")
        .replace('\u{2019}', "'")
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(hypothesis: &str, reference: &str) -> (usize, usize) {
        let w = WordErrors::between(hypothesis, reference);
        (w.errors, w.words)
    }

    #[test]
    fn counts_edits_not_formatting() {
        assert_eq!(counts("Open the door.", "open the DOOR"), (0, 3));
        assert_eq!(counts("open door", "open the door"), (1, 3));
        assert_eq!(counts("open a big door", "open the door"), (2, 3));
        assert_eq!(counts("ещё", "еще"), (0, 1));
        assert_eq!(counts("don\u{2019}t", "don't"), (0, 1));
        assert_eq!(counts("", "open the door"), (3, 3));
    }

    #[test]
    fn set_rate_weights_words_and_empty_reference_has_none() {
        let mut total = WordErrors::default();
        total += WordErrors::between("open door", "open the door");
        total += WordErrors::between("hello", "hello");
        assert_eq!(
            total,
            WordErrors {
                errors: 1,
                words: 4
            }
        );
        assert_eq!(total.rate(), Some(0.25));
        assert_eq!(WordErrors::between("noise", "").rate(), None);
    }
}
