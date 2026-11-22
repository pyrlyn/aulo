---
status: in progress
priority: P0
complexity: 2
readiness: 90%
agent: Claude Code / claude-opus-5-5
---

# T2.6. Embedded default config

Stage: S2 · Area: infra · Depends on: T2.5 · Blocks: 0 task(s)

config/default.toml with every assignment commented out; a test checks it parses to Config::default(). `aulo config show --origin` prints values with provenance.

Done when:

- test passes; trycmd fixture for config show

Execution plan (Kotlin/Native):

1. `aulo-config/config/default.toml`:
   - the earlier Rust file, plus the `[log]` section from T2.7;
   - without the `aulo auth` text, which T4.2 adds;
   - embedded as `DEFAULT_TOML` by a Gradle task, because Kotlin/Native has no resources.
2. Tests, as in the Rust crate:
   - the file is inert and parses to `Config()`;
   - uncommenting the defaults changes nothing, and the examples are valid;
   - every key of the descriptors is documented, and the completeness check catches a missing key.
3. `Loaded.render(withOrigin)`: one `key = value  # origin` line per leaf. The lines are valid TOML, and every value is masked through `aulo-telemetry`'s `Redact`, which gains URL userinfo masking.
4. `aulo config show [--origin]` and `aulo config default` on `pyrlyn.kit:cli`. The command-line layer reads the environment through `pyrlyn.kit:process` and the cwd's `.aulo.toml`.
5. Fixtures in `aulo/tests/cmd`, run in-process by `CmdTest` (the trycmd format, ported). The CI smoke test runs `--version` and `config show --origin` on every target.
6. Verify: `./gradlew test ktlintCheck` locally on macOS, linuxArm64 test binaries on Colima, CI build, link and smoke test.
