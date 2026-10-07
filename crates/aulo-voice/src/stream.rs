//! Raw reply deltas in, speakable sentences out.

use crate::SentenceSplitter;
use crate::speakable::{SpeakOptions, cap_bytes, speakable};

/// Longest chunk handed to TTS in characters; a run-on is cut at a space.
const MAX_SENTENCE_CHARS: usize = 240;
/// A block with no blank line is normalized once it passes this size, so a
/// reply written as one long paragraph still starts speaking.
const MAX_BLOCK_BYTES: usize = 1024;

/// Markdown cannot be stripped delta by delta (a code fence spans many), so
/// text is collected into blocks: a block ends at a blank line outside a code
/// fence. Each block is normalized with [`speakable`] and split into
/// sentences, so TTS starts after the first paragraph, not the whole reply.
#[derive(Debug)]
pub struct SpeakableStream {
    options: SpeakOptions,
    pending: String,
    seen: usize,
    splitter: SentenceSplitter,
    last_was_phrase: bool,
}

impl SpeakableStream {
    pub fn new(options: SpeakOptions) -> Self {
        Self {
            options,
            pending: String::new(),
            seen: 0,
            splitter: SentenceSplitter::new(MAX_SENTENCE_CHARS),
            last_was_phrase: false,
        }
    }

    /// Adds a delta and returns the sentences now ready to speak. Input past
    /// the total cap is ignored, so a runaway stream cannot grow the buffer.
    pub fn push(&mut self, delta: &str) -> Vec<String> {
        let room = self.options.max_input_bytes.saturating_sub(self.seen);
        let delta = cap_bytes(delta, room);
        self.seen += delta.len();
        self.pending.push_str(delta);
        let mut out = Vec::new();
        while let Some(end) = self.block_end() {
            let block: String = self.pending.drain(..end).collect();
            self.emit(&block, &mut out);
        }
        out
    }

    /// Ends the reply and returns the remaining sentences.
    pub fn finish(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        let block = std::mem::take(&mut self.pending);
        self.emit(&block, &mut out);
        out
    }

    /// End of the first complete block in `pending`. A block never starts
    /// inside a fence (it is only cut outside one), so fence state starts closed.
    fn block_end(&self) -> Option<usize> {
        let (mut offset, mut in_fence) = (0, false);
        for line in self.pending.split_inclusive('\n') {
            if !line.ends_with('\n') {
                return None;
            }
            offset += line.len();
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_fence = !in_fence;
            }
            let blank = trimmed.trim().is_empty();
            if !in_fence && (blank || offset >= MAX_BLOCK_BYTES) {
                return Some(offset);
            }
        }
        None
    }

    fn emit(&mut self, block: &str, out: &mut Vec<String>) {
        let text = speakable(block, self.options);
        let phrase = self.options.language.on_screen();
        let mut sentences = self.splitter.push(&text);
        // Blocks end on a terminator; flush so sentences never merge across them.
        sentences.extend(self.splitter.finish());
        for sentence in sentences {
            let is_phrase = sentence == phrase;
            if !(is_phrase && self.last_was_phrase) {
                out.push(sentence);
            }
            self.last_was_phrase = is_phrase;
        }
    }
}
