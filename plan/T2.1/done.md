# T2.1. Done

The Gradle KMP skeleton for Kotlin/Native is in the tree:

- Gradle 9.9.2 wrapper (checksum-pinned), version catalog `gradle/libs.versions.toml`
  (Kotlin 2.4.21, ktlint-gradle 14.2.0, ktlint 1.8.0).
- `build-logic/` with the `aulo.kmp` convention plugin: macosArm64, linuxX64, linuxArm64 and
  mingwX64 (spec §17 D3), warnings as errors, ktlint on every module, and a `test` lifecycle
  task over `allTests`.
- `:aulo` surface module linking the `aulo` and `aulod` executables. Both print name,
  version, commit and the D1 licence expression, which comes from `aulo.license` in
  `gradle.properties` through a generated `BuildInfo.kt`.
- `justfile` (`test`, `lint`, `build`, `api-docs`); `mise.toml` already pinned JDK 21
  (Temurin); `.gitignore` already covered Gradle and Kotlin output.
- README, AGENTS.md and toolchain.md describe the build.

Typesafe project accessors are off: with the root project named `aulo`, the `:aulo` module
would generate a second `getAulo()` accessor.

Pull request: https://github.com/pyrlyn/aulo/pull/83 (branch `t2.1-gradle-skeleton`).

Checked on macOS arm64 with `mise exec -- ./gradlew test ktlintCheck` (BUILD SUCCESSFUL,
1 test, 0 failures) and `just`. All eight link tasks (`aulo` and `aulod` × four targets)
succeed from the macOS host; the macOS executables print
`aulo 0.1.0-dev (<sha>)` / `aulod 0.1.0-dev (<sha>)` and the licence line. The Linux and
Windows binaries were cross-linked, not run. The only Gradle deprecation warning comes from
KGP 2.4.x itself (`KotlinNativeBundleArtifactsTypes`) and goes away with KGP 2.5.
