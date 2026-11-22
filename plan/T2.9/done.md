# T2.9. Done

Grants and the audit log in `:aulo-store`. PR: https://github.com/pyrlyn/aulo/pull/94, stacked on #93.

- **Migration `2026-10-07-000100_grants_audit`**, ported from the Rust store:
  - `grants` (subject, scope, decision, nullable `expires_at`), indexed on `(subject, scope)`;
  - `audit` with an AUTOINCREMENT `seq` and a UNIQUE `prev_hash`, so a second writer that read the same chain head fails instead of forking the chain.
- **Queries.** `Grants.sq` and `Audit.sq` declare the same tables. The drift test covers them.
- **Grant methods on `Store`:**
  - `putGrant` adds a row for a repeated subject and scope, so the history of decisions stays listable;
  - `listGrants` and `listGrantsPage` (keyset by id);
  - `listActiveGrants`, whose expiry filter runs in the query against the store's clock;
  - `revokeGrant`.
- **Audit methods on `Store`:**
  - `appendAudit` uses `INSERT … RETURNING`;
  - `lastAudit` and `listAuditAfter`.
  - There is no update or delete.
- **Scope.** The store only persists. aulo-policy computes the hashes and verifies the chain (T5.9).

How it was checked:

- `./gradlew test ktlintCheck checkModuleGraph` passes on macOS arm64. aulo-store has 32 tests:
  - `SafetyTest` (6): put, list, page and revoke; history for a repeated subject and scope; active grants by subject and expiry (exclusive at the expiry millisecond); audit seq order and pages; a forked chain refused; rows surviving a reopen.
  - `MigrationsTest` reverts and reapplies both migrations.
- The linuxArm64 store test binary passed in Ubuntu 24.04 on Colima: 32 tests.
- The mingwX64 store test binary cross-links, but has not been run.
- CI on the branch built, linked and smoke-tested all four targets: https://github.com/pyrlyn/aulo/actions/runs/45080685489. The org variable `SKIP_CI_TESTS=true` skipped tests there.
