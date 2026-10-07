// Helpers outside `#[test]` fns unwrap; clippy only exempts the test fns themselves.
#![allow(clippy::unwrap_used)]

use std::io::Write;

use tracing_subscriber::fmt::MakeWriter;

use crate::writer::Scrubbed;

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

#[test]
fn secret_split_across_writes_is_still_masked() {
    let sink = Sink::default();
    let make = Scrubbed(sink.clone());
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
    let make = Scrubbed(sink.clone());
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
    let make = Scrubbed(sink.clone());
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

#[cfg(feature = "otlp")]
#[test]
fn otlp_attributes_are_masked() {
    use opentelemetry::KeyValue;

    let mut attributes = vec![
        KeyValue::new("api_key", "k"),
        KeyValue::new("note", "sent Bearer abcdef0123456789"),
        KeyValue::new("turn", 3_i64),
    ];
    crate::otlp::scrub_attributes(&mut attributes);
    assert_eq!(attributes[0].value.as_str(), "[REDACTED]");
    assert_eq!(attributes[1].value.as_str(), "sent [REDACTED]");
    assert_eq!(attributes[2].value, 3_i64.into());
}
