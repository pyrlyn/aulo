# T2.1. Gradle KMP project skeleton

Stage: S2 · Area: infra · Depends on: T0.1 · Blocks: 6 task(s)

Root Gradle KMP project for Kotlin/Native: native targets (macOS arm64, Linux x86_64 and aarch64, Windows x86_64), version catalog, licence from D1, ktlint, a surface module that links `aulo` and `aulod` executables printing their version, `mise.toml` pinning JDK 21, `justfile`, `.gitignore`.

Execution plan: 1. Root `settings.gradle.kts` / `build.gradle.kts` and a version catalog; native targets as in D3; licence expression from D1; ktlint on every module. 2. `:aulo` surface module with `aulo` and `aulod` executables that print their version, so the project has a default entry point. 3. `mise.toml` pins JDK 21 (Temurin); `justfile` wraps `./gradlew test` and `ktlintCheck` once the wrapper exists; `.gitignore` covers Gradle and Kotlin output. 4. Verify with `mise exec -- ./gradlew test` and `./gradlew ktlintCheck` once the wrapper is in the tree.

Done when:

- `./gradlew test` and `./gradlew ktlintCheck` pass on the skeleton
