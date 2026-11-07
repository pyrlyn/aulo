# T3.8. aulo-server: TLS for TCP listeners

Stage: S3 · Area: api · Depends on: T3.4 · Blocks: 1 task(s)


rustls; user-provided certificate or a generated self-signed one with fingerprint pinning in clients (`aulo connect --pin`).

Done when:

- test: client with wrong pin is refused
