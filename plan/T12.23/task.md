---
status: in progress
priority: P1
complexity: 5
readiness: 85%
agent: Claude Code / claude-opus-5-5
---

# T12.23. aulo-chat: complete Figma design and the desktop UI built from it

Stage: S12 · Area: apps · Depends on: T12.22

T12.22 left seven screens per platform in Figma (Pyrlyn UIkit, page "aulo-chat · Screens") and
a client that draws only the chat screen, in one column, on every platform. This task makes
Figma the source the desktop client is built from:

1. **Figma, components and tokens.** Every repeated part of a screen (desktop window shell,
   sidebar, chat header, banners) is an instance of a component on its kit page, and every
   colour, radius and text style is bound to that kit's variables and styles. No hardcoded
   fills.
2. **Figma, the missing flows**, on all six platforms (iOS, Android, macOS, Windows 11, Linux,
   Web): settings (server address, appearance, default model), chat actions (rename, delete
   with a confirmation dialog), connecting on first start, search results, and light-theme
   variants of the desktop screens. Each new flow gets a file in `aulo-chat/design/`.
3. **Desktop client from Figma** (Compose Desktop on the JVM; macOS, Windows 11, Linux): the
   two-pane layout with the conversation sidebar, every designed screen and flow, platform
   tokens read from each OS's kit page, light and dark themes.

Done: the Figma page holds every screen and flow for the six platforms, built from components
and variables; `aulo-chat/design/` describes each; the desktop app on macOS, Windows and Linux
matches the desktop frames; tests and builds listed in `aulo-chat/README.md` pass.

## Execution plan

1. Branch `t12.23-desktop-design`, stacked on `t12.22-aulo-chat` (PR pyrlyn/aulo#98); the PR
   opens against `main` once #98 is merged.
2. Write the new screen descriptions: `design/settings.md`, `design/chat-actions.md`,
   `design/connect.md`, `design/search.md`, and the theme rules in `design/themes.md`.
3. Figma, desktop kits first (macOS, Windows 11, Linux): extract the window shell, sidebar and
   chat header into components, swap the raw copies for instances, add the new flows and the
   light variants (frames bound to the kit's Light mode). Then iOS, Android, Web.
4. Desktop code in `composeApp` (`jvmMain` plus shared `commonMain` pieces): sidebar with
   search and conversation rows, chat header with model picker and connection status, the
   attachments menu, settings, rename and delete, connect screen, light and dark themes.
   Missing client calls (`ListChats`, `RenameChat`, `DeleteChat`, `SearchMessages`) go through
   `ChatRepository` in `shared`.
5. Verify: `:composeApp:jvmTest`, `:shared:jvmTest`, `:server:test`, the Android and iOS
   builds, and screenshots of the running desktop app against the Figma frames.
6. Update `aulo-chat/README.md` and the design docs' Figma links; write `done.md`.
