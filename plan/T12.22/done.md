# T12.22. Done

PR: https://github.com/pyrlyn/aulo/pull/98 (commit `eae2e38`; review follow-ups `b8cc5b5`,
`6db833f`, `d62d052`, `808497a`).

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
- `spec.md`: D12 and §12.1. `AGENTS.md`, the root `README.md`, `ideas.md` and `toolchain.md`
  updated.

After review (SonarQube rated the security of the new code C, on four findings):

- Dependency versions are locked: `gradle.lockfile` in each module and
  `buildscript-gradle.lockfile` for the plugins; `./gradlew resolveAndLockAll --write-locks`
  regenerates them.
- The Android release build is shrunk and obfuscated by R8. `androidApp/proguard-rules.pro`
  holds the keep rules for kotlinx-rpc, which ships none.
- `usesCleartextTraffic="false"` in the Android manifest.
- The bidi override in `ChatScreenTest` is written as an escape.

## How it was checked

- `./gradlew :server:test :shared:jvmTest :composeApp:jvmTest`: server 18 tests, shared 15
  (4 of them run the client against the real development server over gRPC), UI 10; all pass,
  also with the lock files in place.
- `./gradlew :androidApp:assembleDebug`, `:androidApp:assembleRelease` and
  `:composeApp:linkDebugFrameworkIosSimulatorArm64` build; `xcodebuild` of the generated
  `iosApp` project succeeds.
- The Android app on an emulator and the iOS app on a simulator, run by hand against
  `./gradlew :server:run`.
- The release APK, signed with the debug key, on the emulator by hand: it connects, lists the
  models, streams a reply, stops one, and reopens the cached chat with the server gone.
- The desktop runtimes for Linux (x64, arm64) and Windows x64 resolve under the lock; checked
  once from macOS with a temporary dependency on them.

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
- No test runs the release build, so the keep rules are checked by hand only.
- The channel to the development server is still plaintext on Android: gRPC over OkHttp does
  not consult the cleartext setting. TLS comes with aulod's own transport (spec §11).
- The lock files and checksums are written on macOS, where the iOS frameworks link and the
  macOS Kotlin/Native bundle resolves. Whether Renovate
  regenerates them completely was not checked.
