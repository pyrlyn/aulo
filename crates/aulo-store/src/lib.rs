//! SQLite storage for aulo (`~/.aulo/aulo.db`): bots, chats, messages, tool
//! calls, usage, grants and the audit log. This is the only crate that names Diesel.

mod models;
mod safety;
mod schema;

use std::time::{SystemTime, UNIX_EPOCH};

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use ulid::Generator;

pub use models::{AuditRow, Bot, Chat, Grant, Message, ToolCall, Usage};

use schema::{bots, chats, messages, tool_calls, usage};

/// Migrations are keyed by directory name, so two branches adding one never collide.
const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("cannot open database: {0}")]
    Open(#[from] ConnectionError),
    #[error("database query failed: {0}")]
    Query(#[from] diesel::result::Error),
    // Diesel's boxed migration error is not `Send + Sync + 'static` friendly to wrap, so keep the text.
    #[error("migration failed: {0}")]
    Migrate(String),
}

impl StoreError {
    /// True when a write named a row that does not exist (a foreign-key violation),
    /// so callers can answer "not found" without matching on Diesel types.
    #[must_use]
    pub fn is_missing_reference(&self) -> bool {
        matches!(
            self,
            Self::Query(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::ForeignKeyViolation,
                _
            ))
        )
    }
}

/// One SQLite connection. Methods take `&mut self`; the daemon shares a store
/// behind its own lock rather than this crate hiding one.
pub struct Store {
    conn: SqliteConnection,
    // Monotonic so ids sort in insertion order even within one millisecond; message
    // pagination relies on that.
    ids: Generator,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

impl Store {
    /// Opens (creating if needed) the database at `path` and applies pending migrations.
    pub fn open(path: &str) -> Result<Self, StoreError> {
        let mut conn = SqliteConnection::establish(path)?;
        // Diesel's DSL has no PRAGMA support. `busy_timeout` goes first so the WAL
        // switch waits for a concurrent opener; `foreign_keys` is per connection and
        // off by default, and the ON DELETE CASCADE rules depend on it.
        conn.batch_execute(
            "PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;",
        )?;
        conn.run_pending_migrations(MIGRATIONS)
            .map_err(|e| StoreError::Migrate(e.to_string()))?;
        Ok(Self {
            conn,
            ids: Generator::new(),
        })
    }

    fn next_id(&mut self) -> String {
        match self.ids.generate() {
            Ok(id) => id,
            // Random bits exhausted within one millisecond: ordering is still preserved.
            Err(overflow) => overflow.commit_overflow_random(),
        }
        .to_string()
    }

    pub fn create_bot(&mut self, name: &str) -> Result<Bot, StoreError> {
        let bot = Bot {
            id: self.next_id(),
            name: name.to_owned(),
            created_at: now_ms(),
        };
        diesel::insert_into(bots::table)
            .values(&bot)
            .execute(&mut self.conn)?;
        Ok(bot)
    }

    pub fn list_bots(&mut self) -> Result<Vec<Bot>, StoreError> {
        Ok(bots::table
            .order(bots::id.asc())
            .select(Bot::as_select())
            .load(&mut self.conn)?)
    }

    pub fn create_chat(
        &mut self,
        bot_id: Option<&str>,
        title: &str,
        model: &str,
    ) -> Result<Chat, StoreError> {
        let now = now_ms();
        let chat = Chat {
            id: self.next_id(),
            bot_id: bot_id.map(str::to_owned),
            title: title.to_owned(),
            model: model.to_owned(),
            created_at: now,
            updated_at: now,
        };
        diesel::insert_into(chats::table)
            .values(&chat)
            .execute(&mut self.conn)?;
        Ok(chat)
    }

    pub fn get_chat(&mut self, id: &str) -> Result<Option<Chat>, StoreError> {
        Ok(chats::table
            .find(id)
            .select(Chat::as_select())
            .first(&mut self.conn)
            .optional()?)
    }

    /// Most recently active chats first.
    pub fn list_chats(&mut self, limit: i64, offset: i64) -> Result<Vec<Chat>, StoreError> {
        Ok(chats::table
            .order((chats::updated_at.desc(), chats::id.desc()))
            .limit(limit)
            .offset(offset)
            .select(Chat::as_select())
            .load(&mut self.conn)?)
    }

    /// Keyset pagination in the same order as [`Store::list_chats`]: only chats
    /// strictly after `after` (the `(updated_at, id)` of the previous page's last
    /// chat), optionally of one bot. Offsets would skip or repeat chats as their
    /// `updated_at` changes between pages.
    pub fn list_chats_page(
        &mut self,
        bot_id: Option<&str>,
        after: Option<(i64, &str)>,
        limit: i64,
    ) -> Result<Vec<Chat>, StoreError> {
        let mut q = chats::table.into_boxed();
        if let Some(bot_id) = bot_id {
            q = q.filter(chats::bot_id.eq(bot_id));
        }
        if let Some((updated_at, id)) = after {
            q = q.filter(
                chats::updated_at
                    .lt(updated_at)
                    .or(chats::updated_at.eq(updated_at).and(chats::id.lt(id))),
            );
        }
        Ok(q.order((chats::updated_at.desc(), chats::id.desc()))
            .limit(limit)
            .select(Chat::as_select())
            .load(&mut self.conn)?)
    }

