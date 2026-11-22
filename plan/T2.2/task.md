# T2.2. Dependency-graph test

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 0 task(s)

A test reads the Gradle project metadata and enforces module roles: contracts have no I/O deps, only surfaces depend on the CLI and error libraries, one owner module per heavy dependency (SQLite, the gRPC stack, sherpa-onnx, a WASM runtime).

Done when:

- the test fails when a contract module pulls I/O or HTTP client libraries
