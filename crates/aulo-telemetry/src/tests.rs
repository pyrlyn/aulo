// Helpers outside `#[test]` fns unwrap; clippy only exempts the test fns themselves.
#![allow(clippy::unwrap_used)]

use std::io::Write;

use telemetry_setup::Scrubbed;
use telemetry_setup::redact::Redactor;
use tracing_subscriber::fmt::MakeWriter;

/// A sink that records what reaches it.
#[derive(Clone, Default)]
struct Sink(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl MakeWriter<'_> for Sink {
    type Writer = Sink;

    fn make_writer(&self) -> Sink {
        self.clone()
    }
}

fn output(sink: &Sink) -> String {
    String::from_utf8(sink.0.lock().unwrap().clone()).unwrap()
}

fn scrubbed(sink: &Sink) -> Scrubbed<Sink> {
    // Built-in rules only: these cases are bearer tokens and secret field names, not aulod tokens.
    Scrubbed::new(sink.clone(), Redactor::new::<&str>(&[]).unwrap())
}

#[test]
fn secret_split_across_writes_is_still_masked() {
    let sink = Sink::default();
    let make = scrubbed(&sink);
    let mut writer = make.make_writer();
    writer.write_all(b"auth Bearer abcdef").unwrap();
    writer.write_all(b"ghijkl012345\nnext line\n").unwrap();
    drop(writer);
    let out = output(&sink);
    assert!(!out.contains("abcdefghijkl"), "{out}");
    assert!(out.contains("next line\n"), "{out}");
}

#[test]
fn flush_emits_only_whole_lines() {
    let sink = Sink::default();
    let make = scrubbed(&sink);
    let mut writer = make.make_writer();
    writer.write_all(b"one\npart").unwrap();
    writer.flush().unwrap();
    assert_eq!(output(&sink), "one\n");
    drop(writer);
    assert_eq!(output(&sink), "one\npart");
}

#[test]
fn json_lines_keep_their_shape() {
    let sink = Sink::default();
    let make = scrubbed(&sink);
    let mut writer = make.make_writer();
    writer
        .write_all(b"{\"fields\":{\"api_key\":\"x\",\"n\":1}}\n")
        .unwrap();
    drop(writer);
    assert_eq!(
        output(&sink),
        "{\"fields\":{\"api_key\":\"[REDACTED]\",\"n\":1}}\n"
    );
}
