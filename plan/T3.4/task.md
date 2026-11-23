---
status: in progress
priority: P0
complexity: 4
readiness: 100%
agent: Claude Code / claude-opus-5-5
---

# T3.4. aulo-server: authentication

Stage: S3 · Area: api · Depends on: T3.3 · Blocks: 3 task(s)

Local socket: trust by file owner and 0600 permissions. TCP: bearer token required, compared in constant time, interceptor rejects missing tokens. The first token is generated on first run and stored in the OS keychain.

Done when:

- tests: TCP without token is rejected; UDS from owner is accepted

Execution plan:

The creator decided on 2026-11-23:

- Tokens are stored as hashes. The first token gets scope `admin`. Per-method scopes, and issuing and revoking tokens, are a separate task.
- On Linux the token goes to the Secret Service through the system libsecret, loaded with `dlopen`. Nothing is linked, so no LGPL code ships in aulo. Without libsecret or a running Secret Service, it falls back to a 0600 file.

Steps:

1. **Transport** (`:aulo-grpc`). `ServerCall.endpoint` names the listener a call arrived on.
2. **Local socket** (`:aulo-server`).
   - The socket moves to `<aulo home>/run/aulod.sock`.
   - `run/` must be a real directory (not a symlink) owned by the user; aulod makes it 0700.
   - The socket file is set to 0600.
   - So only the owner can reach the socket, with no window between bind and chmod.
   - On Windows the user profile's ACL does this; there it is compiled only.
3. **Token storage** (`:aulo-store`). A migration and an `api_tokens` table: id, name, scope, SHA-256 hash, created_at.
4. **Secrets** (new `:aulo-secrets`). A `SecretStore` with three backends:
   - macOS: Keychain (Security.framework).
   - Windows: Credential Manager (wincred).
   - Linux: libsecret through `dlopen`, with the 0600 file under `<aulo home>/secrets/` as the fallback.
   - Random bytes come from kotlincrypto `crypto-rand`.
5. **Auth** (`:aulo-server`).
   - Every handler is wrapped. On a TCP listener a call needs `authorization: Bearer <token>`, and the token's SHA-256 is compared in constant time with every stored hash.
   - A missing, malformed, oversized or unknown token gets UNAUTHENTICATED.
   - Calls on the local socket need no token.
   - `ensureFirstToken` creates the first token (scope `admin`) when the store has none. It saves the token to the `SecretStore` and stores only the hash.
6. **Verify.**
   - Tests: TCP without a token, with a wrong token and with an oversized token is rejected; TCP with the token is accepted; the local socket from the owner is accepted; `run/` and the socket get their modes; a symlinked or foreign `run/` is refused.
   - Also tested: the first token is created once; the file fallback is 0600.
   - macOS and Linux (Colima) run the tests. In Colima there is no Secret Service, so the file fallback is what runs there.
   - The real Keychain test runs only with `AULO_KEYCHAIN_TEST` set.
