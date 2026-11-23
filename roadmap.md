# aulo — roadmap

The stack is **Kotlin/Native** (Gradle KMP, native targets). Every approved task is already
in `plan/`, and all of them are open again after the reset. New approved work lands here
first and moves to `plan/<id>/` when it is scheduled.

- **API token management** (approved in T3.4; spec §11). Per-method token scopes, and issuing, listing and revoking tokens. T3.4 stores token hashes with a scope column and gives the first token `admin`.
