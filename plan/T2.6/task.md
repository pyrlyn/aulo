# T2.6. Embedded default config

Stage: S2 · Area: infra · Depends on: T2.5 · Blocks: 0 task(s)

config/default.toml with every assignment commented out; a test checks it parses to Config::default(). `aulo config show --origin` prints values with provenance.


Done when:

- test passes; trycmd fixture for config show
