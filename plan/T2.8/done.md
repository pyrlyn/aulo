# T2.8. Done

`:aulo-store`: SQLite with bots, chats, messages, tool calls and usage. PR: https://github.com/pyrlyn/aulo/pull/93, stacked on #92.

- **SQL access.**
  - SQLDelight 2.4.1 checks every query against the schema at build time. It is the version pyrlyn-kit's `db-sqldelight` runtime is built against.
  - SQLite comes from pyrlyn-kit: `db-room` (androidx `sqlite-bundled` 2.7.1) on macOS and Linux, and `db-native` (Conan-built SQLite 3.54.0) on Windows.
  - `checkModuleGraph` counts those adapters as SQLite, so `:aulo-store` stays the only owner.
- **Migrations.**
  - Filename-keyed `migrations/<key>/{up,down}.sql`, embedded by a new shared build-logic `EmbedFilesTask`. aulo-config's default TOML moved to the same task.
  - The first migration ports the Rust `core_tables`, with `message_wire`'s `wire` column folded in.
  - The `__aulo_migrations` ledger has its own SQLDelight schema.
  - Each pending migration runs in one transaction with its ledger row, in key order.
  - Applied keys this binary does not know make the store refuse to migrate.
- **Typed API.**
  - `.sq` files declare the same schema with typed queries. A drift test compares the migrated database with `AuloDatabase.Schema.create`: columns, foreign keys and indexes.
  - `Store` ports the Rust API: keyset pages for chats and messages, `appendTurn` in one transaction, and the cascade on chat delete.
  - A failed write that names a missing row is reported as `isMissingReference`.
- **IDs.** `MonotonicUlid` in aulo-types keeps ids in insertion order within one millisecond.
- **Build.**
  - Google Maven was added, filtered to `androidx.*`.
  - The default hierarchy gained a `unix` group.
- **Tooling.**
  - `mise.toml` pins conan, cmake, ninja and uv. Every `test` run compiles the mingw target, which runs Conan.
  - The CI build job installs those tools, with Python, which mise's Conan needs.

How it was checked:

- `./gradlew test ktlintCheck checkModuleGraph :build-logic:test` passes on macOS arm64:
  - aulo-store: 26 tests;
  - aulo-types: 14 tests, including the monotonic ids;
  - aulo-config: 22, aulo-telemetry: 15, aulo: 4, build-logic: 11.
- The drift test was mutation-checked: renaming one index in a migration makes it fail.
- The linuxArm64 test binaries passed in Ubuntu 24.04 on Colima, with the same counts.
- The mingwX64 store test binary cross-links, but has not been run.
- CI on the branch built, linked and smoke-tested all four targets: https://github.com/pyrlyn/aulo/actions/runs/45080157582. The org variable `SKIP_CI_TESTS=true` skipped tests there.
