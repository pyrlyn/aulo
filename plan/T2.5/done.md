# T2.5. Done

The `:aulo-config` module. PR: https://github.com/pyrlyn/aulo/pull/87 (stacked on #86).

- `packages/pyrlyn-kit` is a Gradle included build, as the creator chose.
  - Gradle finds it in the first ancestor `packages/` or through `KOTLIN_KIT_DIR` and `SHARED_SETTINGS_DIR`.
  - CI checks out pyrlyn/packages at 52e941b (tag 0.0.5).
  - `pyrlyn.kit:toml` parses TOML and `pyrlyn.kit:fs` reads files.
- The model ports the earlier Rust crate (git history before #81) with the same keys and defaults:
  - `daemon`, `providers` (endpoints, tiers) and `voice` (listen, barge_in, stt, tts);
  - `mcp.servers`, which is `stdio` or `http` by `transport`;
  - `plugins` and `policy`.
- Layers: defaults < `$AULO_HOME/config.toml` (else `~/.aulo`) < `<project>/.aulo.toml` < `AULO_<SECTION>__<KEY>` < overrides.
  - The environment is passed in as a map.
  - Each variable is its own layer.
  - Every leaf records its `Origin` in `Provenance`.
- Each layer is checked against the serial descriptors before the merge:
  - unknown keys;
  - wrong types and enum names;
  - `@Minimum`.
  The merged result must also have every required key. Errors name the file or variable and the dotted key, never the value.
- `ConfigSchema` generates `schemas/aulo.schema.json` from the same descriptors. A test compares the file with the generated text; `just schema` rewrites it.
- Left to their own tasks: rejecting inline secrets (T4.2), engine choice (S6), and `default.toml` with `config show` (T2.6).

How it was checked:

- `./gradlew test ktlintCheck` passes locally on macOS arm64, with 13 aulo-config tests. Among them:
  - an unknown key is rejected with the file name and the dotted key;
  - layer order and provenance;
  - the env mapping;
  - map entries merge across layers;
  - a value is never echoed in an error.
- Adding a field without regenerating made the schema test fail with "schemas/aulo.schema.json is stale".
- The linuxArm64 test binaries of `:aulo`, `:aulo-types` and `:aulo-config` ran in an Ubuntu 24.04 container on Colima: 1, 10 and 13 tests passed.
- CI on the branch (https://github.com/pyrlyn/aulo/actions/runs/45074817290) built and linked on all four targets. The org variable `SKIP_CI_TESTS=true` skipped the unit tests there.
- The Windows test binary cross-links, but has not been run.
