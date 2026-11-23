---
status: in progress
priority: P0
complexity: 3
readiness: 100%
agent: Claude Code / claude-opus-5-5
---

# T3.5. aulo-server: ChatService

Stage: S3 · Area: api · Depends on: T3.3, T2.8 · Blocks: 5 task(s)

Implement chat CRUD and message listing over aulo-store with pagination.

Done when:

- grpc integration tests for every RPC

Execution plan:

1. **Shared store** (`aulo-server`, nativeMain). `Store` is not thread-safe, so the daemon shares it behind one `Mutex`. Its blocking SQLite calls run on `Dispatchers.IO`.
2. **Page tokens** (`aulo-server`, commonMain).
   - `page_size` 0 means 50, and values above 200 are clamped (`common.proto`).
   - Tokens are opaque base64url strings. They hold the keyset cursors aulo-store already pages by: `(updatedAt, id)` for chats, the last id for messages, and `(rank, id)` for search, with the rank as its exact bits.
   - A token that is malformed or over its cap is INVALID_ARGUMENT, and nothing of it is echoed.
3. **ChatService** (`aulo-server`, nativeMain), implementing Wire's `ChatServiceServer` over the shared store. It is bound through the generated `grpcHandlers()`.
   - Every id is checked as a ULID (`aulo-types`); a bad id is INVALID_ARGUMENT, and a missing chat or bot is NOT_FOUND.
   - Titles must be at most 200 bytes, and a rename must be non-empty. A search query must be at most 1 KiB. A `ModelRef` provider must be 1–64 bytes with no `/`, and its model 1–128 bytes.
   - The store keeps the model as `provider/model`, split at the first `/`.
   - CreateChat without a model takes the config's `tiers.main`. If neither is set, the call fails with FAILED_PRECONDITION.
   - Messages carry their tool calls, fetched in one query per page. Tool output is capped at 64 KiB.
   - Roles and statuses map from the store's lowercase strings; an unknown one is UNSPECIFIED.
   - Per-token scopes (`read` vs `chat`) wait for the "API token management" roadmap item. Today every token is `admin`.
4. **Tests** (`aulo-server` nativeTest). Every RPC runs over the real transport on the local socket, against a temporary database. The tests cover:
   - paging through more than one page with no gaps or repeats;
   - the page-size clamp;
   - bad ids, tokens, titles and queries;
   - NOT_FOUND cases;
   - delete cascading to messages;
   - search with a chat filter;
   - the tool-output cap.
5. **Verify.** Tests and ktlint on macOS, test binaries in Colima on linuxX64 and linuxArm64, CI dispatch for the cross links.
