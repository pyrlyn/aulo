# T7.13. Done

PR: https://github.com/pyrlyn/aulo/pull/120

## What was done

- **The engine.** `OpenAiTts` in `aulo-speech-openai` is a `TtsEngine` over an OpenAI-compatible `/audio/speech` endpoint.
  - An `OpenAiTtsEndpoint` takes its voices from config, with the first as the default, since the API cannot list them.
  - Queued sentences become `response_format=pcm` JSON requests. Text that queued while a request was speaking is batched into the next one. Text over the 4096-character input cap is cut at whitespace.
  - The 24 kHz PCM answer streams into a fixed per-reply `SampleRing`, which `poll` drains without blocking. A full ring pauses the read.
  - `TtsRegistry.registerOpenAi` registers it.
- **Shared code.** The base URL and key checks, the per-request key header and the HTTP error mapping are now shared with `OpenAiStt`.
- **Caps on untrusted input.**
  - error bodies;
  - audio per request;
  - text per reply;
  - stall time;
  - non-PCM answers.
- **Docs.** research.md 4.17 cites the OpenAPI spec and the TTS guide. AGENTS.md describes the engine.

OpenAI shuts its speech models down on 2027-01-06. Its replacement works only through the Realtime API, so after that date this engine serves compatible servers only.

## How it was checked

- **Tests.** `OpenAiTtsTest` has 20 cases against the in-process fake server, the "wiremock test" of the task. They cover:
  - request shape, and audio split at odd bytes;
  - voice by language, and speed;
  - batching and cutting;
  - status, JSON and format errors, and a missing key;
  - the caps, back-pressure, cancel and stall.

  They pass in `:aulo-speech-openai:macosArm64Test` over 3 runs.
- **Lint and module graph.** `ktlintCheck` and `checkModuleGraph` pass.
- **Other targets.** The test binaries compile and link for linuxX64, linuxArm64 and mingwX64.
