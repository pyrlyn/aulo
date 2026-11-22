# T2.2. Done

The module graph is checked on every `./gradlew test`:

- Each module declares a role in its build file, `aulo { role = ModuleRole.X }` (`CONTRACT`,
  `LIBRARY`, `SURFACE`); the `aulo.kmp` plugin writes a manifest of the module's role, declared
  production dependencies and cinterop libraries (`moduleManifest`).
- The root `checkModuleGraph` task (`aulo.graph` plugin) reads every manifest and applies
  `ModuleGraph.violations` from `build-logic/src/main/kotlin/aulo/buildlogic/ModuleGraph.kt`:
  - a module without a role fails;
  - a contract fails on any I/O, HTTP or storage library (Ktor, Okio, OkHttp, kotlinx-io and
    every heavy dependency), any cinterop library, or a project dependency on a non-contract;
  - only surfaces may use the CLI libraries (Clikt, Mordant), and no module depends on a
    surface;
  - SQLite, the gRPC stack, sherpa-onnx and a WASM runtime may each have one owner module.
  Native libraries count as `cinterop:<name>`. The pattern lists are the one place to extend
  when a task wires a new library of these kinds.
- `:aulo` is a `SURFACE`. ktlint now covers `build-logic` too; `.editorconfig` switches its
  rules off below `build/`, where kotlin-dsl generates accessors.

Written in-tree: modules-graph-assert (https://github.com/jraska/modules-graph-assert, 2.9.1,
checked 2026-11-22) supports KMP configurations but only checks project-to-project
dependencies, not external libraries or cinterop, which the done criterion and the one-owner
rule need.

Pull request: https://github.com/pyrlyn/aulo/pull/84 (branch `t2.2-dependency-graph`, stacked
on #83).

Checked on macOS arm64 with `mise exec -- ./gradlew test ktlintCheck`: BUILD SUCCESSFUL,
10 `ModuleGraphTest` cases and 1 `VersionTest` case, 0 failures. The done criterion was
checked against the real build too: with `:aulo` switched to `CONTRACT` and
`io.ktor:ktor-client-core` added, `./gradlew test` fails with
`:aulo: contract module depends on I/O library io.ktor:ktor-client-core`. ktlint was checked
across clean and incremental runs, and still reports a planted style error in `build-logic`.
