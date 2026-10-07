//! Validation of everything a client sends: ids, titles, model refs and page
//! tokens. Failures say which field is wrong, never echo the value.

use aulo_proto::aulo::v1::ModelRef;
use aulo_types::{BotId, ChatId};
use tonic::Status;

const MAX_TITLE_BYTES: usize = 200;
const MAX_PROVIDER_BYTES: usize = 64;
const MAX_MODEL_BYTES: usize = 128;
const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 200;
// A real token is under 60 bytes; the cap keeps garbage from being parsed at all.
const MAX_TOKEN_BYTES: usize = 128;
/// Joins provider and model in the store, which keeps one model string per chat.
pub(super) const MODEL_SEPARATOR: char = '/';

fn invalid(msg: &'static str) -> Status {
    Status::invalid_argument(msg)
}

// Parsed and re-printed so a lowercase id still finds the row stored in canonical form.
pub(super) fn chat_id(raw: &str) -> Result<String, Status> {
    raw.parse::<ChatId>()
        .map(|id| id.to_string())
        .map_err(|_| invalid("chat_id is not a valid ULID"))
}

pub(super) fn bot_id(raw: &str) -> Result<String, Status> {
    raw.parse::<BotId>()
        .map(|id| id.to_string())
        .map_err(|_| invalid("bot_id is not a valid ULID"))
}

/// Titles are shown in lists, so control characters (newlines, escapes) are refused.
pub(super) fn title(raw: &str, allow_empty: bool) -> Result<String, Status> {
    if raw.len() > MAX_TITLE_BYTES {
        return Err(invalid("title is longer than 200 bytes"));
    }
    if raw.chars().any(char::is_control) {
        return Err(invalid("title contains control characters"));
    }
    if !allow_empty && raw.trim().is_empty() {
        return Err(invalid("title must not be empty"));
    }
    Ok(raw.to_owned())
}

fn model_part(raw: &str, max: usize, name: &'static str) -> Result<(), Status> {
    if raw.is_empty() || raw.len() > max || raw.chars().any(char::is_control) {
        return Err(invalid(name));
    }
    Ok(())
}

/// The store's single model string for a [`ModelRef`].
pub(super) fn model(m: &ModelRef) -> Result<String, Status> {
    model_part(
        &m.provider,
        MAX_PROVIDER_BYTES,
        "model.provider is empty, too long or has control characters",
    )?;
    model_part(
        &m.model,
        MAX_MODEL_BYTES,
        "model.model is empty, too long or has control characters",
    )?;
    if m.provider.contains(MODEL_SEPARATOR) {
        return Err(invalid("model.provider must not contain '/'"));
    }
    Ok(format!("{}{MODEL_SEPARATOR}{}", m.provider, m.model))
}

/// Search text is matched as plain words by the store; here it is only bounded.
pub(super) fn search_query(raw: &str) -> Result<String, Status> {
    if raw.len() > aulo_store::MAX_QUERY_BYTES {
        return Err(invalid("query is longer than 1 KiB"));
    }
    if raw.trim().is_empty() {
        return Err(invalid("query must not be empty"));
    }
    Ok(raw.to_owned())
}

/// Page size 0 means the default; anything above the cap is clamped.
pub(super) fn page_size(requested: u32) -> usize {
    match usize::try_from(requested).unwrap_or(MAX_PAGE_SIZE) {
        0 => DEFAULT_PAGE_SIZE,
        n => n.min(MAX_PAGE_SIZE),
    }
}

/// Rows to fetch: one more than the page, to learn whether a next page exists.
pub(super) fn fetch_limit(size: usize) -> i64 {
    i64::try_from(size.saturating_add(1)).unwrap_or(i64::MAX)
}

/// Drops the probe row and returns the last kept row, whose cursor starts the next page.
pub(super) fn trim_page<T>(rows: &mut Vec<T>, size: usize) -> Option<&T> {
    if rows.len() <= size {
        return None;
    }
    rows.truncate(size);
    rows.last()
}

// Tokens are produced by this server only, so ids must be canonical ULIDs.
fn is_ulid(s: &str) -> bool {
    s.len() == 26
        && s.bytes()
            .all(|b| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&b))
}

fn bad_token() -> Status {
    invalid("page_token is not valid")
}

fn token_body<'a>(token: &'a str, prefix: &str) -> Result<Option<&'a str>, Status> {
    if token.is_empty() {
        return Ok(None);
    }
    if token.len() > MAX_TOKEN_BYTES {
        return Err(bad_token());
    }
    token.strip_prefix(prefix).map(Some).ok_or_else(bad_token)
}

/// Position after a chat in `(updated_at desc, id desc)` order. The prefix keeps
/// a message token from being accepted by ListChats and the reverse.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ChatCursor {
    pub updated_at: i64,
    pub id: String,
}

impl ChatCursor {
    pub(super) fn encode(updated_at: i64, id: &str) -> String {
        format!("c1.{updated_at}.{id}")
    }

    pub(super) fn parse(token: &str) -> Result<Option<Self>, Status> {
        let Some(body) = token_body(token, "c1.")? else {
            return Ok(None);
        };
        let (updated_at, id) = body.split_once('.').ok_or_else(bad_token)?;
        let updated_at = updated_at.parse::<i64>().ok().filter(|n| *n >= 0);
        match updated_at {
            Some(updated_at) if is_ulid(id) => Ok(Some(Self {
                updated_at,
                id: id.to_owned(),
            })),
            _ => Err(bad_token()),
        }
    }
}

/// The id of the last message of the previous page.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct MessageCursor(pub String);

impl MessageCursor {
    pub(super) fn encode(id: &str) -> String {
        format!("m1.{id}")
    }

    pub(super) fn parse(token: &str) -> Result<Option<Self>, Status> {
        match token_body(token, "m1.")? {
            None => Ok(None),
            Some(id) if is_ulid(id) => Ok(Some(Self(id.to_owned()))),
            Some(_) => Err(bad_token()),
        }
    }
}

/// Position after a search hit in `(rank, id)` order, best match first.
#[derive(Debug, PartialEq)]
pub(super) struct SearchCursor {
    pub rank: f64,
    pub id: String,
}

impl SearchCursor {
    pub(super) fn encode(rank: f64, id: &str) -> String {
        // `{:e}` round-trips every finite f64 exactly in a few bytes, unlike plain
        // `{}`, which would print a tiny rank as hundreds of zeros.
        format!("s1.{rank:e}.{id}")
    }

    pub(super) fn parse(token: &str) -> Result<Option<Self>, Status> {
        let Some(body) = token_body(token, "s1.")? else {
            return Ok(None);
        };
        let (rank, id) = body.rsplit_once('.').ok_or_else(bad_token)?;
        // "inf" and "nan" parse as floats but are not ranks this server hands out.
        match rank.parse::<f64>() {
            Ok(rank) if rank.is_finite() && is_ulid(id) => Ok(Some(Self {
                rank,
                id: id.to_owned(),
            })),
            _ => Err(bad_token()),
        }
    }
}
