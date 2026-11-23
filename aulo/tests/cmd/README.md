# Command fixtures

Each `<case>.toml` runs one `aulo` or `aulod` command line in-process, as trycmd does for Rust CLIs:

- `program`: `aulo` (the default) or `aulod`. `aulod run` keeps its secrets in files and stops as soon as it listens; its log lines come first in the expected stderr.
- `args`: the arguments after the program name.
- `status`: the expected exit code (default 0).
- `[env]`: the whole environment the command sees. `AULO_HOME` is added and points at a temporary directory.
- `[home]` and `[project]`: files to create in `$AULO_HOME` and in the project directory, by name.
- `running`: `aulod` only; another aulod with this pid holds the home's lock and pidfile while the command runs.

`<case>.stdout` and `<case>.stderr` hold the expected output; a missing file means empty.
`[HOME]` and `[PROJECT]` stand for the two temporary directories, with `/` as the separator.
`AULO_UPDATE_FIXTURES=1 ./gradlew :aulo:allTests` rewrites the expected files from the actual output; review the diff.
`CmdTest` runs every case: `./gradlew :aulo:allTests`.
