---
status: in progress
priority: P0
complexity: 3
readiness: 100%
agent: Claude Code / claude-fable-5-2
---

# T3.2. aulo-proto: generated gRPC code

Stage: S3 · Area: api · Depends on: T3.1, T2.1 · Blocks: 5 task(s)

gRPC/protobuf code generation from `proto/aulo/v1`; CI runs `buf breaking` against main.

Done when:

- generated module builds; CI fails on a breaking field change

Execution plan:

The creator chose Wire (spec §17 D10): no library has a gRPC transport on all four targets, and kotlinx-rpc gRPC has no Windows target (`research.md` 4.9). The transport is T3.3's.

1. A new contract module, `:aulo-proto`, applies the Wire Gradle plugin through `build-logic`.
   - Source: `proto/` at the repository root. Output: `build/generated`, never committed.
   - Messages, plus one `suspend` interface per service (`rpcRole = "server"`). The client role needs `wire-grpc-client`, which has no linuxArm64 build.
2. Tests in `commonTest`:
   - the bytes of a message follow the proto's field numbers;
   - a round trip, a `oneof`, and a field this build does not know;
   - a streaming service interface implemented and called through channels.
3. Docs: the decision in `spec.md` §17, the sources in `research.md` 4.9, rows in `toolchain.md` (and the unused `protoc` row removed), the module in `AGENTS.md`.
4. Verify:
   - macOS: `./gradlew test ktlintCheck checkModuleGraph :build-logic:test`;
   - the linuxArm64 test binary on Colima; the linuxX64 and mingwX64 test binaries cross-link;
   - `buf breaking` fails on a renumbered field. `proto.yml` already runs it on every pull request that touches `proto/**`.
