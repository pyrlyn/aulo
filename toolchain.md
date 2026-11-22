# Toolchain

Only what the project uses today. Libraries are added by the task that wires each of them in;
the planned stack is in `spec.md` and `research.md` Part 4. The implementation is
**Kotlin/Native** (Gradle KMP, native targets).

## Programs

| Program | How to install | Why here | Source |
| --- | --- | --- | --- |
| java (JDK 21, Temurin) | mise (`mise.toml`, `temurin-21`) | Runs Gradle and the Kotlin/Native compiler | https://adoptium.net/ |
| Gradle (9.9.2) | Gradle wrapper (`gradlew`, `gradle/wrapper/`) | Builds the KMP project, runs tests and ktlint | https://github.com/gradle/gradle |
| Kotlin/Native (2.4.21) | Kotlin Gradle plugin (`gradle/libs.versions.toml`), downloads its own toolchain | Compiles the `aulo` and `aulod` executables for macOS arm64, Linux x86_64/aarch64 and Windows x86_64 | https://github.com/JetBrains/kotlin |
| python3 | system / mise (`mise.toml`) | Runs `scripts/sync-issues.py` (stdlib only) | https://github.com/python/cpython |
| gh | brew / mise | Creates labels, milestones and issues from `plan/` | https://github.com/cli/cli |
| just | mise (`mise.toml`) | Task runner (`justfile`) | https://github.com/casey/just |
| jactionlint | mise (`mise.toml` pins `github:jdx/jactionlint` 2.0.2) | Lint the workflows (the pre-push hook and `mise x -- jactionlint`) | https://github.com/jdx/jactionlint |
| hk | mise (`mise.toml` pins 2.5.0) | Runs jactionlint as a pre-push hook (`hk.pkl`) | https://github.com/jdx/hk |
| shellcheck | mise (`mise.toml` pins 0.11.0) | Checks `run:` scripts inside jactionlint | https://github.com/koalaman/shellcheck |
| buf (1.73.0) | brew | Lints and builds the protos, `buf format`, breaking-change check (`proto/buf.yaml`) | https://github.com/bufbuild/buf |
| protoc | brew (`PROTOC` or `PATH`) | Compiles `proto/aulo/v1` where a build step needs the compiler on `PATH` | https://github.com/protocolbuffers/protobuf |
| mise | brew | Pins the JDK, just, jactionlint, hk, shellcheck, Python and the agent CLIs | https://github.com/jdx/mise |
| protoc-gen-doc (v1.5.1) | buf remote plugin `buf.build/community/pseudomuto-doc` (`buf.gen.yaml`), nothing to install | Generates the Markdown gRPC API reference in `docs/api/` (`just api-docs`) | https://github.com/pseudomuto/protoc-gen-doc |

## Gradle

| Package | Where | Source | Why here |
| --- | --- | --- | --- |
| org.jetbrains.kotlin:kotlin-gradle-plugin | local (`build-logic`) | https://github.com/JetBrains/kotlin | Kotlin Multiplatform with the native targets |
| org.jlleitschuh.gradle:ktlint-gradle | local (`build-logic`) | https://github.com/JLLeitschuh/ktlint-gradle | `ktlintCheck` / `ktlintFormat` on every module |
| com.pinterest.ktlint (1.8.0) | local (run by ktlint-gradle) | https://github.com/pinterest/ktlint | Kotlin style rules |
| org.jetbrains.kotlin:kotlin-serialization | local (`build-logic`) | https://github.com/JetBrains/kotlin | Serialization compiler plugin for `@Serializable` types |
| org.jetbrains.kotlinx:kotlinx-serialization-core (1.13.0) | local (`aulo-types`) | https://github.com/Kotlin/kotlinx.serialization | `@Serializable` ids and events (spec §17 D4: the equivalent of serde) |
| org.jetbrains.kotlinx:kotlinx-serialization-json (1.13.0) | local (`aulo-types` tests) | https://github.com/Kotlin/kotlinx.serialization | JSON round-trip tests of the contract types |
| org.jetbrains.kotlin:kotlin-test | local (`commonTest`) | https://github.com/JetBrains/kotlin | Unit tests |
| org.jetbrains.kotlin:kotlin-test-junit5 | local (`build-logic` tests) | https://github.com/JetBrains/kotlin | Tests of the module-graph rules on the JVM |
| org.junit.platform:junit-platform-launcher | local (`build-logic` tests) | https://github.com/junit-team/junit-framework | Gradle 9 needs it on the test runtime classpath |

Native libraries used by the speech and audio modules (sherpa-onnx, whisper.cpp, libopus,
and so on) are chosen and recorded here by the task that first wires each of them, including
any cinterop or CMake dependency.
