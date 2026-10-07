//! Streaming behaviour: sentences come out as soon as they are certain, no
//! matter how the text is cut into deltas.

use aulo_voice::{Language, SentenceSplitter, SpeakOptions, SpeakableStream, speakable};
use insta::assert_snapshot;

fn split_by(text: &str, chunk: usize, max_chunk: usize) -> Vec<String> {
    let mut splitter = SentenceSplitter::new(max_chunk);
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = chars
        .chunks(chunk)
        .flat_map(|piece| splitter.push(&piece.iter().collect::<String>()))
        .collect();
    out.extend(splitter.finish());
    out
}

#[test]
fn splits_the_same_way_for_any_delta_size() {
    let text = "Hello there. How are you? Fine! Dr. Smith said 3.14 is pi. \
                Ну что… Привет, мир! J. Doe (a \"quote.\") left. Last one";
    let whole = split_by(text, usize::MAX, 500);
    assert_eq!(
        whole,
        [
            "Hello there.",
            "How are you?",
            "Fine!",
            "Dr. Smith said 3.14 is pi.",
            "Ну что…",
            "Привет, мир!",
            "J. Doe (a \"quote.\")",
            "left.",
            "Last one",
        ]
    );
    for chunk in [1, 2, 3, 7] {
        assert_eq!(split_by(text, chunk, 500), whole, "delta size {chunk}");
    }
}

#[test]
fn a_terminator_at_the_end_waits_for_the_next_delta() {
    let mut splitter = SentenceSplitter::new(100);
    assert!(splitter.push("Pi is 3.").is_empty());
    assert!(splitter.push("14 roughly.").is_empty());
    assert_eq!(splitter.push(" Next"), ["Pi is 3.14 roughly."]);
    assert_eq!(splitter.finish().as_deref(), Some("Next"));
    assert_eq!(splitter.finish(), None);
}

#[test]
fn run_ons_are_cut_at_a_space_and_long_tokens_hard() {
    let words = "word ".repeat(40);
    let chunks = split_by(&words, 9, 30);
    assert!(chunks.len() > 5);
    assert!(chunks.iter().all(|c| c.chars().count() <= 31), "{chunks:?}");
    assert_eq!(chunks.join(" "), words.trim());

    let token = "x".repeat(100);
    let chunks = split_by(&token, 100, 30);
    assert!(chunks.iter().all(|c| c.chars().count() <= 31), "{chunks:?}");
    assert_eq!(chunks.concat(), token);
}

const REPLY: &str = "# Result\n\nI ran the tests. All **42** passed.\n\n```text\nok\nok\n```\n\n\
                     ```text\nmore\n```\n\nSee https://example.com/report for details";

#[test]
fn stream_matches_the_pure_normalizer_and_starts_early() {
    let options = SpeakOptions::new(Language::En);
    let mut stream = SpeakableStream::new(options);
    let mut first_at = None;
    let mut sentences = Vec::new();
    for (i, delta) in REPLY.as_bytes().chunks(5).enumerate() {
        let ready = stream.push(&String::from_utf8_lossy(delta));
        if first_at.is_none() && !ready.is_empty() {
            first_at = Some(i);
        }
        sentences.extend(ready);
    }
    sentences.extend(stream.finish());
    // The first sentence is out long before the reply ends.
    assert!(first_at.is_some_and(|i| i < 8), "{first_at:?}");
    assert_snapshot!(sentences.join("\n"));
    // Block-wise streaming says what the whole-text call says.
    assert_eq!(sentences.join(" "), speakable(REPLY, options));
}

#[test]
fn stream_ignores_input_past_the_cap_and_fences_hold_blank_lines() {
    let options = SpeakOptions {
        max_input_bytes: 20,
        ..SpeakOptions::new(Language::En)
    };
    let mut stream = SpeakableStream::new(options);
    let mut out = stream.push("Short one. And a very long second sentence.\n\n");
    out.extend(stream.push("ignored.\n\n"));
    out.extend(stream.finish());
    assert_eq!(out, ["Short one.", "And a ver."]);

    let mut stream = SpeakableStream::new(SpeakOptions::new(Language::Ru));
    let mut out = stream.push("```\na\n\nb\n```\n\nГотово.\n\n");
    out.extend(stream.finish());
    assert_eq!(out, ["Это на экране.", "Готово."]);
}
