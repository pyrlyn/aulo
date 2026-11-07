# T3.2. aulo-proto: code generation and breaking-change check

Stage: S3 · Area: api · Depends on: T3.1 · Blocks: 3 task(s)

tonic-build/prost code generation; CI runs buf breaking against main.


Done when:

- generated crate builds; CI fails on a breaking field change
