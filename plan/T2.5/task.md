# T2.5. aulo-config: typed config with schema and layers

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 5 task(s)

Config types with schemars; committed schemas/aulo.schema.json with a stale test; figment layers default < ~/.aulo/config.toml < project .aulo.toml < AULO_* env < flags, with per-key provenance. The only crate that touches toml/toml_edit/figment. Use config-schema when T1.14 lands.


Done when:

- schema test fails when types change without regenerating
- unknown keys are rejected with the file name in the error
