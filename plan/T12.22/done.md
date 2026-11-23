# T12.22. Done

PR: https://github.com/pyrlyn/aulo/pull/98 (commit `eae2e38`).

## What was done

- `aulo-chat/`, its own Gradle build: `:proto` (kotlinx-rpc code from `proto/aulo/v1`),
  `:shared` (domain, data, view model), `:composeApp` (UI, desktop entry, iOS framework),
  `:androidApp`, `:server`, and an XcodeGen spec for the iOS app.
- Client for Android, iOS and desktop: one chat screen with streamed replies, a stop button,
  model switching, staged attachments, a dark theme and message animations; the 20 most recent
  chats are cached in SQLDelight and reopen when the server is unreachable.
- Development server standing in for aulod: `SessionService.Converse`, `ChatService` and
  `ConfigService.ListModels` over an in-memory store, with a token bucket, a bounded request
  queue and retries with backoff.
- Figma: the missing chat components added to each platform kit of the Pyrlyn UIkit file, and
  seven screens for six platforms (iOS, Android, macOS, Windows 11, Linux, Web) on the page
  `aulo-chat · Screens`.
- `aulo-chat/design/`: one Markdown file per screen with its Figma frames; `aulo-chat/README.md`
  with the build commands, the Figma link and the mockup-to-code map.
- `spec.md`: D10 and §12.1. `AGENTS.md`, the root `README.md`, `ideas.md` and `toolchain.md`
  updated.

## How it was checked

- `./gradlew :server:test :shared:jvmTest :composeApp:jvmTest`: server 18 tests, shared 15
  (4 of them run the client against the real development server over gRPC), UI 10; all pass.
- `./gradlew :androidApp:assembleDebug` and `:composeApp:linkDebugFrameworkIosSimulatorArm64`
  build; `xcodebuild` of the generated `iosApp` project succeeds.
- The Android app on an emulator and the iOS app on a simulator, run by hand against
  `./gradlew :server:run`.

## Not done, and where it went

- Not checked: the desktop window run by hand, real devices, packaging.
- Images and voice notes are staged but not sent, because `UserTurn` has no attachment field;
  the attachment picker returns sample files. Both are in `ideas.md`; attachments in a turn is
  open in spec §17.
- Only the chat screen is built; the other six screens are mockups and `design/*.md`
  (`ideas.md`).
- No web target in code (open in spec §17).
- The `iosApp` link step warns that the Skiko ICU object is built for iOS simulator 18.5 while
  the deployment target is 16.0.
- `gradlew.bat` is stored with LF line endings, as the root `.gitattributes` sets `eol=lf`.