    /// Returns `false` when no chat has that id.
    pub fn rename_chat(&mut self, id: &str, title: &str) -> Result<bool, StoreError> {
        let n = diesel::update(chats::table.find(id))
            .set((chats::title.eq(title), chats::updated_at.eq(now_ms())))
            .execute(&mut self.conn)?;
        Ok(n > 0)
    }

    /// Messages, tool calls and usage rows of the chat go with it (foreign-key cascade).
    pub fn delete_chat(&mut self, id: &str) -> Result<bool, StoreError> {
        let n = diesel::delete(chats::table.find(id)).execute(&mut self.conn)?;
        Ok(n > 0)
    }

    /// Appends a message and bumps the chat's `updated_at` atomically.
    pub fn append_message(
        &mut self,
        chat_id: &str,
        role: &str,
        content: &str,
    ) -> Result<Message, StoreError> {
        let msg = Message {
            id: self.next_id(),
            chat_id: chat_id.to_owned(),
            role: role.to_owned(),
            content: content.to_owned(),
            created_at: now_ms(),
        };
        self.conn
            .transaction::<_, diesel::result::Error, _>(|conn| {
                diesel::insert_into(messages::table)
                    .values(&msg)
                    .execute(conn)?;
                diesel::update(chats::table.find(chat_id))
                    .set(chats::updated_at.eq(msg.created_at))
                    .execute(conn)?;
                Ok(())
            })?;
        Ok(msg)
    }

    /// Keyset pagination: oldest first, only messages after `after` (the last id of
    /// the previous page). Offsets would skip or repeat rows while a chat is growing.
    pub fn list_messages(
        &mut self,
        chat_id: &str,
        after: Option<&str>,
        limit: i64,
    ) -> Result<Vec<Message>, StoreError> {
        let mut q = messages::table
            .filter(messages::chat_id.eq(chat_id))
            .into_boxed();
        if let Some(after) = after {
            q = q.filter(messages::id.gt(after));
        }
        Ok(q.order(messages::id.asc())
            .limit(limit)
            .select(Message::as_select())
            .load(&mut self.conn)?)
    }

    pub fn record_tool_call(
        &mut self,
        message_id: &str,
        name: &str,
        arguments: &str,
        output: Option<&str>,
        status: &str,
    ) -> Result<ToolCall, StoreError> {
        let call = ToolCall {
            id: self.next_id(),
            message_id: message_id.to_owned(),
            name: name.to_owned(),
            arguments: arguments.to_owned(),
            output: output.map(str::to_owned),
            status: status.to_owned(),
            created_at: now_ms(),
        };
        diesel::insert_into(tool_calls::table)
            .values(&call)
            .execute(&mut self.conn)?;
        Ok(call)
    }

    /// Tool calls of several messages in one query, oldest first, so a page of
    /// messages costs one round trip instead of one per message.
    pub fn list_tool_calls_for(
        &mut self,
        message_ids: &[String],
    ) -> Result<Vec<ToolCall>, StoreError> {
        Ok(tool_calls::table
            .filter(tool_calls::message_id.eq_any(message_ids))
            .order(tool_calls::id.asc())
            .select(ToolCall::as_select())
            .load(&mut self.conn)?)
    }

    pub fn list_tool_calls(&mut self, message_id: &str) -> Result<Vec<ToolCall>, StoreError> {
        Ok(tool_calls::table
            .filter(tool_calls::message_id.eq(message_id))
            .order(tool_calls::id.asc())
            .select(ToolCall::as_select())
            .load(&mut self.conn)?)
    }

    /// One row per provider call, so a mid-chat model switch stays attributable.
    pub fn record_usage(
        &mut self,
        chat_id: &str,
        provider: &str,
        model: &str,
        input_tokens: i64,
        output_tokens: i64,
    ) -> Result<Usage, StoreError> {
        let row = Usage {
            id: self.next_id(),
            chat_id: chat_id.to_owned(),
            provider: provider.to_owned(),
            model: model.to_owned(),
            input_tokens,
            output_tokens,
            created_at: now_ms(),
        };
        diesel::insert_into(usage::table)
            .values(&row)
            .execute(&mut self.conn)?;
        Ok(row)
    }

    pub fn list_usage(&mut self, chat_id: &str) -> Result<Vec<Usage>, StoreError> {
        Ok(usage::table
            .filter(usage::chat_id.eq(chat_id))
            .order(usage::id.asc())
            .select(Usage::as_select())
            .load(&mut self.conn)?)
    }
}

fn now_ms() -> i64 {
    // A clock before 1970 is treated as 0 rather than failing every write.
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    i64::try_from(ms).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests;
