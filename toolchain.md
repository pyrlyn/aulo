# Toolchain

Only what the project uses today. Libraries are added by the task that wires each of them in;
the planned stack is in `spec.md` and `research.md` Part 4. The implementation is
**Kotlin/Native** (Gradle KMP, native targets).

## Programs

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| java (JDK 21, Temurin) | mise (`mise.toml`, `temurin-21`) | Runs Gradle and the Kotlin/Native compiler | https://adoptium.net/ |
| Gradle (9.9.2) | Gradle wrapper (`gradlew`, `gradle/wrapper/`); `aulo-chat/gradlew` for the chat client's own build | Builds the KMP project, runs tests and ktlint; builds `aulo-chat` | https://github.com/gradle/gradle |
| Kotlin/Native (2.4.21) | Kotlin Gradle plugin (`gradle/libs.versions.toml`), downloads its own toolchain | Compiles the `aulo` and `aulod` executables for macOS arm64, Linux x86_64/aarch64 and Windows x86_64 | https://github.com/JetBrains/kotlin |
| Android SDK (platform 37.1, build tools) | Android Studio or `sdkmanager`; found through `ANDROID_HOME` | Builds the `aulo-chat` Android app | https://developer.android.com/studio |
| Xcode | Mac App Store | Builds the `aulo-chat` iOS app and links its Kotlin framework | https://developer.apple.com/xcode/ |
| XcodeGen (2.48.0) | mise (`aulo-chat/mise.toml`, `aqua:yonaskolb/XcodeGen`) | Generates `aulo-chat/iosApp/iosApp.xcodeproj` from `project.yml`, so no Xcode project is kept in git | https://github.com/yonaskolb/XcodeGen |
| python3 | system / mise (`mise.toml`) | Runs `scripts/sync-issues.py` (stdlib only) | https://github.com/python/cpython |
| gh | brew / mise | Creates labels, milestones and issues from `plan/` | https://github.com/cli/cli |
| just | mise (`mise.toml`) | Task runner (`justfile`) | https://github.com/casey/just |
| jactionlint | mise (`mise.toml` pins `github:jdx/jactionlint` 2.0.2) | Lint the workflows (the pre-push hook and `mise x -- jactionlint`) | https://github.com/jdx/jactionlint |
| hk | mise (`mise.toml` pins 2.5.0) | Runs jactionlint as a pre-push hook (`hk.pkl`) | https://github.com/jdx/hk |
| shellcheck | mise (`mise.toml` pins 0.11.0) | Checks `run:` scripts inside jactionlint | https://github.com/koalaman/shellcheck |
| buf (1.73.0) | brew | Lints and builds the protos, `buf format`, breaking-change check (`proto/buf.yaml`) | https://github.com/bufbuild/buf |
| mise | brew | Pins the JDK, just, jactionlint, hk, shellcheck, Python and the agent CLIs | https://github.com/jdx/mise |
| conan (2.33.0) | mise (`mise.toml`) | Installs the static C libraries: SQLite that kotlin-kit's `db-native` links into the Windows binaries (`kotlin-kit/db-native/conan.lock`), and the packages in the Conan table below (`aulo.conan` plugin, profiles in `conan/`) | https://github.com/conan-io/conan |
| cmake (3.31.12) | mise (`mise.toml`) | Conan's builds of those libraries from source (every target but macOS) | https://github.com/Kitware/CMake |
| ninja (1.13.2) | mise (`mise.toml`) | CMake's generator for those builds on a Windows host | https://github.com/ninja-build/ninja |
| uv (0.13.9) | mise (`mise.toml`) | Installs conan for mise (a PyPI package) | https://github.com/astral-sh/uv |
| grpcurl (1.9.4) | mise, on demand (`mise x aqua:fullstorydev/grpcurl@1.9.4`) | Optional: `AULO_GRPCURL` makes the aulo-server tests list services and call health with it | https://github.com/fullstorydev/grpcurl |
| protoc-gen-doc (v1.5.1) | buf remote plugin `buf.build/community/pseudomuto-doc` (`buf.gen.yaml`), nothing to install | Generates the Markdown gRPC API reference in `docs/api/` (`just api-docs`) | https://github.com/pseudomuto/protoc-gen-doc |

