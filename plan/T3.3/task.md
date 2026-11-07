# T3.3. aulo-server: tonic server scaffold

Stage: S3 · Area: api · Depends on: T3.2, T2.5, T2.7 · Blocks: 4 task(s)

tonic server with tonic-health and reflection, listeners for a Unix socket / Windows named pipe (local) and TCP (remote), graceful shutdown, request size limits.


Done when:

- integration test connects over UDS and calls health
