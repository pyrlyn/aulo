# T16.8. Isolate credential-list fixtures from developer credentials

The auth-ls fixture expects no credentials but inherits OPENAI_API_KEY, making the suite fail on configured developer machines.

Execution plan: reproduce with a synthetic inherited API key using a subprocess regression in crates/aulo/tests/cli.rs; disable environment inheritance for the isolated auth-ls fixture using trycmd configuration; run the CLI tests with fake credentials, then workspace tests, lint and formatting. Reuse trycmd environment controls instead of adding a custom environment filter.
