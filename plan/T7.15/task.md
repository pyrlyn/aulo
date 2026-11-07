# T7.15. Speakable-text normalizer

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 1 task(s)

Turns model text into speech input: strips markdown, replaces code blocks and long output with "it is on the screen", shortens URLs, expands units and numbers per language, splits sentences for streaming TTS, picks the voice by detected language.


Done when:

- insta snapshot tests for en and ru
