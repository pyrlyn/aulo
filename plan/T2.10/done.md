# T2.10. Done

Full-text search over messages in `:aulo-store`. PR: https://github.com/pyrlyn/aulo/pull/95, stacked on #94.

- **Migration `2026-10-07-000200_message_search`**, ported from the Rust store:
  - an external-content FTS5 index over `messages.content`, using `unicode61 remove_diacritics 2` (case and diacritics folded, no English stemming);
  - a `rebuild` for rows that already exist;
  - insert, delete and update triggers.
- **SQL.** The task expected raw SQL for FTS5. SQLDelight 2.4.1 models the FTS5 table, `MATCH` and `bm25()`, so:
  - `MessageSearch.sq` declares them with build-time checks;
  - the only raw FTS5 SQL is the migration file.
- **`Store.searchMessages(query, chatId, after, limit)`:**
  - best matches first;
  - keyset pages by `(rank, id)` through `SearchCursor`;
  - an optional chat filter.
- **User text.**
  - It reaches `MATCH` only through `matchExpression`: every word becomes one quoted phrase, so FTS5 operators are plain words.
  - Capped at `MAX_QUERY_BYTES` (1024 UTF-8 bytes, cut on a character boundary) and 32 terms.

How it was checked:

- `./gradlew test ktlintCheck checkModuleGraph` passes on macOS arm64. aulo-store has 41 tests.
- `SearchTest` has 9 tests:
  - a message found by a word, with case and diacritics folded;
  - every word must match;
  - edited and deleted messages, and deleted chats, leave the index;
  - the chat filter;
  - the migration indexes existing rows;
  - 23 hostile queries are plain words, not errors;
  - the quoting, the CJK-space split and both caps;
  - keyset pages cover every hit once, best first.
- The linuxArm64 store test binary passed in Ubuntu 24.04 on Colima: 41 tests.
- The mingwX64 store test binary cross-links, but has not been run.
- CI on the branch built, linked and smoke-tested all four targets: https://github.com/pyrlyn/aulo/actions/runs/45080687719 (at 1e167f1). The org variable `SKIP_CI_TESTS=true` skipped tests there.
- SonarCloud flagged a literal U+202E in `SearchTest`; 6b464ad replaced it with the escape, a test-only change.