## Gradle

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| org.jetbrains.kotlin:kotlin-gradle-plugin | local (`build-logic`) | https://github.com/JetBrains/kotlin | Kotlin Multiplatform with the native targets |
| org.jlleitschuh.gradle:ktlint-gradle | local (`build-logic`) | https://github.com/JLLeitschuh/ktlint-gradle | `ktlintCheck` / `ktlintFormat` on every module |
| com.pinterest.ktlint (1.8.0) | local (run by ktlint-gradle) | https://github.com/pinterest/ktlint | Kotlin style rules |
| org.jetbrains.kotlin:kotlin-serialization | local (`build-logic`) | https://github.com/JetBrains/kotlin | Serialization compiler plugin for `@Serializable` types |
| org.jetbrains.kotlinx:kotlinx-serialization-core (1.13.0) | local (`aulo-types`) | https://github.com/Kotlin/kotlinx.serialization | `@Serializable` ids and events (spec §17 D4: the equivalent of serde) |
| org.jetbrains.kotlinx:kotlinx-serialization-json (1.13.0) | local (`aulo-config`; `aulo-types` tests) | https://github.com/Kotlin/kotlinx.serialization | Config layers as JSON trees, the generated schema; JSON round-trip tests of the contract types |
| pyrlyn.kit:toml | local (included build `packages/kotlin-kit`, pyrlyn/packages 0.0.5 in CI) | https://github.com/pyrlyn/packages/tree/main/kotlin-kit/toml | Strict TOML 1.1 parser for config files |
| pyrlyn.kit:fs | local (included build `packages/kotlin-kit`, pyrlyn/packages 0.0.5 in CI) | https://github.com/pyrlyn/packages/tree/main/kotlin-kit/fs | Reading config files (missing is empty), atomic writes, rotating log files, temporary directories in tests |
| pyrlyn.kit:cli | local (included build `packages/kotlin-kit`, pyrlyn/packages 0.0.5 in CI) | https://github.com/pyrlyn/packages/tree/main/kotlin-kit/cli | `aulo` command tree, help and errors with clap semantics |
| pyrlyn.kit:process | local (included build `packages/kotlin-kit`, pyrlyn/packages 0.0.5 in CI) | https://github.com/pyrlyn/packages/tree/main/kotlin-kit/process | The process environment for the config env layer |
| app.cash.sqldelight:gradle-plugin (2.4.1) | local (`build-logic`) | https://github.com/sqldelight/sqldelight | Typed SQLite queries in `aulo-store`, checked against the schema at build time |
| app.cash.sqldelight:runtime (2.4.1) | local (via `pyrlyn.kit:db-sqldelight`) | https://github.com/sqldelight/sqldelight | Runtime of the generated queries |
| pyrlyn.kit:db-sqldelight | local (included build `packages/kotlin-kit`, pyrlyn/packages 0.0.5 in CI) | https://github.com/pyrlyn/packages/tree/main/kotlin-kit/db-sqldelight | Runs SQLDelight's generated queries on a kit `db` connection (`aulo-store`) |
| pyrlyn.kit:db-room | local (included build `packages/kotlin-kit`, pyrlyn/packages 0.0.5 in CI) | https://github.com/pyrlyn/packages/tree/main/kotlin-kit/db-room | SQLite on macOS and Linux (`aulo-store`) |
| pyrlyn.kit:db-native | local (included build `packages/kotlin-kit`, pyrlyn/packages 0.0.5 in CI) | https://github.com/pyrlyn/packages/tree/main/kotlin-kit/db-native | SQLite on Windows, linked statically (`aulo-store`) |
| androidx.sqlite:sqlite-bundled (2.7.1) | local (via `pyrlyn.kit:db-room`; Google's Maven repository, `androidx.*` only) | https://developer.android.com/jetpack/androidx/releases/sqlite | The bundled SQLite (with FTS5) on macOS and Linux |
| sqlite3 (3.54.0) | local (via `pyrlyn.kit:db-native`, Conan) | https://sqlite.org/ | The SQLite (with FTS5) on Windows |
| com.squareup.wire:wire-gradle-plugin (7.2.2) | local (`build-logic`) | https://github.com/square/wire | Generates `aulo-proto` from `proto/aulo/v1` with its own proto parser, so no `protoc` (spec §17 D10; Apache-2.0) |
| com.squareup.wire:wire-schema (7.2.2) | local (`build-logic`) | https://github.com/square/wire | Wire's schema API and descriptor encoder for `GrpcTables`, the handler that writes the gRPC method tables and reflection descriptors (Apache-2.0) |
| com.squareup.wire:wire-runtime (7.2.2) | local (`aulo-proto`) | https://github.com/square/wire | Runtime of the generated messages: `Message`, `ProtoAdapter`, `Service` (Apache-2.0) |
| com.squareup.okio:okio (3.18.2) | local (via `wire-runtime`) | https://github.com/square/okio | Byte strings and buffers of the Wire runtime (Apache-2.0) |
| org.jetbrains.kotlinx:kotlinx-coroutines-core (1.11.0) | local (`aulo-proto`, `aulo-grpc`) | https://github.com/Kotlin/kotlinx.coroutines | `suspend` service methods and the channels of streaming RPCs; the version kotlin-kit uses (Apache-2.0) |
| io.ktor:ktor-network (3.6.1) | local (`aulo-grpc`) | https://github.com/ktorio/ktor | TCP and Unix sockets under the gRPC transport on every target; HTTP/2 itself is nghttp2 (spec §17 D11; Apache-2.0) |
| io.ktor:ktor-io, io.ktor:ktor-utils (3.6.1) | local (via `ktor-network`) | https://github.com/ktorio/ktor | The socket byte channels (Apache-2.0) |
| org.jetbrains.kotlinx:kotlinx-io-core | local (via `ktor-io`) | https://github.com/Kotlin/kotlinx-io | Removes a Unix socket file when the gRPC server shuts down (Apache-2.0) |
| org.jetbrains.kotlin:kotlin-test | local (`commonTest`) | https://github.com/JetBrains/kotlin | Unit tests |
| org.jetbrains.kotlin:kotlin-test-junit5 | local (`build-logic` tests) | https://github.com/JetBrains/kotlin | Tests of the module-graph rules on the JVM |
| org.junit.platform:junit-platform-launcher | local (`build-logic` tests) | https://github.com/junit-team/junit-framework | Gradle 9 needs it on the test runtime classpath |

## Conan

Static libraries a module binds through cinterop: the version in its `conanfile.txt`, the recipe
revision in its `conan.lock`. Conan Center's binary on macOS; built from the recipe with
Kotlin/Native's own clang and sysroots for Linux and Windows (`research.md` 4.10).

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| libnghttp2 (1.68.1) | local (`aulo-grpc/conanfile.txt`) | https://github.com/nghttp2/nghttp2 | HTTP/2 under aulo's gRPC transport on every target (spec §17 D11; MIT) |

Native libraries used by the speech and audio modules (sherpa-onnx, whisper.cpp, libopus,
and so on) are chosen and recorded here by the task that first wires each of them, including
any cinterop or CMake dependency.

## Gradle (`aulo-chat/gradle/libs.versions.toml`)

Everything here is local to the `aulo-chat` build (T12.22).

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| Kotlin Gradle plugins 2.4.21 (multiplatform, jvm, compose compiler) | local | https://github.com/JetBrains/kotlin | Compile the client for Android, iOS and the JVM, and the server |
| Compose Multiplatform 1.12.2 (plugin; runtime, foundation, ui, ui-test) | local | https://github.com/JetBrains/compose-multiplatform | The UI toolkit of the client on every target, and its UI tests |
| Compose Material 3 1.9.0 (`org.jetbrains.compose.material3`) | local | https://github.com/JetBrains/compose-multiplatform | The widgets of the chat screen |
| Android Gradle Plugin 9.5.2 (application, KMP library) | local | https://developer.android.com/build | Builds the Android app and the Android side of the shared modules |
| kotlinx-rpc 0.11.0-grpc-190 (plugin; grpc-core, grpc-client, grpc-server, protobuf) | local, from `redirector.kotlinlang.org/maven/kxrpc-grpc` | https://github.com/Kotlin/kotlinx-rpc | Generates Kotlin from `proto/aulo/v1` and runs gRPC on the JVM, Android and Kotlin/Native; a pre-release, the first with gRPC on Native |
| grpc-java 1.81.0 (`grpc-netty-shaded`, `grpc-okhttp`) | local | https://github.com/grpc/grpc-java | The gRPC transports under kotlinx-rpc: Netty on desktop and in the server, OkHttp on Android |
| kotlinx-coroutines 1.11.0 (core, swing, test) | local | https://github.com/Kotlin/kotlinx.coroutines | Flows and `StateFlow`, the desktop main dispatcher, tests |
| Koin 4.2.2 (core, core-viewmodel, compose, compose-viewmodel) | local | https://github.com/InsertKoinIO/koin | Dependency injection |
| SQLDelight 2.4.1 (plugin; runtime, coroutines-extensions, sqlite, android and native drivers) | local | https://github.com/sqldelight/sqldelight | The offline cache of recent chats |
| JetBrains Lifecycle 2.11.0 (viewmodel, viewmodel-compose, runtime-compose) | local | https://www.jetbrains.com/help/kotlin-multiplatform-dev/compose-lifecycle.html | `ViewModel` and lifecycle-aware state collection on every target |
| androidx.activity:activity-compose 1.13.0 | local | https://developer.android.com/jetpack/androidx/releases/activity | Hosts Compose in the Android activity |
| kotlin-test | local | https://github.com/JetBrains/kotlin | Test assertions |
