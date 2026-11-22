---
status: in progress
priority: P0
complexity: 3
readiness: 80%
agent: Claude Code / claude-opus-5-5
---

# T2.5. aulo-config: typed config with schema and layers

Stage: S2 · Area: infra · Depends on: T2.1 · Blocks: 5 task(s)

Config types; committed schemas/aulo.schema.json with a stale-schema test; layers default < ~/.aulo/config.toml < project .aulo.toml < AULO_* env < flags, with per-key provenance. The only module that reads and writes TOML config. Use config-schema when T1.14 lands.

Done when:

- schema test fails when types change without regenerating
- unknown keys are rejected with the file name in the error

Execution plan:

1. Include `packages/kotlin-kit` as a Gradle included build, as ketch does (creator's choice). The build finds it in any ancestor `packages/` or through `KOTLIN_KIT_DIR` and `SHARED_SETTINGS_DIR`; CI checks out pyrlyn/packages at tag `0.0.5`. The TOML parser is `pyrlyn.kit:toml`; file reads go through `pyrlyn.kit:fs`.
2. New `:aulo-config` module, role `LIBRARY`. The model ports the earlier Rust crate (git history before #81): sections `daemon`, `providers` (endpoints, tiers), `voice` (listen, barge_in, stt, tts), `mcp.servers` (stdio or http, discriminated by `transport`), `plugins` and `policy`, with the same keys and defaults.
3. Layers: defaults < `$AULO_HOME/config.toml` (else `~/.aulo`) < `<project>/.aulo.toml` < `AULO_<SECTION>__<KEY>` < caller overrides. Missing files are skipped. The environment is passed in as a map, so tests need no process state. Each layer is checked against the serial descriptors before the merge:
   - unknown keys, wrong types, bad enum values and minimums are reported with the layer's file or variable;
   - values are never echoed, since a layer may hold something secret.
   Every leaf records its origin.
4. A JSON Schema generator walks the same descriptors and writes `schemas/aulo.schema.json` (draft 2020-12, `$defs`, `additionalProperties: false`, defaults, descriptions from a `@Doc` annotation). A native test compares the generated text with the committed file; `AULO_UPDATE_SCHEMA=1` rewrites it (`just schema`).
5. Tests: the stale schema, unknown keys naming the file, layer order and provenance, the env mapping and type coercion, missing required map-entry keys, and that values are never echoed. Verify with `./gradlew test ktlintCheck` locally and CI dispatched on the branch.

