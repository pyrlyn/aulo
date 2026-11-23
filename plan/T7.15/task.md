---
status: in progress
priority: P1
complexity: 3
readiness: 10%
agent: Claude Code / claude-opus-5-5
---

# T7.15. Speakable-text normalizer

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 1 task(s)

Turns model text into speech input: strips markdown, replaces code blocks and long output with "it is on the screen", shortens URLs, expands units and numbers per language, splits sentences for streaming TTS, picks the voice by detected language.

Done when:

- insta snapshot tests for en and ru

## Execution plan

Ported from the retired Rust `aulo-voice` normalizer (commit f31589f). Branch `t7.15-speakable` on
top of `t7.2-registry`, because the code joins `:aulo-voice` as the Rust crate did.

1. `org.jetbrains:markdown` (JetBrains' CommonMark/GFM parser, Apache-2.0, all four native
   targets, no dependencies) parses the reply; nothing in the workspace strips markdown for speech.
2. `aulo-voice/.../speak/`:
   - `Language` (en, ru, from a BCP 47 tag), `detectLanguage` (Latin vs Cyrillic letters);
   - `speakable(markdown, SpeakOptions)`: caps the input (64 KiB) and the spoken text (2000
     characters), drops control, zero-width and bidi characters, strips markdown, replaces code
     blocks and tables with "It's on the screen." / "Это на экране.", shortens `http(s)` URLs to
     the host (or "link"), expands decimals, `%` and a table of units with Russian plurals;
   - `SentenceSplitter`: deltas in, sentences out, abbreviation-aware, bounded chunk;
   - `SpeakableStream`: blocks end at a blank line outside a fence, each normalized and split, so
     TTS starts after the first paragraph.
3. Tests: golden snapshots for en and ru under `aulo-voice/tests/snapshots`, rewritten with
   `AULO_UPDATE_FIXTURES=1`; splitter and stream tests; the caps.
4. Docs: AGENTS.md, `research.md` (the parser), `toolchain.md`.

Check: `ktlintCheck`, `checkModuleGraph`, tests on macOS, Linux x64/arm64 (Colima), mingw link.
