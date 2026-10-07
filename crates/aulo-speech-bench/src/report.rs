//! What `aulo bench speech` prints: one row per engine, as a table or JSON.

use std::fmt::Write;

use serde_json::{Value, json};

use crate::measure::Metrics;

const NOTE_WIDTH: usize = 60;
const EMPTY: &str = "-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Stt,
    Tts,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stt => "stt",
            Self::Tts => "tts",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Measured(Metrics),
    /// The engine cannot run here: no model, no key, no platform API.
    Unavailable(String),
    /// The engine started and then broke on a clip.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub kind: Kind,
    pub engine: String,
    pub outcome: Outcome,
}

impl Row {
    fn cells(&self) -> [String; 6] {
        let [mut wer, mut rtf, mut first] = [EMPTY.to_owned(), EMPTY.to_owned(), EMPTY.to_owned()];
        let note = match &self.outcome {
            Outcome::Measured(m) => {
                wer = m
                    .wer
                    .map_or(EMPTY.to_owned(), |w| format!("{:.1}%", w * 100.0));
                rtf = format!("{:.2}", m.rtf);
                first = m.first.map_or(EMPTY.to_owned(), |d| {
                    format!("{:.0}", d.as_secs_f64() * 1e3)
                });
                match m.skipped {
                    0 => format!("{} clips", m.clips),
                    n => format!("{} clips, {n} skipped", m.clips),
                }
            }
            Outcome::Unavailable(why) => format!("unavailable: {}", shorten(why)),
            Outcome::Failed(why) => format!("failed: {}", shorten(why)),
        };
        [
            self.kind.as_str().to_owned(),
            self.engine.clone(),
            wer,
            rtf,
            first,
            note,
        ]
    }

    fn json(&self) -> Value {
        let mut row = json!({"kind": self.kind.as_str(), "engine": self.engine});
        match &self.outcome {
            Outcome::Measured(m) => row.as_object_mut().map(|o| {
                o.insert("status".into(), "ok".into());
                o.insert("wer".into(), json!(m.wer));
                o.insert("rtf".into(), json!(m.rtf));
                o.insert(
                    "first_ms".into(),
                    json!(m.first.map(|d| d.as_secs_f64() * 1e3)),
                );
                o.insert("clips".into(), m.clips.into());
                o.insert("skipped".into(), m.skipped.into());
            }),
            Outcome::Unavailable(why) | Outcome::Failed(why) => row.as_object_mut().map(|o| {
                let failed = matches!(self.outcome, Outcome::Failed(_));
                o.insert(
                    "status".into(),
                    if failed { "failed" } else { "unavailable" }.into(),
                );
                o.insert("reason".into(), why.as_str().into());
            }),
        };
        row
    }
}

/// Engine reasons are one line already (`SpeechError` flattens them); the cap
/// keeps a long one from wrapping the table.
fn shorten(reason: &str) -> String {
    match reason.char_indices().nth(NOTE_WIDTH) {
        Some((end, _)) => format!("{}...", &reason[..end]),
        None => reason.to_owned(),
    }
}

pub fn render_table(rows: &[Row]) -> String {
    const HEADER: [&str; 6] = ["kind", "engine", "WER", "RTF", "first ms", "note"];
    let body: Vec<_> = rows.iter().map(Row::cells).collect();
    let mut widths = HEADER.map(str::len);
    for cells in &body {
        for (width, cell) in widths.iter_mut().zip(cells) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for cells in std::iter::once(HEADER.map(str::to_owned)).chain(body) {
        let mut line = String::new();
        for (cell, width) in cells.iter().zip(widths) {
            let _ = write!(line, "{cell:<width$}  ");
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

pub fn render_json(rows: &[Row]) -> String {
    let mut text = Value::Array(rows.iter().map(Row::json).collect()).to_string();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn rows() -> Vec<Row> {
        let measured = |kind, engine: &str, wer, rtf, first_ms: Option<u64>, skipped| Row {
            kind,
            engine: engine.to_owned(),
            outcome: Outcome::Measured(Metrics {
                wer,
                rtf,
                first: first_ms.map(Duration::from_millis),
                clips: 5 - skipped,
                skipped,
            }),
        };
        vec![
            measured(Kind::Stt, "sherpa-parakeet", Some(0.0423), 0.0712, None, 0),
            measured(Kind::Stt, "deepgram", Some(0.0), 0.31, Some(412), 1),
            measured(Kind::Tts, "system", None, 0.2999, Some(95), 0),
            Row {
                kind: Kind::Tts,
                engine: "sherpa-kokoro".to_owned(),
                outcome: Outcome::Unavailable(
                    "model `kokoro-multi-lang-v1_0` is not installed".into(),
                ),
            },
            Row {
                kind: Kind::Tts,
                engine: "openai".to_owned(),
                outcome: Outcome::Failed(format!("{} and then some", "x".repeat(70))),
            },
        ]
    }

    #[test]
    fn table_aligns_columns_and_marks_missing_numbers() {
        let expected = "\
kind  engine           WER   RTF   first ms  note
stt   sherpa-parakeet  4.2%  0.07  -         5 clips
stt   deepgram         0.0%  0.31  412       4 clips, 1 skipped
tts   system           -     0.30  95        5 clips
tts   sherpa-kokoro    -     -     -         unavailable: model `kokoro-multi-lang-v1_0` is not installed
tts   openai           -     -     -         failed: xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx...
";
        assert_eq!(render_table(&rows()), expected);
    }

    #[test]
    fn json_has_one_object_per_row() {
        let json: Value = serde_json::from_str(&render_json(&rows())).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 5);
        assert_eq!(json[0]["status"], "ok");
        assert_eq!(json[0]["first_ms"], Value::Null);
        assert_eq!(json[1]["first_ms"], 412.0);
        assert_eq!(json[3]["status"], "unavailable");
        assert_eq!(json[4]["status"], "failed");
        assert!(
            json[4]["reason"]
                .as_str()
                .unwrap()
                .ends_with("and then some")
        );
    }
}
