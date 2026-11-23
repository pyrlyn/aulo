---
status: in progress
priority: P1
complexity: 3
readiness: 100%
agent: Claude Code / claude-opus-5-5
---

# T7.6. STT: OpenAI-compatible transcription endpoint

Stage: S7 · Area: speech · Depends on: T7.1 · Blocks: 0 task(s)

/v1/audio/transcriptions client: works with OpenAI (GPT Transcribe), the local model runner (local server) and other compatible servers. Streaming variant where the server supports it.

Done when:

- wiremock tests for both shapes

## Execution plan

1. Kit: `HttpClient.streamPost`, so a POST answer is read as it arrives and capped while it is read
   (`post` reads the whole body). A kit PR stacked on pyrlyn/packages#9; aulo's CI pin moves to it.
2. New module `aulo-speech-openai` (LIBRARY): `OpenAiStt`, an `SttEngine` that buffers the
   utterance as 16-bit WAV up to a fixed cap, posts it as `multipart/form-data` to
   `<base>/v1/audio/transcriptions` on `finish`, and parses either a JSON body or server-sent
   events (`transcript.text.delta`, `transcript.text.done`), whichever the server answers.
   Deltas arrive as partials, the done event or JSON body as the final.
3. The API key is a secret handle: a `SecretStore` name, loaded only to build the request
   header, never in a message. Server text is capped and flattened before it reaches an
   exception or a transcript.
4. `SttRegistry.registerOpenAi(endpoint, http, secrets)` registers one server (OpenAI, a local
   server, any compatible one) under its own engine id. Wiring into aulod comes with the voice
   pipeline, as for sherpa.
5. Tests against an in-process fake server: both shapes, status errors, a missing key, caps on
   the body, an event and the audio, cancel and stale results.
6. research.md: the endpoint facts with their primary sources.

Check: `./gradlew :aulo-speech-openai:allTests ktlintCheck checkModuleGraph` and every target links.
