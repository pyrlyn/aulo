# T3.4. aulo-server: authentication

Stage: S3 · Area: api · Depends on: T3.3 · Blocks: 3 task(s)

Local socket: trust by file owner and 0600 permissions. TCP: bearer token required, compared in constant time, interceptor rejects missing tokens. The first token is generated on first run and stored in the OS keychain.


Done when:

- tests: TCP without token is rejected; UDS from owner is accepted
