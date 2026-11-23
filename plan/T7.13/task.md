---
status: in progress
priority: P2
complexity: 3
readiness: 100%
agent: Claude Code / claude-opus-5-5
---

# T7.13. TTS: OpenAI speech endpoint

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

OpenAI-compatible /v1/audio/speech with streaming PCM; voice list from config.

Done when:

- wiremock test

## Execution plan

1. In `aulo-speech-openai` (T7.6), share what the transcription engine already does: the base URL
   and API-key checks of `OpenAiEndpoint`, reading the key from the `SecretStore` per request,
   and turning an error status into a `SpeechException`.
2. `OpenAiTtsEndpoint`: engine id, name, base URL, default model, the voices from config (the
   first is the default) and the API-key secret name; posts to `<base>/audio/speech`.
3. `OpenAiTts`, a `TtsEngine`: text pushed during a reply is queued; one I/O coroutine per reply
   sends it as JSON requests (`response_format=pcm`, `speed` from the rate), batching what queued
   up while the last request was speaking, each within the endpoint's input cap, and streams the
   24 kHz 16-bit PCM answer into a fixed ring buffer that `poll` drains without blocking. A full
   ring makes the reader wait rather than grow. Each reply owns its ring, so a cancelled request
   cannot write into the next reply.
4. Untrusted answers: error bodies, a non-audio content type and endless audio are capped and
   become `SpeechException`s; the text queue per reply is capped too.
5. `TtsRegistry.registerOpenAi(endpoint, http, secrets)`. Wiring into aulod comes with the voice
   pipeline, as for the STT engines.
6. Tests against an in-process fake server: request shape, streamed audio split at odd bytes,
   batching, voice and language checks, status errors, missing key, caps, cancel and back-pressure.
7. research.md: the endpoint facts with their primary sources.

Check: `./gradlew :aulo-speech-openai:allTests ktlintCheck checkModuleGraph` and every target links.
