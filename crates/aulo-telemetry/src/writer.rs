//! A `MakeWriter` wrapper that masks secrets in every line before it reaches the sink.

use std::io::{self, Write};

use tracing_subscriber::fmt::MakeWriter;

use crate::redact::scrub_line;

/// Wraps a sink so everything the formatter writes is scrubbed first. It sits between the
/// formatter and the sink, so the scrubbing also happens before a non-blocking file worker
/// thread ever sees the bytes.
#[derive(Debug, Clone)]
pub(crate) struct Scrubbed<M>(pub M);

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for Scrubbed<M> {
    type Writer = ScrubWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        ScrubWriter {
            inner: self.0.make_writer(),
            pending: Vec::new(),
        }
    }
}

/// Buffers one event and scrubs it line by line. Buffering matters: the formatter may write an
/// event in several pieces, and a secret split across two writes would escape a per-write scrub.
#[derive(Debug)]
pub(crate) struct ScrubWriter<W: Write> {
    inner: W,
    pending: Vec<u8>,
}

impl<W: Write> ScrubWriter<W> {
    /// Scrubs and writes the first `upto` buffered bytes.
    fn emit(&mut self, upto: usize) -> io::Result<()> {
        let ready: Vec<u8> = self.pending.drain(..upto).collect();
        for chunk in ready.split_inclusive(|byte| *byte == b'\n') {
            let (body, newline) = match chunk.strip_suffix(b"\n") {
                Some(body) => (body, true),
                None => (chunk, false),
            };
            let mut line = scrub_line(&String::from_utf8_lossy(body));
            if newline {
                line.push('\n');
            }
            self.inner.write_all(line.as_bytes())?;
        }
        Ok(())
    }
}

impl<W: Write> Write for ScrubWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Only whole lines: scrubbing half a JSON record would miss its structure.
        if let Some(end) = self.pending.iter().rposition(|byte| *byte == b'\n') {
            self.emit(end + 1)?;
        }
        self.inner.flush()
    }
}

impl<W: Write> Drop for ScrubWriter<W> {
    fn drop(&mut self) {
        // A failed log write has nowhere to be reported; losing a line beats panicking in drop.
        let _ = self.emit(self.pending.len());
    }
}
