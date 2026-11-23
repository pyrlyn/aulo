# T3.2. Done

`aulo.v1` as Kotlin/Native code in `:aulo-proto`. PR: https://github.com/pyrlyn/aulo/pull/97, stacked on #96.

- **Generator.** The Wire Gradle plugin (7.2.2, Apache-2.0) generates the module from `proto/aulo/v1` on every build. Nothing generated is committed.
  - The creator chose Wire (spec §17 D10). The comparison and its sources are in `research.md` 4.9.
  - Wire parses the protos itself, so `protoc` left `toolchain.md`.
- **Output.**
  - A message class for every type, with its `ProtoAdapter`.
  - One `suspend` interface per service, such as `ChatServiceServer`. Streaming RPCs take coroutine channels.
- **Not here.**
  - No transport: Wire has none for Kotlin/Native. T3.3 binds the interfaces to one.
  - No generated client role: it needs `wire-grpc-client`, which has no linuxArm64 build.
  - No method table (path and adapters per RPC). Wire puts it in annotations, which Kotlin/Native cannot read at run time, so T3.3 has to generate or write one.
- **Module role.** `:aulo-proto` is a contract. `checkModuleGraph` accepts `wire-runtime` there and still gives `wire-grpc*` one owner.
- **Breaking changes.** `proto.yml` already runs `buf breaking` against the base of every pull request that touches `proto/**`. Nothing was added.

How it was checked:

- `./gradlew test ktlintCheck checkModuleGraph :build-logic:test` passes on macOS arm64, with warnings as errors on the generated code. aulo-proto has 5 tests:
  - the bytes of a message follow the proto's field numbers;
  - a round trip, with an unset optional field staying unset;
  - a `oneof` holds the one case that was sent;
  - a field this build does not know is kept and re-encoded;
  - a streaming service implemented and called through channels.
- The linuxArm64 test binary passed in Ubuntu 24.04 on Colima: 5 tests.
- The linuxX64 and mingwX64 test binaries cross-link, but have not been run.
- `buf breaking` exits 100 on a renumbered field (`CreateChatRequest.title` 2 → 7) and 0 on the branch.
- CI on the branch built, linked and smoke-tested all four targets: https://github.com/pyrlyn/aulo/actions/runs/45135488896. The org variable `SKIP_CI_TESTS=true` skipped tests there.
  - The first attempt failed on Linux before any aulo code ran: Maven Central answered 429 while Gradle resolved the Kotlin plugin. The rerun of the failed jobs passed.
