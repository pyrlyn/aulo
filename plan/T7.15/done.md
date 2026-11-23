# T7.15. Done

PR: https://github.com/pyrlyn/aulo/pull/111 (stacked on `t7.2-registry`).

## What was done

- **`aulo-voice`, package `aulo.voice.speak`**
  - `speakable(markdown, SpeakOptions)`:
    - strips markdown with `org.jetbrains:markdown` 0.7.18 (GFM flavour);
    - says `Language.onScreen` instead of code blocks, tables and math;
    - reads `http(s)` URLs as their host: user info and port are dropped, and an unparsable URL is read as "link";
    - expands decimals, `%` and 24 units, with Russian plural forms;
    - decodes entities and escapes;
    - drops control, bidi and zero-width characters, both before parsing and after decoding;
    - caps input at 64 KiB and speech at 2000 characters.
  - `SentenceSplitter` and `SpeakableStream` release sentences as deltas arrive, so TTS starts after the first paragraph.
  - `detectLanguage` and `Language.fromTag` choose en or ru.
- **Hardening beyond the Rust reference**
  - `limitNesting` keeps at most 16 quote and list markers per line, because the parser is quadratic in nesting: 64 KiB of `>` took about 3.5 minutes before the cap.
  - The AST is walked with an explicit stack, not recursion.
  - The stream scans blocks incrementally.
- **Golden snapshots for en and ru** are in `aulo-voice/tests/snapshots`. `GoldenTest` rewrites them with `AULO_UPDATE_FIXTURES=1`. Every case from the Rust reference gives the same output.
- **Docs**: an `AGENTS.md` bullet, `research.md` 4.15 and a `toolchain.md` row.

## Limits

- Numbers are not spelled out, because engines read digits.
- Dates, times, versions and compound units such as `km/h` stay as written.
- There is no per-sentence language switching.
- Picking the voice from `detectLanguage` is wired in T8.1.

## How it was checked

- `ktlintCheck` and `checkModuleGraph` pass.
- `:aulo-voice:macosArm64Test` passes 30/30.
- The Linux x64 and arm64 test binaries pass 30/30 in `ubuntu:24.04` on Colima.
- The mingwX64 test binary links.
