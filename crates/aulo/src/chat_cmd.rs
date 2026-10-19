//! `aulo chat`: a line-oriented client of `SessionService.Converse`.
//!
//! On a terminal the prompt is rustyline. A pipe (and the trycmd fixtures)
//! reads stdin a line at a time, so a turn is one line and EOF leaves the
//! chat. Ctrl-C during a turn sends Interrupt; a second Ctrl-C leaves. At
//! the prompt, Ctrl-C leaves without sending anything.

use std::io::{self, IsTerminal, Write};
use std::sync::mpsc::{self, Receiver, Sender};

use anyhow::{Context, Result, bail};
use aulo_server::{
    ChatEvent, DaemonClient, MAX_TURN_TEXT_BYTES, MAX_USER_TEXT_BYTES, TurnOutcome, plain_text,
};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use tokio::sync::mpsc::UnboundedSender;

use crate::runtime::{home, runtime};

pub fn run(chat: Option<String>, out: &mut impl Write) -> Result<()> {
    let home = home()?;
    let tty = io::stdin().is_terminal();
    runtime()?.block_on(chat_loop(home, chat, tty, out))
}

async fn chat_loop(
    home: std::path::PathBuf,
    chat: Option<String>,
    tty: bool,
    out: &mut impl Write,
) -> Result<()> {
    let client = DaemonClient::connect(&home).await?;
    let chat_id = match chat {
        Some(id) => id,
        None => client.create_chat().await?,
    };
    if tty {
        eprintln!("chat {chat_id}");
    }
    let mut interrupts = Interrupts::install()?;
    let mut reader = LineReader::spawn(tty)?;
    loop {
        reader.ask()?;
        let line = tokio::select! {
            line = reader.recv() => line?,
            _ = interrupts.recv() => break,
        };
        let Some(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if line.trim() == "/exit" {
            break;
        }
        if line.len() > MAX_USER_TEXT_BYTES {
            bail!("that line is longer than 64 KiB");
        }
        let action = turn(&client, &chat_id, &line, &mut interrupts, out).await?;
        if action == AfterTurn::Quit {
            break;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AfterTurn {
    Continue,
    Quit,
}

async fn turn(
    client: &DaemonClient,
    chat_id: &str,
    text: &str,
    interrupts: &mut Interrupts,
    out: &mut impl Write,
) -> Result<AfterTurn> {
    let mut conversation = client.converse(chat_id).await?;
    conversation.user_turn(text).await?;
    let mut render = Render::default();
    let mut sent_interrupt = false;
    loop {
        tokio::select! {
            event = conversation.next_event() => {
                match event? {
                    None => {
                        if !render.saw_finish {
                            bail!("the conversation ended before the turn finished");
                        }
                        break;
                    }
                    Some(event) => {
                        let finished = render.push(&event, out)?;
                        if finished {
                            break;
                        }
                    }
                }
            }
            _ = interrupts.recv() => {
                if sent_interrupt {
                    return Ok(AfterTurn::Quit);
                }
                conversation.interrupt().await?;
                sent_interrupt = true;
            }
        }
    }
    Ok(AfterTurn::Continue)
}

#[derive(Debug, Default)]
struct Render {
    mid_text: bool,
    text_bytes: usize,
    truncated: bool,
    saw_finish: bool,
}

impl Render {
    /// Renders one event. Returns whether the turn is finished.
    fn push(&mut self, event: &ChatEvent, out: &mut impl Write) -> io::Result<bool> {
        match event {
            ChatEvent::Text(text) => self.write_text(text, out)?,
            ChatEvent::ToolCall {
                name,
                arguments_json,
            } => {
                self.break_line(out)?;
                let name = one_line(name, 128);
                let args = one_line(arguments_json, 16 * 1024);
                let name = if name.is_empty() {
                    "tool".to_owned()
                } else {
                    name
                };
                writeln!(out, "tool {name} {args}")?;
            }
            ChatEvent::ToolOutput(text) => {
                self.break_line(out)?;
                let (text, _) = plain_text(text, 16 * 1024);
                if !text.is_empty() {
                    writeln!(out, "{text}")?;
                }
            }
            ChatEvent::ToolFinished(status) if status != "succeeded" => {
                self.break_line(out)?;
                writeln!(out, "tool {status}")?;
            }
            ChatEvent::ToolFinished(_) => {}
            ChatEvent::Notice(message) => {
                self.break_line(out)?;
                writeln!(out, "notice: {message}")?;
            }
            ChatEvent::Approval {
                tool,
                summary,
                needs_click,
            } => {
                self.break_line(out)?;
                if *needs_click {
                    writeln!(out, "approval required: {tool}: {summary} (needs a click)")?;
                } else {
                    writeln!(out, "approval required: {tool}: {summary}")?;
                }
            }
            ChatEvent::TurnFinished { outcome, error } => {
                self.break_line(out)?;
                match outcome {
                    TurnOutcome::Interrupted => writeln!(out, "interrupted")?,
                    TurnOutcome::Failed => {
                        let message = error.as_deref().unwrap_or("the turn failed");
                        writeln!(out, "error: {message}")?;
                    }
                    TurnOutcome::Completed => {}
                }
                self.saw_finish = true;
                out.flush()?;
                return Ok(true);
            }
        }
        out.flush()?;
        Ok(false)
    }

    fn write_text(&mut self, text: &str, out: &mut impl Write) -> io::Result<()> {
        if self.truncated {
            return Ok(());
        }
        let room = MAX_TURN_TEXT_BYTES.saturating_sub(self.text_bytes);
        let (piece, cut) = plain_text(text, room);
        if !piece.is_empty() {
            out.write_all(piece.as_bytes())?;
            self.text_bytes += piece.len();
            self.mid_text = !piece.ends_with('\n');
        }
        if cut {
            self.truncated = true;
            self.break_line(out)?;
            writeln!(out, "[truncated]")?;
        }
        Ok(())
    }

    fn break_line(&mut self, out: &mut impl Write) -> io::Result<()> {
        if self.mid_text {
            writeln!(out)?;
            self.mid_text = false;
        }
        Ok(())
    }
}

fn one_line(input: &str, max_bytes: usize) -> String {
    plain_text(input, max_bytes).0.replace(['\n', '\t'], " ")
}

enum Line {
    Text(String),
    End,
    Err(String),
}

struct LineReader {
    ask: Sender<()>,
    lines: tokio::sync::mpsc::UnboundedReceiver<Line>,
}

impl LineReader {
    fn spawn(tty: bool) -> Result<Self> {
        let (ask, permits) = mpsc::channel();
        let (tx, lines) = tokio::sync::mpsc::unbounded_channel();
        std::thread::Builder::new()
            .name("aulo-chat".to_owned())
            .spawn(move || read_lines(tty, permits, tx))
            .context("cannot start the input reader")?;
        Ok(Self { ask, lines })
    }

    fn ask(&self) -> Result<()> {
        self.ask.send(()).context("the input reader stopped")
    }

    async fn recv(&mut self) -> Result<Option<String>> {
        match self.lines.recv().await {
            Some(Line::Text(text)) => Ok(Some(text)),
            Some(Line::End) | None => Ok(None),
            Some(Line::Err(message)) => bail!(message),
        }
    }
}

fn read_lines(tty: bool, permits: Receiver<()>, tx: UnboundedSender<Line>) {
    let mut editor = if tty {
        match DefaultEditor::new() {
            Ok(editor) => Some(editor),
            Err(error) => {
                let _ = tx.send(Line::Err(error.to_string()));
                return;
            }
        }
    } else {
        None
    };
    while permits.recv().is_ok() {
        let line = match editor.as_mut() {
            Some(editor) => match editor.readline("> ") {
                Ok(line) => Line::Text(line),
                Err(ReadlineError::Interrupted | ReadlineError::Eof) => Line::End,
                Err(error) => Line::Err(error.to_string()),
            },
            None => read_pipe(),
        };
        let stop = matches!(line, Line::End | Line::Err(_));
        if tx.send(line).is_err() {
            break;
        }
        if stop {
            break;
        }
    }
}

fn read_pipe() -> Line {
    let mut buf = String::new();
    match io::stdin().read_line(&mut buf) {
        Ok(0) => Line::End,
        Ok(_) => {
            if buf.ends_with('\n') {
                buf.pop();
                if buf.ends_with('\r') {
                    buf.pop();
                }
            }
            Line::Text(buf)
        }
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Line::End,
        Err(error) => Line::Err(error.to_string()),
    }
}

struct Interrupts {
    #[cfg(unix)]
    inner: tokio::signal::unix::Signal,
}

impl Interrupts {
    fn install() -> io::Result<Self> {
        #[cfg(unix)]
        {
            Ok(Self {
                inner: tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?,
            })
        }
        #[cfg(windows)]
        {
            Ok(Self {})
        }
    }

    async fn recv(&mut self) {
        #[cfg(unix)]
        {
            self.inner.recv().await;
        }
        #[cfg(windows)]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_all(events: &[ChatEvent]) -> Vec<u8> {
        let mut render = Render::default();
        let mut out = Vec::new();
        for event in events {
            render.push(event, &mut out).unwrap();
        }
        out
    }

    fn finish() -> ChatEvent {
        ChatEvent::TurnFinished {
            outcome: TurnOutcome::Completed,
            error: None,
        }
    }

    #[test]
    fn streamed_text_is_joined_and_closed_with_a_newline() {
        let out = render_all(&[
            ChatEvent::Text("hel".into()),
            ChatEvent::Text("lo".into()),
            finish(),
        ]);
        assert_eq!(out, b"hello\n");
    }

    #[test]
    fn a_tool_call_is_one_plain_line_after_the_text() {
        let out = render_all(&[
            ChatEvent::Text("running a tool".into()),
            ChatEvent::ToolCall {
                name: "shell".into(),
                arguments_json: r#"{"cmd":"ls"}"#.into(),
            },
            ChatEvent::ToolFinished("succeeded".into()),
            finish(),
        ]);
        assert_eq!(out, b"running a tool\ntool shell {\"cmd\":\"ls\"}\n");
    }

    #[test]
    fn terminal_and_bidi_controls_never_reach_stdout() {
        let out = render_all(&[ChatEvent::Text("a\u{1b}[31m\u{202e}red".into()), finish()]);
        assert!(!out.contains(&0x1b));
        assert!(!out.windows(3).any(|bytes| bytes == "\u{202e}".as_bytes()));
        assert_eq!(out, b"a[31mred\n");
    }

    #[test]
    fn a_turn_is_capped() {
        let out = render_all(&[ChatEvent::Text("a".repeat(100_000)), finish()]);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("[truncated]"));
        assert!(text.len() < MAX_TURN_TEXT_BYTES + 32);
        assert!(!text.contains('\u{1b}'));
    }

    #[test]
    fn an_interrupt_is_a_line_of_its_own() {
        let out = render_all(&[
            ChatEvent::Text("...".into()),
            ChatEvent::TurnFinished {
                outcome: TurnOutcome::Interrupted,
                error: None,
            },
        ]);
        assert_eq!(out, b"...\ninterrupted\n");
    }
}
