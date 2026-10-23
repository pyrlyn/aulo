//! Untrusted-content guards (spec §7, T5.12).
//!
//! Web pages, tool output, screenshot text and plugin output are data, not
//! instructions. [`guard`] sanitizes them with `text-sanitize`, caps the
//! bytes, and wraps what remains so a closing marker inside the text cannot
//! end the wrapper. Instruction-like content, or a body cut short by the
//! cap, arms a [`Latch`]. The next consequential action then asks; a read
//! does not.

/// Bytes of the raw input that are scanned. The rest is dropped and the
/// guard fails closed.
pub const MAX_SCAN_BYTES: usize = 256 * 1024;

/// Bytes of sanitized text kept inside the wrapper.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

const END_MARKER: &str = "[/aulo untrusted]";

/// Phrases that mean the text is trying to give the model instructions.
/// Matched after sanitizing, lower-casing, stripping tags and collapsing
/// whitespace, so a zero-width character or a tag in the middle still hits.
const PHRASES: &[&str] = &[
    "ignore previous instructions",
    "ignore all previous instructions",
    "ignore prior instructions",
    "disregard previous instructions",
    "disregard your instructions",
    "forget your instructions",
    "forget all previous instructions",
];

/// Where the untrusted bytes came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A web page.
    WebPage,
    /// Output of a tool.
    ToolOutput,
    /// Text read off a screenshot.
    ScreenshotText,
    /// Output of a plugin.
    PluginOutput,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Self::WebPage => "web-page",
            Self::ToolOutput => "tool-output",
            Self::ScreenshotText => "screenshot-text",
            Self::PluginOutput => "plugin-output",
        }
    }
}

/// Sanitized, wrapped text plus whether it should force an ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guarded {
    /// What a prompt may include. The original bytes are not in here.
    pub text: String,
    /// An instruction-like phrase survived sanitizing.
    pub injection: bool,
    /// The input or the sanitized body was longer than the cap, so the
    /// unread tail is treated as hostile.
    pub truncated: bool,
}

/// A tool action the latch judges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// A read. An injection does not block it.
    Read,
    /// Send, write, delete, pay, or anything else that changes the world.
    Consequential,
}

/// What the latch adds on top of the normal policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The injection guard does not ask.
    Allow,
    /// The next consequential action must ask.
    Ask,
}

/// Remembers that untrusted content asked for the next consequential action.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Latch {
    armed: bool,
}

impl Latch {
    /// A latch that has not seen an injection.
    #[must_use]
    pub fn new() -> Self {
        Self { armed: false }
    }

    /// Whether the next consequential action will ask.
    #[must_use]
    pub fn armed(&self) -> bool {
        self.armed
    }

    /// Arms the latch when `guarded` found an injection or had to truncate.
    pub fn observe(&mut self, guarded: &Guarded) {
        if guarded.injection || guarded.truncated {
            self.armed = true;
        }
    }

    /// The decision for `action`. A consequential ask consumes the latch;
    /// a read leaves it armed.
    #[must_use]
    pub fn decide(&mut self, action: Action) -> Decision {
        if action == Action::Consequential && self.armed {
            self.armed = false;
            Decision::Ask
        } else {
            Decision::Allow
        }
    }
}

/// Sanitizes `raw`, wraps it, and records whether it looks like an injection.
#[must_use]
pub fn guard(source: Source, raw: &str) -> Guarded {
    let truncated_input = raw.len() > MAX_SCAN_BYTES;
    let scanned = prefix_bytes(raw, MAX_SCAN_BYTES);
    let sanitized = match source {
        Source::ToolOutput => text_sanitize::sanitize(scanned),
        Source::WebPage | Source::ScreenshotText | Source::PluginOutput => {
            text_sanitize::sanitize_prose(scanned)
        }
    };
    let truncated_body = sanitized.len() > MAX_BODY_BYTES;
    let body = prefix_bytes(&sanitized, MAX_BODY_BYTES);
    let injection = instruction_like(body);
    Guarded {
        text: wrap(source, &neutralize(body)),
        injection,
        truncated: truncated_input || truncated_body,
    }
}

fn instruction_like(sanitized: &str) -> bool {
    let folded = collapse_space(&decode_spaces(&strip_tags(&sanitized.to_lowercase())));
    PHRASES.iter().any(|phrase| folded.contains(phrase))
}

fn neutralize(body: &str) -> String {
    body.replace(END_MARKER, "[/ aulo untrusted]")
}

fn wrap(source: Source, body: &str) -> String {
    let mut text = String::with_capacity(body.len() + 64);
    text.push_str("[aulo untrusted ");
    text.push_str(source.label());
    text.push_str("]\n");
    text.push_str(body);
    if !body.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(END_MARKER);
    text
}

/// `s[..max]` on a char boundary, or all of `s` when it is shorter.
fn prefix_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let close = after.chars().take(256).position(|c| c == '>');
        if let Some(at) = close {
            let mut consumed = 0;
            for (i, c) in after.char_indices() {
                if i > at {
                    break;
                }
                consumed = i + c.len_utf8();
                if c == '>' {
                    break;
                }
            }
            out.push(' ');
            rest = &after[consumed..];
        } else {
            out.push('<');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

fn decode_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        if let Some(end) = after.find(';').filter(|end| *end <= 16) {
            let entity = &after[..=end];
            if is_space_entity(entity) {
                out.push(' ');
                rest = &after[end + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &rest[start + 1..];
    }
    out.push_str(rest);
    out
}

fn is_space_entity(entity: &str) -> bool {
    matches!(
        entity.to_ascii_lowercase().as_str(),
        "&nbsp;" | "&#32;" | "&#x20;" | "&#x00020;"
    )
}

fn collapse_space(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending = false;
    for c in s.chars() {
        if c.is_whitespace() {
            pending = true;
            continue;
        }
        if pending && !out.is_empty() {
            out.push(' ');
        }
        pending = false;
        out.push(c);
    }
    out
}
