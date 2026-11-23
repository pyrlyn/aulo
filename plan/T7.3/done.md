# T7.3. Done

PR: https://github.com/pyrlyn/aulo/pull/110 (stacked on #107, T3.9).

## What was done

- New `:aulo-models` module (LIBRARY), ported from the retired Rust `aulo-models` crate (commit a110cf9):
  - `data/models.json` is embedded in the binary. It holds Parakeet TDT 0.6B v3 int8 pinned to a Hugging Face commit (sources in `research.md` 4.14). T7.5 and T7.9 add their models.
  - `Catalog.validate()` refuses unsafe ids and paths, duplicates, a path that is both a file and a directory, non-http base URLs, and missing sizes or hashes. Unknown keys are refused too.
  - `fetchVerified` streams through the kit `http` interface into `<file>.part` and hashes with okio's `HashingSink`:
    - it resumes with `Range` and checks `Content-Range`; a server that ignores the range restarts the file;
    - it refuses an announced oversize and cuts the body off at the pinned size;
    - a hash, size, range or HTTP failure deletes the partial, while a dropped connection keeps it;
    - only a verified file gets its final name.
  - `ModelManager` has `list` (installed by pinned size), `path`, `pull(id, http, onProgress)` and `remove`. Ids that are not plain names are refused without being echoed. One pull or removal per model runs at a time, through a `FileLock`.
- `aulo models ls|pull <ID>|rm <ID>`, with progress on stderr: one line per file and per 5% of the model. `aulo` passes `KtorHttp(MODEL_HTTP_SETTINGS)`: 15 s to connect, 60 s socket idle, no total timeout.
- Docs: an AGENTS.md bullet, `research.md` 4.14, toolchain rows for okio, `pyrlyn.kit:http` and `http-ktor`, and an idea for a durable append in kotlin-kit. `appendSink` cannot fsync, so a verified file is renamed without one; a short file left by a power loss is told apart by size and fetched again.

## How it was checked

- `ktlintCheck` and `checkModuleGraph` pass.
- macOS arm64:
  - `aulo-models`: 25 tests, including a corrupted download rejected with nothing kept, oversize, HTTP errors, resume after a drop, an ignored range, a wrong range, a corrupt partial, truncation, a lock held by another writer, and unsafe ids.
  - `aulo`: all tests, including the new `models_*` fixtures and `ModelsTest`.
- Linux x64 and arm64 on Colima: the `aulo-models` and `aulo` test binaries pass.
- mingwX64: the test binaries and `aulo.exe` link.
- Against Hugging Face, by hand on macOS:
  - `aulo models pull parakeet-tdt-0.6b-v3-int8` downloaded 670.5 MB and verified all four hashes;
  - the encoder, truncated to 100 MB as a `.part`, resumed from 111.8 MB and matched its pin;
  - `aulo models rm` removed the model.
