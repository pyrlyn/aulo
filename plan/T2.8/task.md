# T2.8. aulo-store: SQLite with chats and messages

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 3 task(s)

~/.aulo/aulo.db, bundled SQLite, WAL, filename-keyed migrations. Tables: bots, chats, messages, tool_calls, usage. Typed queries only; no ad-hoc SQL in callers.

Done when:

- migration up/down tests; CRUD tests for chats and messages
