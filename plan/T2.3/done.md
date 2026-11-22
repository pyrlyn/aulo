# T2.3. Done

`.github/workflows/ci.yml` builds and tests aulo on every supported target. PR: https://github.com/pyrlyn/aulo/pull/85.

- The `build` matrix job is named `<os> (<target>)` and runs on three targets:
  - macOS arm64 on `macos-15`, picked through pyrlyn/ci's `macos-runner` action;
  - Linux x86_64 on `ubuntu-24.04`;
  - Windows x86_64 on `windows-2025`.
- Each `build` job runs `./gradlew test ktlintCheck`, links the release `aulo` and `aulod`, runs `--version` on both as a smoke test and uploads them. Nothing builds for x86_64 macOS.
- linuxArm64 is cross-linked on the Linux x86_64 job. The `ubuntu-24.04-arm (linuxArm64)` job then runs that job's test binary and both executables on real arm64 hardware.
- The docs-only skip and the `SKIP_CI_TESTS` and `MACOS_CI_DISABLED` variables behave as in the other Kotlin/Native project. The commit-subject job and pipeline.yml stay.
- pyrlyn/ci has no shared Kotlin/Gradle workflow (checked at 9675ff42). The jobs therefore live in this repository and use pyrlyn/ci's `config` and `macos-runner` actions.

How it was checked:

- `jactionlint` passes locally.
- The branch was run with `workflow_dispatch`, because CI only triggers on pull requests to main.
- First run (45071654815) failed on two causes:
  - GitHub no longer serves the `macos-14` image ("configuration 'macos-14-arm64-us-default' is not supported"), so the label moved to `macos-15`;
  - Maven Central returned 429 to the Linux job.
- Second run (https://github.com/pyrlyn/aulo/actions/runs/45072136180) was green on all four targets.
- After the T2.4 run hit 429 again in a second Gradle pass, test, lint and both links became one Gradle invocation per job.
