# T3.5 done

`aulo.v1.ChatService` in aulod: chat CRUD, message history and search over aulo-store, with paging.

## What was done

[pyrlyn/aulo#104](https://github.com/pyrlyn/aulo/pull/104), stacked on T3.4's #103:

- `ChatService` implements Wire's `ChatServiceServer` and is bound with the generated `grpcHandlers()`. It serves CreateChat, ListChats, GetChat, RenameChat, DeleteChat, ListMessages and SearchMessages.
- `SharedStore` lets every service use the daemon's one `Store`, one caller at a time, off the transport threads.
- `Pages.kt` pages as `common.proto` states:
  - `page_size` 0 is 50, and anything above 200 is clamped, including a uint32 that reaches Kotlin as a negative Int.
  - Page tokens are opaque base64url strings of aulo-store's keyset cursors: `(updatedAt, id)` for chats, the last id for messages, `(rank bits, id)` for search.
  - A malformed, oversized or foreign token is INVALID_ARGUMENT, and nothing of it is echoed.
- The limits from the protos:
  - ids must be ULIDs;
  - titles at most 200 bytes, and a rename must be non-empty;
  - a search query at most 1 KiB;
  - a `ModelRef` provider of 1–64 bytes without `/`, and its model 1–128 bytes;
  - tool output capped at 64 KiB in responses.
- Error statuses:
  - a missing chat or bot is NOT_FOUND;
  - CreateChat without a model takes `defaultModel()` (the daemon passes `tiers.main`); with neither it is FAILED_PRECONDITION.

Not done here: per-token scopes (`read` vs `chat`) wait for "API token management" in `roadmap.md`; today the only token is `admin`. The daemon does not serve ChatService yet: aulod opening the store and serving gRPC is T3.6.

## How it was checked

- `ChatServiceTest` (10 tests) runs every RPC over the real transport on the local socket. It covers:
  - paging with no gaps or repeats while chats move between pages;
  - the size clamp;
  - forged tokens and bad arguments;
  - cascade on delete;
  - search across chats and in one chat;
  - the tool-output cap.
- aulo-server's 28 tests pass on macOS, and in Colima on linuxX64 and linuxArm64. ktlint and `checkModuleGraph` pass.
- CI run 45163955931: all four targets build, link and smoke-test. Tests are skipped there while `SKIP_CI_TESTS` is on.
