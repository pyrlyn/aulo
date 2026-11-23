---
status: in progress
priority: P0
complexity: 5
readiness: 5%
agent: Claude Code / claude-fable-5-2
---

# T3.3. aulo-server: gRPC server scaffold

Stage: S3 · Area: api · Depends on: T3.2, T2.5, T2.7 · Blocks: 4 task(s)

gRPC server with health and reflection, listeners for a Unix socket / Windows named pipe (local) and TCP (remote), graceful shutdown, request size limits.

Done when:

- integration test connects over UDS and calls health

Execution plan:

The creator chose nghttp2 through Conan with aulo's own gRPC layer on top (2026-11-23). Ktor's native server engine has no HTTP/2, and Wire has no native transport (`research.md` 4.9).

The work lands in three pull requests, each stacked on the one before.

1. **nghttp2 on every target.** A new library module, `:aulo-grpc`, the one owner of the gRPC stack.
   - Conan installs a static nghttp2: the Conan Center binary on macOS; built from the locked recipe with Kotlin/Native's own clang and sysroots for Linux and Windows.
   - Profiles in `conan/`, a Conan task in `build-logic`, a cinterop binding, and a test that calls the library.
   - Verify: the test passes on macOS and on Colima (linuxArm64); the linuxX64 and mingwX64 test binaries link.
2. **The transport.** In `:aulo-grpc`, with no aulo service in it.
   - Byte streams and listeners: TCP and Unix sockets on macOS and Linux, TCP on Windows.
   - An HTTP/2 session over nghttp2, server and client side.
   - gRPC framing, status codes, deadlines, cancellation, graceful shutdown.
   - Caps on untrusted input, each with a test: message size, header size, concurrent streams, connections.
   - Verify: tests call a test service over a Unix socket and over TCP, unary and streaming.
3. **The server.** A new assembly module, `:aulo-server`.
   - A Wire schema handler in `build-logic` generates what Kotlin/Native cannot reflect: the method table of each service and the file descriptors.
   - `grpc.health.v1` and server reflection.
   - Listeners from config, graceful shutdown.
   - Verify: an integration test connects over a Unix socket and calls health; `grpcurl` lists the services and calls health.

Open: the Windows named pipe. It cannot be run on this machine or in CI while `SKIP_CI_TESTS` is on. Ask the creator before writing it.
