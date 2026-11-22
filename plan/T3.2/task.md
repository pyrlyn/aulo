# T3.2. aulo-proto: generated gRPC code

Stage: S3 · Area: api · Depends on: T3.1, T2.1 · Blocks: 5 task(s)

gRPC/protobuf code generation from `proto/aulo/v1`; CI runs `buf breaking` against main.

Done when:

- generated module builds; CI fails on a breaking field change
