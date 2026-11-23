---
status: in progress
priority: P1
complexity: 5
readiness: 95%
agent: Claude Code / claude-fable-5-2
---

# T12.22. aulo-chat: Compose Multiplatform chat client skeleton

Stage: S12 · Area: apps · Depends on: T3.1 · Blocks: 0 task(s)

The creator asked for a multiplatform chat client on Kotlin Multiplatform with Compose
Multiplatform (spec §17 D12): one chat screen with streamed replies, a stop button, chat
memory, model switching, text/voice/image attachments, a dark theme, message animations and
an offline cache of recent chats in SQLDelight. It talks to aulod over gRPC only (D6), using
the `aulo.v1` contract as it is. Until aulod exists (S3, S4), a development server stands in
for it: it serves the same contract with a stub model, a bounded request queue, retries and
rate limiting.

The screens are designed in the Pyrlyn UIkit Figma file with each platform's kit; `aulo-chat/design/`
holds one Markdown file per screen that an agent can implement from without opening Figma.

Done when:

- `aulo-chat/` builds: the desktop app, the Android app, the iOS framework and the dev server
- the chat screen shows tokens as they arrive, stops a running reply, switches models and
  reopens recent chats from the cache when the server is unreachable
- server tests cover the queue, the retries, the rate limiter and a Converse round trip;
  client tests cover the view model over a fake repository
- `aulo-chat/README.md` gives the build commands, the Figma link and the screen-to-code map
- `aulo-chat/design/` has one file per screen, each with its Figma frame link

Execution plan:

1. `spec.md`: record D12 (Compose Multiplatform client, mobile in scope), keep D6; D12
   replaces the native apps of D2 (those apps are not needed). The web target stays open.
2. `aulo-chat/` as its own Gradle build, so the Android Gradle Plugin and Compose stay out of
   the Kotlin/Native daemon build (T2.1): `:proto` (kotlinx-rpc code from `proto/aulo/v1`),
   `:shared` (domain, data, view model), `:composeApp` (UI, desktop entry, iOS framework),
   `:androidApp`, `:server` (dev stand-in for aulod).
3. Server: `SessionService.Converse`, `ChatService` and `ConfigService.ListModels` over an
   in-memory store; token bucket, bounded queue, retry with backoff; tests.
4. Client: gRPC gateway, SQLDelight cache, repository, use cases, `ChatViewModel` with
   `StateFlow`, Koin modules; the chat screen in Material 3 with per-platform tokens taken
   from the UIkit variables.
5. Figma: add the missing chat components to each platform kit, build the seven screens per
   platform on a new page of the UIkit file.
6. `design/*.md`, `README.md`, `toolchain.md` rows.

Verify: `./gradlew :server:test :shared:jvmTest :composeApp:jvmTest` (the shared tests
include the client against the real development server over gRPC),
`./gradlew :androidApp:assembleDebug`, `./gradlew :composeApp:linkDebugFrameworkIosSimulatorArm64`,
an `xcodebuild` of the generated `iosApp` project, and the Android and iOS apps run by hand
against `./gradlew :server:run`.

What is left before closing: the creator's review, the commit and the PR; `done.md` is
written with them.
