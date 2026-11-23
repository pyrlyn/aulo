---
status: in progress
priority: P0
complexity: 3
readiness: 100%
agent: Claude Code / claude-opus-5-5
---

# T2.8. aulo-store: SQLite with chats and messages

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 3 task(s)

~/.aulo/aulo.db, bundled SQLite, WAL, filename-keyed migrations. Tables: bots, chats, messages, tool_calls, usage. Typed queries only; no ad-hoc SQL in callers.

Done when:

- migration up/down tests; CRUD tests for chats and messages

Execution plan:

1. A new `:aulo-store` LIBRARY module with SQLDelight 2.4.1 (the version kotlin-kit's `db-sqldelight` runtime uses). It runs on kotlin-kit's `db-room` on macOS and Linux, and on `db-native` on Windows.
   - It is the only SQLite owner: `pyrlyn.kit:db*` joins the SQLite list in `ModuleGraph`.
2. Migrations live in `aulo-store/migrations/<key>/{up,down}.sql`.
   - They are ported from the Rust `core_tables` and `message_wire` migrations, with the `wire` column folded into the first one.
   - They are embedded by a shared build-logic `EmbedFilesTask`. `aulo-config`'s default TOML moves to the same task.
   - A ledger table `__aulo_migrations` is created by its own SQLDelight schema, so no raw SQL is needed for it.
   - A pending migration runs in one transaction with its ledger row.
   - Applied keys this binary does not know refuse to run.
3. The `.sq` files declare the same schema, with typed queries for bots, chats, messages, tool calls and usage. `Store` ports the Rust API:
   - keyset pages for chats and messages;
   - `appendTurn` in one transaction;
   - cascading deletes;
   - `isMissingReference` on foreign-key errors.
4. IDs come from a monotonic ULID generator added to `aulo-types`, so they sort in insertion order within a millisecond and fit the typed ids.
5. Tests:
   - migration up, down, ordering, failure and unknown-key cases;
   - a drift test comparing the migrated schema with SQLDelight's;
   - CRUD and pagination tests ported from the Rust store.
6. CI and docs:
   - CI installs conan, cmake and ninja through mise, because `test` compiles the mingw target, whose SQLite Conan builds.
   - Update `toolchain.md` and `AGENTS.md`.
7. Verify with `./gradlew test ktlintCheck checkModuleGraph` on macOS, with the linuxArm64 test binaries on Colima, and with a mingwX64 cross-link.
