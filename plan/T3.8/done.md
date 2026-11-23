# T3.8 done

PR: https://github.com/pyrlyn/aulo/pull/106 (stacked on #105), branch `t3.8-tls`.

## What was done

- mbedTLS 3.6.7 through Conan (`conanfile.txt`, lock, `Conan.kt` takes several static
  libraries), linked into `aulo-grpc` behind the C shim
  `aulo-grpc/src/nativeInterop/cinterop/mbedtls.def`: TLS 1.3 only, ALPN `h2` required, a
  memory BIO, one process-wide lock because mbedTLS is built without threading. Licence and
  advisory analysis (3.6.8 CVEs do not reach a 1.3-only config) in `research.md` §4.12.
- `aulo-grpc`: HTTP/2 over `ByteStream` (`SocketStream`, `TlsStream`); `TlsServer`,
  `TlsClient(pin)`, `Tls.selfSigned`, `Tls.formatPin`/`parsePin`. The pin is the SHA-256 of
  the certificate DER, checked right after the handshake; a mismatch sends a fatal alert.
  `GrpcServer.bind(endpoint, tls)` handshakes each connection under
  `GrpcLimits.handshakeTimeout`, and handshaking connections count toward `maxConnections`;
  the client has the same deadline.
- `aulo-server`: `serverTls` loads `daemon.tls_cert` + `daemon.tls_key` (both or neither,
  relative to the home, 64 KiB cap, links followed) or makes a self-signed certificate once in
  `<home>/tls` through `FileSecretStore`. `runDaemon` serves it on `daemon.listen` and logs its
  SHA-256. The "TCP refused until TLS" path is gone.
- `aulo connect ADDR --pin SHA256`: handshake with the pin, then `grpc.health` with the token
  from `AULO_TOKEN` when set; the token is never echoed and nothing is saved.

## How it was checked

- `TlsTest` (aulo-grpc): right pin streams 4 MiB both ways; wrong pin refused before any call;
  plain client on a TLS port and TLS on a plain port fail; silent peer evicted at the handshake
  deadline; client handshake deadline; certificate/key pairing; pin format and parsing.
- `ServerTlsTest`, `ListenersTest`, `DaemonTest` (aulo-server): self-signed made once,
  owner-only and reused; configured files checked and capped; tokens still required over TLS;
  aulod serves the pin it logs.
- `ConnectTest` and command fixtures (`connect_*`, `aulod_listen_half_tls`) in aulo.
- macOS: aulo-grpc 33, aulo-server 34, aulo-config 23, aulo 10 tests pass; aulo-grpc,
  aulo-server and aulo pass on linuxX64 and linuxArm64 on Colima; mingwX64 test binaries and
  `aulod` link; `ktlintCheck` and `checkModuleGraph` pass.
- CI run 45175053181 dispatched on `t3.8-tls`.
