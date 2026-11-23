# T0.2. Done

`spec.md` §17 **D2** names the toolkit: native SwiftUI (macOS) and WinUI 3 (Windows) over
`aulo-app` and `aulo-ffi`, a C ABI. On the Kotlin/Native stack that C ABI is exported by the
Kotlin/Native core, so the choice carries over without change; Compose Multiplatform and a web
UI stay rejected.

The reset (#81) removed this report together with every other `done.md`. Closed again in the
pull request that adds this file, on the creator's word.

Checked: `spec.md` §17 D2 is present on `main` at 9d360a6.
