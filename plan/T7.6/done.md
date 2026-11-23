# T7.6 done

PR: https://github.com/pyrlyn/aulo/pull/117. Its base is `t7.4-sherpa-stt`.
Kit: https://github.com/pyrlyn/packages/pull/10, stacked on #9.

## What was done

- **kit-http** `HttpClient.streamPost`: a POST whose answer is read as it arrives. `post` reads the whole body first, so it can neither stream server-sent events nor cap the answer while reading it. CI pins kit `303aa1f` until #9 and #10 merge.
- **`:aulo-speech-openai`** `OpenAiStt`, an `SttEngine` for `<base>/audio/transcriptions`:
  - The utterance is kept as 16-bit samples, capped at 10 minutes, with drops reported by `push`.
  - `finish` posts it as one multipart WAV. Fields: `model`, `response_format=json`, `language` (or `languages[]` for `gpt-transcribe*`), and `stream=true` when the endpoint asks for it.
  - The answer is a JSON body, or server-sent events where deltas become partials and `transcript.text.done` is the final.
- **`OpenAiEndpoint`** describes one server: engine id, base URL, default model, API key secret name, stream flag and languages. `registerOpenAi` adds it to an `SttRegistry`.
- **Caps**, each with a test:
  - the whole answer (1 MiB);
  - an event line or event (64 KiB);
  - an error body (4 KiB, read no further);
  - the audio (10 minutes);
  - the time to the final (120 s).
- **Secrets and URLs.**
  - The key is a `SecretStore` name, read per request into the header only.
  - Base URLs with credentials, a query or a fragment are refused.
  - A key goes over plain HTTP only to loopback.
- `research.md` 4.16 has the API facts with their sources and the open points marked **unverified**.

## How it was checked

- 29 tests against an in-process fake server, covering the JSON and the stream shapes, errors, caps, cancel and timeout.
  - They pass on macosArm64 and on linuxArm64 (Colima).
  - A mutation of the done-event parsing makes two of them fail.
- `checkModuleGraph` and ktlint pass. The test binaries link for linuxX64, linuxArm64 and mingwX64.
- `:aulo-models` tests pass on the new kit.
- In the kit, `:http:check` and `:http-ktor:check` pass: 13 tests, ktlint, detekt.

## Not done here

- aulod does not register the engine yet. The config for base URL and key secret comes with the voice-pipeline wiring, as for sherpa.
- No request was made to the real OpenAI API.
