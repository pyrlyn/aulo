---
status: in progress
priority: P0
complexity: 4
readiness: 80%
agent: Claude Code / claude-opus-5-5
---

# T3.8. aulo-server: TLS for TCP listeners

Stage: S3 · Area: api · Depends on: T3.4 · Blocks: 1 task(s)

TLS: user-provided certificate or a generated self-signed one with fingerprint pinning in clients (`aulo connect --pin`).

Done when:

- test: client with wrong pin is refused

## Execution plan

The creator chose mbedTLS (3.6 LTS, Conan Center `mbedtls/3.6.7`, Apache-2.0 OR GPL-2.0+).

1. `aulo-grpc`: HTTP/2 runs over a small byte-stream interface instead of a ktor `Socket`; the
   plain one wraps the socket, the TLS one wraps it with mbedTLS through a C shim
   (`mbedtls.def`, memory BIO: Kotlin moves bytes, C never touches the socket). TLS 1.3 only,
   ALPN `h2` required on both sides. The Conan helper links several static libraries
   (`mbedtls`, `mbedx509`, `mbedcrypto`).
2. Server: `GrpcServer.bind(endpoint, tls)` handshakes each TCP connection in its own coroutine
   under a deadline before HTTP/2 starts; handshakes count against `maxConnections`.
3. Client: `GrpcClient.connect(endpoint, tls = TlsClient(pin))`. The pin is the SHA-256 of the
   server certificate's DER; a different certificate ends the handshake with a fatal alert and
   UNAVAILABLE before any request is sent. `:scheme` is `https` over TLS.
4. `aulo-server`: `daemon.tls_cert` and `daemon.tls_key` both set → those files; both unset →
   a self-signed ECDSA P-256 certificate made once in the owner-only `<home>/tls` (key 0600) and
   reused; one without the other is an error. aulod logs the fingerprint to pin.
5. `aulo connect <ADDR> --pin <SHA256>`: handshakes, checks the pin and calls `grpc.health`
   with the bearer token from `AULO_TOKEN` when set. It saves nothing; remote profiles come
   with the text client (T3.7). A client without a pin (CA verification) is not part of this task.
6. Tests: transport (right pin works, wrong pin refused, plain client on a TLS port and TLS on
   a plain port fail, handshake deadline), aulo-server (self-signed made once and reused, key
   mode, tokens still checked over TLS), `aulo connect` with a right and a wrong pin.
7. Verify: macOS, linuxX64 and linuxArm64 on Colima, mingwX64 link, ktlint, module graph; CI.
