# T0.2. Decide the desktop UI toolkit

Stage: S0 · Area: decision · Depends on: none · Blocks: 1 task(s)

Candidates: native SwiftUI/WinUI over a C ABI exported from the Kotlin/Native core (the package manager and the coding agent pattern, best platform feel, two codebases), Compose Multiplatform (one Kotlin codebase), Tauri or a web UI (heavier). Record the choice and the reason. D2 currently records SwiftUI and WinUI 3; confirm or change that on the Kotlin/Native stack.

Done when:

- spec.md Decisions names the toolkit
