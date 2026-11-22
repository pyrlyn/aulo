---
status: in progress
priority: P0
complexity: 2
readiness: 90%
agent: Claude Code / claude-opus-5-5
---

# T2.10. aulo-store: full-text search over messages

Stage: S2 · Area: infra · Depends on: T2.8 · Blocks: 1 task(s)

FTS5 virtual table over message text. Raw SQL for FTS5 stays in the storage module, with a comment that the query layer may not model FTS5.

Done when:

- search test finds a message by a word

Execution plan:

1. A new migration, `aulo-store/migrations/2026-10-07-000200_message_search/`, ported from the Rust store:
   - an external-content FTS5 table over `messages.content`, with the `unicode61 remove_diacritics 2` tokenizer;
   - a `rebuild` step for messages that already exist;
   - insert, delete and update triggers.
2. `MessageSearch.sq` declares the same table and triggers, plus a `search` query: bm25 rank, an optional chat filter, keyset pages by `(rank, id)`.
   - SQLDelight 2.4.1 models FTS5, MATCH and `bm25()`, so the only raw FTS5 SQL is the migration file.
3. `Store.searchMessages(query, chatId, after, limit)` with `SearchHit` and `SearchCursor`.
   - User text only reaches FTS5 through `matchExpression`, which makes every word one quoted phrase.
   - The expression is capped at `MAX_QUERY_BYTES` (1024, UTF-8, cut on a character boundary) and 32 terms.
4. Tests, ported from the Rust store:
   - a word found with case and diacritics folded;
   - every word must match;
   - edited and deleted messages, and deleted chats, follow the index;
   - the chat filter;
   - the migration indexes existing rows;
   - hostile queries are plain words;
   - the expression quoting and caps;
   - pages cover every hit once.
5. Verify:
   - macOS: `./gradlew test ktlintCheck checkModuleGraph`;
   - the linuxArm64 store tests on Colima;
   - the mingwX64 test binary cross-links.
