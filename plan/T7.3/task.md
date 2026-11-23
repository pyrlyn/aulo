---
status: in progress
priority: P0
complexity: 3
readiness: 10%
agent: Claude Code / claude-opus-5-5
---

# T7.3. Model manager

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 4 task(s)

Download speech models from Hugging Face with pinned SHA-256 (the coding agent whisper-models.json pattern) into ~/.aulo/models, progress events, aulo models pull/ls/rm. Refuse a hash mismatch.

Done when:

- test: corrupted download is rejected

## Execution plan

Ported from the retired Rust `aulo-models` crate (commit a110cf9). Branch `t7.3-models` on top of
`t3.9-service`, so `aulo models` joins the current CLI.

1. New `:aulo-models` module (LIBRARY, nativeMain).
   - `data/models.json` is the committed catalog, embedded with `EmbedFilesTask`. It starts with
     Parakeet TDT 0.6B v3 int8, pinned to a Hugging Face commit; T7.5 and T7.9 add their models.
   - `Catalog`, `Model`, `ModelFile` and `ModelKind` are `@Serializable`; unknown keys are refused.
   - `validate()` checks:
     - ids are safe directory names and unique;
     - every `base_url` is http(s) and ends in `/`;
     - file paths are plain relative names: no `..`, `\`, `:`, NUL or `.part` suffix, and none is
       both a file and a directory;
     - sizes are positive and hashes are lowercase SHA-256.
2. `fetchVerified` streams through the kit `http` interface (`HttpClient.stream`) into
   `<file>.part` and hashes on the way with okio's `HashingSink`. okio is already on the build
   through Wire.
   - A resumed download sends a `Range` request; a server that ignores it restarts the file.
   - The body is cut off at the pinned size. A bad size, a bad hash or an HTTP error deletes the
     partial; a dropped connection keeps it.
   - Only a verified file is renamed to its final name.
3. `ModelManager(home, catalog, http)` installs under `<AULO_HOME>/models`:
   - `list()` reports install state by pinned size;
   - `path(id)`;
   - `pull(id, onProgress)` reports `Progress` callbacks (model, file, bytes and totals);
   - `remove(id)`.
   Errors are a sealed `ModelException`.
4. `aulo models ls | pull <ID> | rm <ID>` in `:aulo`.
   - `:aulo` is the composition root, so it picks `pyrlyn.kit:http-ktor`.
   - `pull` prints the licence before downloading and progress to stderr.
   - Command fixtures cover `ls`, `rm`, an unknown id and help.
5. Tests, with an in-process fake `HttpClient` (no network) that honours `Range`:
   - a corrupted download (same length, one byte flipped) is rejected and leaves nothing;
   - a body over the pinned size is cut off;
   - an HTTP error installs nothing;
   - an interrupted download resumes with `Range`;
   - a server that ignores `Range` restarts the file;
   - a corrupt partial fails the hash and is deleted;
   - installed files are not fetched again;
   - `remove` deletes partials too;
   - unknown and unsafe ids are refused;
   - the catalog validation cases;
   - the embedded catalog is valid and pinned.
6. ktlint, `checkModuleGraph`; tests on macOS and on Linux x64/arm64 (Colima); mingw link. Then
   `AGENTS.md`, `research.md` (catalog sources) and `toolchain.md`; PR with base `t3.9-service`.
