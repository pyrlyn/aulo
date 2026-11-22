# Command fixtures

Each `<case>.toml` runs one `aulo` command line in-process, as trycmd does for Rust CLIs:

- `args`: the arguments after the program name.
- `status`: the expected exit code (default 0).
- `[env]`: the whole environment the command sees. `AULO_HOME` is added and points at a temporary directory.
- `[home]` and `[project]`: files to create in `$AULO_HOME` and in the project directory, by name.

`<case>.stdout` and `<case>.stderr` hold the expected output; a missing file means empty.
`[HOME]` and `[PROJECT]` stand for the two temporary directories, with `/` as the separator.
`AULO_UPDATE_FIXTURES=1 ./gradlew :aulo:allTests` rewrites the expected files from the actual output; review the diff.
`CmdTest` runs every case: `./gradlew :aulo:allTests`.
