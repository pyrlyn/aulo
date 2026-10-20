//! Plain-text bounds for model and tool output.
//!
//! Text from a model, a tool or a remote client is untrusted. Callers render
//! it as characters, never as terminal controls, and never past the cap the
//! proto documents for that field.

/// A user turn the server accepts. Longer text is rejected, not truncated.
pub const MAX_USER_TEXT_BYTES: usize = 64 * 1024;
/// One streamed assistant delta.
pub const MAX_DELTA_BYTES: usize = 16 * 1024;
/// Assistant text kept from a single turn. The rest is dropped.
pub const MAX_TURN_TEXT_BYTES: usize = 64 * 1024;
/// Tool name on a `ToolCallStarted` event.
pub const MAX_TOOL_NAME_BYTES: usize = 128;
/// Tool arguments and one piece of tool output.
pub const MAX_TOOL_ARGS_BYTES: usize = 16 * 1024;
/// A notice or a turn error shown to a person.
pub const MAX_NOTICE_BYTES: usize = 2 * 1024;

/// Strips terminal and bidi controls and caps `input` at `max_bytes`.
///
/// Newlines and tabs stay, so a multi-line tool result still breaks into
/// lines. Everything else in Unicode class Cc is removed, along with the
/// bidi overrides that would reorder what the person reads. The cut is on a
/// char boundary. The `bool` is true when some non-control character did not
/// fit.
pub fn plain_text(input: &str, max_bytes: usize) -> (String, bool) {
    let mut out = String::new();
    let mut truncated = false;
    for ch in input.chars() {
        if is_hidden(ch) {
            continue;
        }
        let mut buf = [0; 4];
        let encoded = ch.encode_utf8(&mut buf);
        if out.len().saturating_add(encoded.len()) > max_bytes {
            truncated = true;
            break;
        }
        out.push(ch);
    }
    (out, truncated)
}

/// Splits `text` into pieces of at most `max_bytes`, on char boundaries.
pub fn byte_chunks(text: &str, max_bytes: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    if max_bytes == 0 {
        return chunks;
    }
    let mut rest = text;
    while !rest.is_empty() {
        let mut end = max_bytes.min(rest.len());
        while end > 0 && !rest.is_char_boundary(end) {
            end -= 1;
        }
        if end == 0 {
            break;
        }
        chunks.push(rest[..end].to_owned());
        rest = &rest[end..];
    }
    chunks
}

fn is_hidden(ch: char) -> bool {
    if ch == '\n' || ch == '\t' {
        return false;
    }
    ch.is_control() || is_bidi(ch)
}

fn is_bidi(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_strips_terminal_and_bidi_controls() {
        let (text, truncated) = plain_text("a\u{1b}[31m\u{202e}b\r\n\tok", 64);
        assert!(!truncated);
        assert_eq!(text, "a[31mb\n\tok");
        assert!(!text.chars().any(char::is_control) || text.contains('\n'));
        assert!(!text.contains('\u{1b}'));
        assert!(!text.contains('\u{202e}'));
    }

    #[test]
    fn plain_text_caps_on_a_char_boundary() {
        let (text, truncated) = plain_text("ééé", 3);
        assert!(truncated);
        assert_eq!(text, "é");
        assert_eq!(text.len(), 2);
    }

    #[test]
    fn byte_chunks_cover_the_text_without_splitting_a_char() {
        assert_eq!(byte_chunks("hello", 2), ["he", "ll", "o"]);
        let long = "a".repeat(20_000);
        let chunks = byte_chunks(&long, MAX_DELTA_BYTES);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), MAX_DELTA_BYTES);
        assert_eq!(chunks.concat(), long);
        assert!(byte_chunks("é", 1).is_empty());
    }
}
