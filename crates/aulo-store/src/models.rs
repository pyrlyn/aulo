//! Row types. Ids are ULID strings (audit rows use a database sequence) and timestamps are Unix milliseconds;
//! `role` and `status` stay plain strings until `aulo-types` owns the enums.

use diesel::prelude::*;

use crate::schema::{audit, bots, chats, grants, messages, tool_calls, usage};

#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Insertable)]
#[diesel(table_name = bots)]
pub struct Bot {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Insertable)]
#[diesel(table_name = chats)]
pub struct Chat {
    pub id: String,
    pub bot_id: Option<String>,
    pub title: String,
    pub model: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Insertable)]
#[diesel(table_name = messages)]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Insertable)]
#[diesel(table_name = tool_calls)]
pub struct ToolCall {
    pub id: String,
    pub message_id: String,
    pub name: String,
    pub arguments: String,
    pub output: Option<String>,
    pub status: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Insertable)]
#[diesel(table_name = usage)]
pub struct Usage {
    pub id: String,
    pub chat_id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable, Insertable)]
#[diesel(table_name = grants)]
pub struct Grant {
    pub id: String,
    pub subject: String,
    pub scope: String,
    pub decision: String,
    /// Unix milliseconds; `None` never expires.
    pub expires_at: Option<i64>,
    pub created_at: i64,
}

/// A stored audit row. `seq` is assigned by the database, so it is absent from [`NewAuditRow`].
#[derive(Debug, Clone, PartialEq, Eq, Queryable, Selectable)]
#[diesel(table_name = audit)]
pub struct AuditRow {
    pub seq: i64,
    pub prev_hash: String,
    pub hash: String,
    pub kind: String,
    pub payload: String,
    pub created_at: i64,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = audit)]
pub(crate) struct NewAuditRow<'a> {
    pub prev_hash: &'a str,
    pub hash: &'a str,
    pub kind: &'a str,
    pub payload: &'a str,
    pub created_at: i64,
}
