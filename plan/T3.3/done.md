# T3.3 done

aulod's gRPC server: health, reflection and the local listener from config, on aulo's own gRPC layer over nghttp2.

## What was done

Three stacked pull requests:

1. [pyrlyn/aulo#99](https://github.com/pyrlyn/aulo/pull/99): nghttp2 on every target.
   - New `:aulo-grpc` module.
   - Conan installs a static nghttp2: the Conan Center binary on macOS, built with Kotlin/Native's clang and sysroots for Linux and Windows.
   - A cinterop binding.
2. [pyrlyn/aulo#100](https://github.com/pyrlyn/aulo/pull/100): the transport in `:aulo-grpc`.
   - Byte streams and listeners over ktor-network: TCP, and Unix sockets including AF_UNIX on Windows.
   - An HTTP/2 session over nghttp2.
   - gRPC framing, status codes, deadlines, cancellation and graceful shutdown.
   - Tested caps on message size, header size, concurrent streams and connections.
3. [pyrlyn/aulo#102](https://github.com/pyrlyn/aulo/pull/102): `:aulo-server`.
   - A Wire schema handler in `build-logic` generates each service's method table, plus the file descriptors that reflection serves.
   - `grpc.health.v1` (Check, List, Watch) and `grpc.reflection.v1`.
   - Listeners from `daemon` config:
     - the local socket `<aulo home>/aulod.sock`, with the path length capped;
     - `daemon.listen` is parsed strictly but refused at start until T3.8 serves TLS.
   - Graceful shutdown turns health NOT_SERVING first.

Reused instead of written:

- `pyrlyn.kit` `fs` (`TempDir`), `process` (`runBounded`) and `gradle/conan.gradle.kts`;
- ktor-network for sockets;
- Wire's `SchemaEncoder` for descriptors.

## How it was checked

- macOS arm64: every aulo-grpc and aulo-server test passes, and grpcurl 1.9.4 (`AULO_GRPCURL`) lists the services, calls health `Check` and describes `aulo.v1.ChatService`.
- Linux x64 and arm64: the same test binaries pass in Colima (ubuntu:24.04).
- Windows (mingwX64): the test binaries link; they were not run, because no Windows host is available here and CI skips tests.
- CI builds, lints and links every target. Tests are not run in CI while `SKIP_CI_TESTS` is on.
