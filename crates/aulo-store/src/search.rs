//! Full-text search over message text. The only raw SQL outside migrations: Diesel
//! cannot model an FTS5 virtual table, so the query goes through `sql_query`.
//! The user's text is only ever a bound parameter, never part of the SQL string.

use diesel::prelude::*;
use diesel::sql_types::{BigInt, Double, Nullable, Text};

use crate::{Message, Store, StoreError};

/// Longest user query the store will turn into a MATCH expression; the rest is dropped.
pub const MAX_QUERY_BYTES: usize = 1024;
// Each term is one phrase the index must intersect, so unbounded terms mean unbounded work.
const MAX_TERMS: usize = 32;

/// A matching message and its bm25 rank (lower is a better match).
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub message: Message,
    pub rank: f64,
}

#[derive(QueryableByName)]
struct HitRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    chat_id: String,
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = Text)]
    content: String,
    #[diesel(sql_type = BigInt)]
    created_at: i64,
    #[diesel(sql_type = Double)]
    rnk: f64,
}

impl From<HitRow> for SearchHit {
    fn from(r: HitRow) -> Self {
        Self {
            message: Message {
                id: r.id,
                chat_id: r.chat_id,
                role: r.role,
                content: r.content,
                created_at: r.created_at,
            },
            rank: r.rnk,
        }
    }
}

/// Turns free text into an FTS5 MATCH expression that cannot carry syntax: every
/// whitespace-separated word becomes one quoted phrase (so `OR`, `NEAR(`, `col:`,
/// `*` and `-` are plain words) and the phrases are ANDed. The index tokenizer, not
/// this function, decides what a word is, so queries split exactly as indexed text
/// did. Returns `None` when nothing searchable is left, for example `"` alone.
fn match_expression(query: &str) -> Option<String> {
    let mut end = query.len().min(MAX_QUERY_BYTES);
    while !query.is_char_boundary(end) {
        end -= 1;
    }
    let terms: Vec<String> = query[..end]
        .split_whitespace()
        .map(|w| w.chars().filter(|c| !c.is_control()).collect::<String>())
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .take(MAX_TERMS)
        .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

impl Store {
    /// Best matches first. Keyset pagination in `(rank, id)` order: pass the last
    /// hit's `(rank, id)` as `after`. Ranks depend on corpus statistics, so a message
    /// written between two pages can shift a hit across the boundary; that is
    /// acceptable for search, unlike for history. A query with no searchable word
    /// matches nothing.
    pub fn search_messages(
        &mut self,
        query: &str,
        chat_id: Option<&str>,
        after: Option<(f64, &str)>,
        limit: i64,
    ) -> Result<Vec<SearchHit>, StoreError> {
        let Some(expr) = match_expression(query) else {
            return Ok(Vec::new());
        };
        // The subquery makes `rnk` an ordinary column, so the keyset predicate can use it.
        let rows = diesel::sql_query(
            "SELECT id, chat_id, role, content, created_at, rnk FROM (\
                 SELECT m.id AS id, m.chat_id AS chat_id, m.role AS role, \
                        m.content AS content, m.created_at AS created_at, \
                        message_search.rank AS rnk \
                 FROM message_search JOIN messages m ON m.rowid = message_search.rowid \
                 WHERE message_search MATCH ?1 AND (?2 IS NULL OR m.chat_id = ?2)\
             ) WHERE ?3 IS NULL OR rnk > ?3 OR (rnk = ?3 AND id > ?4) \
             ORDER BY rnk, id LIMIT ?5",
        )
        .bind::<Text, _>(expr)
        .bind::<Nullable<Text>, _>(chat_id)
        .bind::<Nullable<Double>, _>(after.map(|a| a.0))
        .bind::<Text, _>(after.map_or("", |a| a.1))
        .bind::<BigInt, _>(limit)
        .load::<HitRow>(&mut self.conn)?;
        Ok(rows.into_iter().map(SearchHit::from).collect())
    }
}

#[cfg(test)]
mod tests;
