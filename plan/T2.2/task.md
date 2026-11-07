# T2.2. Dependency-graph test

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 0 task(s)

crates/aulo/tests/deps.rs reads cargo metadata and enforces crate roles: contracts have no I/O deps, only surfaces depend on clap/anyhow, one owner crate per heavy dependency (diesel, tonic, sherpa-onnx, wasmtime).


Done when:

- the test fails when a contract crate pulls tokio::fs or reqwest
