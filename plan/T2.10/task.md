# T2.10. aulo-store: full-text search over messages

Stage: S2 · Area: infra · Depends on: T2.8 · Blocks: 1 task(s)


FTS5 virtual table over message text. Raw SQL stays in the storage crate with a comment that Diesel cannot model FTS5.

Done when:

- search test finds a message by a word
