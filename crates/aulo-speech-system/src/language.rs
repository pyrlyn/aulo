//! Matching a requested language to what a system engine offers, shared by
//! every platform so the synthesizer and the recognizer choose alike.

#![cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]

use std::cmp::Reverse;

const SUBTAG_SEPARATOR: char = '-';

/// The same rule over any `(tag, rank)` list, so the recognizer picks its
/// locale exactly as the synthesizer picks its voice.
pub(crate) fn best_match<'a>(
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
