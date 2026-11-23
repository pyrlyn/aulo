# aulo-chat

A chat client for aulo on Kotlin Multiplatform and Compose Multiplatform (Android, iOS,
desktop), plus a development server that stands in for `aulod`.

This is a working skeleton: one screen, the chat, against a stub model. You type a message,
the answer streams in token by token, you can stop it, switch the model in the middle of the
chat, stage attachments, and read your recent chats when the server is gone.

Client and server speak the `aulo.v1` gRPC contract from [`../proto`](../proto/aulo/v1)
unchanged (aulo's API is gRPC, spec §17 D6): tokens arrive on the `SessionService.Converse`
stream, not over SSE or WebSocket. When `aulod` serves that contract, the client connects to
it instead of the development server.

## Build and run

You need JDK 21. With [mise](https://mise.jdx.dev/) it comes from the repository's
`mise.toml`: put `mise x --` in front of each `./gradlew` below, or run `mise install` once
and use a shell with mise activated. All commands run from this directory (`aulo-chat/`).

```bash
./gradlew :server:run                 # development server on port 50051
./gradlew :composeApp:run             # desktop app (macOS, Windows, Linux)
./gradlew :androidApp:installDebug    # Android app onto a running emulator or device
./gradlew :server:test :shared:jvmTest :composeApp:jvmTest   # all tests
```

Start the server first, then a client. A client started without a server shows the offline
state; start the server and press Retry.

Dependency versions are locked: `gradle.lockfile` in each module and
`buildscript-gradle.lockfile` for the plugins. `gradle/verification-metadata.xml` holds the
SHA-256 of every file the build downloads. A build fails when it resolves a version the lock
does not name or a file whose checksum is not listed, so after changing a dependency or a
plugin regenerate both on macOS (the iOS configurations exist only there):

```bash
./gradlew resolveAndLockAll --write-locks
./gradlew --write-verification-metadata sha256 help :composeApp:jvmTest :shared:jvmTest :server:test \
  :server:installDist :androidApp:assembleDebug :androidApp:assembleRelease \
  :composeApp:linkDebugFrameworkIosSimulatorArm64 :composeApp:packageDistributionForCurrentOS
```

The Compose Desktop, Skiko and Kotlin/Native files for Linux and Windows hosts are trusted by
name in that file instead: their names carry the host, so one host cannot list the others.

### Desktop

`./gradlew :composeApp:run` connects to `127.0.0.1:50051`. Another server:

```bash
AULO_CHAT_HOST=192.168.1.20 AULO_CHAT_PORT=50051 ./gradlew :composeApp:run
```

### Android

Needs the Android SDK (`ANDROID_HOME`, or `sdk.dir` in `local.properties`) with platform
37.1. The server address is fixed at build time; the default `10.0.2.2:50051` is the host
machine as the emulator sees it. For a real device give the machine's address:

```bash
./gradlew :androidApp:installDebug -Paulo.chat.host=192.168.1.20 -Paulo.chat.port=50051
./gradlew :androidApp:assembleDebug   # only build the APK: androidApp/build/outputs/apk/debug/
```

`./gradlew :androidApp:assembleRelease` builds the release APK, shrunk and obfuscated by R8
and unsigned (the project has no signing configuration). kotlinx-rpc finds its generated code
by reflection and ships no shrinker rules, so `androidApp/proguard-rules.pro` keeps its
runtime and the `aulo.v1` contract; no test runs the release build, so after changing
kotlinx-rpc or the contract, run it once against the server.

### iOS

Needs macOS on Apple Silicon and Xcode. The Xcode project is generated from
`iosApp/project.yml` by XcodeGen (pinned in `mise.toml` here), so no `.xcodeproj` is kept in
the repository:

```bash
mise install                                              # XcodeGen
mise x -- xcodegen generate --spec iosApp/project.yml     # writes iosApp/iosApp.xcodeproj
open iosApp/iosApp.xcodeproj                              # then Run on a simulator
```

Or from the command line (replace the simulator name with one you have):

```bash
xcodebuild -project iosApp/iosApp.xcodeproj -scheme iosApp -configuration Debug -destination 'platform=iOS Simulator,name=iPhone 17' -derivedDataPath build/ios-derived build
```

Xcode builds the Kotlin framework itself through a Gradle build phase. The simulator reaches
the server on the Mac at `127.0.0.1:50051`. For a device, or another server, add the build
settings `AULO_CHAT_HOST=192.168.1.20 AULO_CHAT_PORT=50051` to the `xcodebuild` line (or set
them in the target's build settings).

Only the framework, without Xcode: `./gradlew :composeApp:linkDebugFrameworkIosSimulatorArm64`.

### Server

`./gradlew :server:run` listens on port 50051; `AULO_CHAT_PORT` changes it.

> [!WARNING]
> The development server speaks plaintext gRPC without authentication, listens on every
> network interface and keeps chats in memory only. Run it on a machine and a network you
> trust. It is a stand-in for `aulod`, not something to deploy.

What it does:

- **Stub models.** `stub/echo` streams a reply built from your message and the earlier ones
  (so you can see the dialog memory), `stub/slow` does the same slowly (easy to stop),
  `stub/flaky` fails twice before each answer (to see the retries). A message containing
  `/fail` is refused (to see the error state).
- **Request queue.** Two turns reach the model at once, eight more wait in order and are
  told their position; beyond that a turn is refused.
- **Retries.** A temporary model failure is retried up to three attempts with exponential
  backoff, but never after the first token has been sent.
- **Rate limit.** A token bucket per chat: a burst of 5 turns, refilled at 30 per minute.

## Layout

| Module | What is in it |
| --- | --- |
| `proto` | Kotlin code generated by kotlinx-rpc from `../proto/aulo/v1` |
| `shared` | Clean architecture, top to bottom: `presentation` (`ChatViewModel`, `StateFlow<ChatUiState>`), `domain` (models, repository interface, use cases), `data` (gRPC gateway `GrpcChatRemote`, SQLDelight cache `ChatCache`, `DefaultChatRepository`), `di` (Koin modules) |
| `composeApp` | The Compose UI (`ChatScreen`), the dark theme with per-platform tokens, the desktop entry point and the iOS framework |
| `androidApp` | The Android application |
| `iosApp` | The SwiftUI wrapper and the XcodeGen spec |
| `server` | The development server |
| `design` | One Markdown file per designed screen |

The offline cache keeps the 20 most recent chats, with up to 500 messages each, in a SQLite
database (SQLDelight): the app's data directory on desktop (`aulo-chat/aulo-chat.db` under
Application Support, `%LOCALAPPDATA%` or `$XDG_DATA_HOME`), the app's database directory on
Android and iOS.

## Design

Figma: [Pyrlyn UIkit, page "aulo-chat · Screens"](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=49-227)

The page has one section per platform (iOS, Android, macOS, Windows 11, Linux, Web), each
with the same seven screens built from that platform's kit in the same file. Components the
kits lacked (message bubble, composer, typing indicator, attachment chip, model picker and
model row, conversation row, voice recorder, suggestion chip, banner, empty state) were
added to each kit's own page.

Each screen has a file in [`design/`](design) that describes it without the mockup: purpose,
layout, components, states, behaviour on errors and offline, and what the code has. Its
second line links the six Figma frames.

| Mockup (frame name `<Platform> / …`) | Description | In code |
| --- | --- | --- |
| Chat | [design/chat.md](design/chat.md) | `ChatScreen` in `composeApp/src/commonMain/kotlin/aulo/chat/ui/ChatScreen.kt`, with `Composer.kt` and `MessageBubble.kt` |
| Empty state | [design/empty-state.md](design/empty-state.md) | `ChatScreen` when `ChatUiState.empty` (`EmptyState` in `ChatScreen.kt`) |
| Error | [design/error.md](design/error.md) | `ChatScreen` with `ChatUiState.notice` set (`NoticeBanner`) and messages in `MessageState.Failed` or `Interrupted` (`MessageBubble`) |
| Offline | [design/offline.md](design/offline.md) | `ChatScreen` when `ChatUiState.connection` is `ConnectionState.Offline` |
| Model settings | [design/model-settings.md](design/model-settings.md) | A dropdown for now: `ModelPicker` in `ChatScreen.kt`. The sheet and popover are not built |
| Attachments | [design/attachments.md](design/attachments.md) | A dropdown for now: `AttachButton` in `Composer.kt`; staged chips are `AttachmentChip`. The sheet, popover and recorder are not built |
| Conversation list | [design/conversation-list.md](design/conversation-list.md) | Not built. The data is there: `ChatRepository.chats`, `ChatViewModel.newChat` |

So four of the seven mockups are states of the one screen in code, two exist in a reduced
form, and one is design only. `ChatScreen` is a function of `ChatUiState`, so each state can
be rendered and tested from a hand-made value: `composeApp/src/jvmTest/.../ChatScreenTest.kt`
does that, and writes PNGs of the chat in every platform theme and of the empty, error and
offline states to `composeApp/build/renders/`.

Colours, corner radii and fonts of the code come from the platform kits' variables
(`composeApp/.../theme/PlatformTokens.kt`): Android uses the Android kit, iOS the iOS kit,
desktop the macOS, Windows 11 or Linux kit by operating system. The widgets themselves are
Material 3 on every platform, so the code matches the Android mockups closest. There is no
web target yet (see below), so the Web mockups have no code counterpart.

## Tests

- `:server:test` — the rate limiter, the queue, the retry rule and `Converse` round trips.
- `:shared:jvmTest` — the view model over a fake repository, and `EndToEndTest`: the real
  client against the real development server over gRPC (streaming, dialog memory, stop,
  model switch, a refused turn and retry, losing the server and reconnecting).
- `:composeApp:jvmTest` — the chat screen in each state, and the rendered images.

## Limits of the skeleton

- **Attachments.** `UserTurn` in `aulo.v1` has no attachment field. Text files are placed
  inside the message text; images and voice notes are staged and shown but not sent, and the
  app says so. The picker is a stub that returns sample files.
- **No web target.** kotlinx-rpc's gRPC has no JavaScript or WebAssembly build, and a
  browser needs gRPC-Web on the server side.
- **Dark theme only**, English only.
- **No automatic reconnect.** Retry on the offline banner reconnects.
- **Plaintext to the server.** TLS and tokens arrive with `aulod`'s own transport (spec §11).
  On Android the manifest turns clear text off, but gRPC over OkHttp does not consult that
  setting, so the channel to the server is plaintext there too.
- **Model text is plain text.** It is untrusted input, so nothing in it is interpreted;
  markdown rendering is a later, deliberate step.
