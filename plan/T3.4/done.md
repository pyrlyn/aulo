# T3.4 done

Authentication for aulod's gRPC server: the local socket trusts its owner, TCP needs a bearer token, and the first token lives in the OS keychain.

## What was done

[pyrlyn/aulo#103](https://github.com/pyrlyn/aulo/pull/103), stacked on T3.3's #102:

- `:aulo-grpc`: `ServerCall.endpoint` names the listener a call arrived on.
- `:aulo-server`, local socket:
  - The socket moved to `<aulo home>/run/aulod.sock`.
  - `run/` must be a real directory owned by the user; aulod makes it 0700 and the socket 0600.
  - A symlinked or loose `run/` is refused at start.
- `:aulo-store`: a migration and the `api_tokens` table (id, name, scope, SHA-256 hash, created_at). Only hashes are stored.
- New `:aulo-secrets`: a `SecretStore` with three backends.
  - macOS: Keychain.
  - Windows: Credential Manager.
  - Linux: the system libsecret loaded with `dlopen`, so no LGPL code is linked. Without libsecret or a running Secret Service it falls back to a 0600 file under `<aulo home>/secrets/`.
- `:aulo-server`, auth:
  - Every handler is wrapped. On TCP a call needs exactly one `authorization: Bearer <token>` header of at most 256 characters.
  - The token's SHA-256 is compared in constant time with every stored hash.
  - Every refusal gets the same UNAUTHENTICATED message. Calls on the local socket need no token.
  - `ensureFirstToken` creates the first token (`aulo_` + 32 random bytes, scope `admin`) when the store has none. It saves the secret first, then the hash.
- Random bytes come from kotlincrypto `crypto-rand` 0.6.0 (`getrandom`, `CCRandomGenerateBytes`, `BCryptGenRandom`).
- CI links the `aulo-secrets` test binaries. The Kotlin/Native cache now restores from an older key after a version-catalog change.

Per-method scopes, and issuing and revoking tokens, are in `roadmap.md` ("API token management").

## How it was checked

- macOS arm64 tests pass:
  - aulo-secrets: 11;
  - aulo-store: 42;
  - aulo-grpc: 26;
  - aulo-server: 18, including TCP with no, wrong, oversized and lowercase-scheme tokens, the local socket's modes and the first token made once.
- The same test binaries pass on linuxX64 and linuxArm64 in Colima.
- The real Secret Service test (gnome-keyring under `dbus-run-session`, arm64) passed.
- CI run 45161317091: all four targets build, link and smoke-test. Tests are skipped there while `SKIP_CI_TESTS` is on, so Windows is compiled and linked only.

## Caveat

kotlin-kit 0.0.5's `gradle/conan.gradle.kts` reads `konan.properties` when the task is created, before Kotlin/Native is downloaded. With a cold `~/.konan` this breaks the Windows build. The cache restore-key works around it here; the fix belongs in pyrlyn/packages.
