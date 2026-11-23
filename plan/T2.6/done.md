# T2.6. Done

The embedded default config and `aulo config show`. PR: https://github.com/pyrlyn/aulo/pull/92, stacked on #91.

- `aulo-config/config/default.toml` is the earlier Rust file, adapted:
  - every key is commented out, so the file changes nothing when loaded;
  - the auth paragraph now says API keys are not configuration;
  - a `[log]` section was added for T2.7.
- The `embedDefaultToml` Gradle task embeds the file as `DEFAULT_TOML`. The task fails if the file contains `"""`.
- `Loaded.render(withOrigin)` prints every leaf as `key = value`, sorted, in TOML syntax.
  - It masks through `Redact.scrubJson` first.
  - With `--origin` it appends `  # <layer>`.
- The CLI uses `pyrlyn.kit:cli`:
  - `aulo config show [--origin]` and `aulo config default`;
  - `--version` prints the version, git SHA and licence;
  - a config error goes to stderr with exit code 1, a usage error with exit code 2.
- `aulo/tests/cmd/` holds command fixtures: `<name>.toml` with args, env and files, plus the expected `.stdout` and `.stderr`. `CmdTest` runs them in-process. `AULO_UPDATE_FIXTURES=1` rewrites the expected files.
- The CI smoke test also runs `aulo config show --origin`.
- T2.7 ran first, because masking `config show` needs `Redact`.

How it was checked:

- `./gradlew test ktlintCheck checkModuleGraph` passes on macOS arm64: 4 aulo tests, 22 config tests and 15 telemetry tests.
  - `DefaultTomlTest` checks the file is inert and that uncommenting the examples loads.
  - It also checks that every config key is documented, and that the check catches a missing one.
  - `RenderTest` checks a render loads back to the same config, and that secrets are masked.
- The linuxArm64 test binaries passed in Ubuntu 24.04 on Colima, with the same counts.
- The release `aulo` binary ran there: `config show --origin`, `config default`, `--version`, and a bad key (exit code 1).
- The mingwX64 binaries cross-link, but have not been run.
- CI on the branch built, linked and smoke-tested all four targets: https://github.com/pyrlyn/aulo/actions/runs/45077320715. The org variable `SKIP_CI_TESTS=true` skipped tests there.
