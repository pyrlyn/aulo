---
status: in progress
priority: P0
complexity: 2
readiness: 90%
agent: Claude Code / claude-opus-5-5
---

# T2.9. aulo-store: grants and audit tables

Stage: S2 · Area: infra · Depends on: T2.8 · Blocks: 2 task(s)

Tables for permission grants (subject, scope, decision, expiry) and the audit log (seq, prev_hash, hash, kind, payload).

Done when:

- migrations and CRUD tests pass

Execution plan:

1. A new migration, `aulo-store/migrations/2026-10-07-000100_grants_audit/`, ported from the Rust store. It has:
   - `grants`, indexed by subject and scope;
   - `audit` with an AUTOINCREMENT `seq` and a UNIQUE `prev_hash`, so a second writer cannot fork the chain.
2. `Grants.sq` and `Audit.sq` declare the same tables, with typed queries. The drift test keeps the two schemas equal.
3. `Store` gets:
   - `putGrant`, `listGrants`, `listGrantsPage` (keyset by id), `listActiveGrants` (expiry filtered in the query against the store's clock) and `revokeGrant`;
   - `appendAudit`, `lastAudit` and `listAuditAfter`.
   The store only persists rows; aulo-policy computes the hashes and verifies the chain (T5.9).
4. Tests, ported from the Rust store:
   - put, list, page and revoke;
   - active grants by subject and expiry;
   - audit rows in seq order, with pages;
   - a forked chain is refused;
   - the migration reverts and reapplies.
   `MigrationsTest` stops assuming a single migration.
5. Verify:
   - macOS: `./gradlew test ktlintCheck checkModuleGraph`;
   - the linuxArm64 store tests on Colima;
   - the mingwX64 test binary cross-links.
