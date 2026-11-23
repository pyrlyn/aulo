---
status: in progress
priority: P1
complexity: 2
readiness: 90%
agent: Claude Code / claude-opus-5-5
---

# T12.24. Bot-based desktop screens: design docs and frame links

The desktop sections (macOS, Windows 11, Linux) of the Figma page `aulo-chat · Screens` were
rebuilt around bots, following spec §12: every chat belongs to a bot with its own avatar, tool
calls and approvals sit in the chat, and settings cover providers, models, voice, devices,
extensions, rules and connections. The twelve old desktop frames per platform and their light
twins were deleted, so the desktop links in `aulo-chat/design/` point at nothing.

Done means: `aulo-chat/design/` describes the new desktop screens, every desktop link points
at a live frame (dark and light), the components added to the kits are listed with links, and
`docs/design.md` explains where the design lives and how to work with it. iOS, Android and Web
frames are unchanged and keep their links.

## Plan

1. `aulo-chat/design/README.md`: entry point for agents (Figma file, page, sections, naming,
   grid, rules, file index).
2. `aulo-chat/design/desktop.md`: the desktop layout, a link table of all 16 screens × dark and
   light × 3 platforms, and the aulo-chat kit components per platform.
3. New screen files: `approval.md`, `voice.md`, `quick-talk.md`, `screen-viewer.md`,
   `settings-desktop.md`.
4. Existing files: replace the dead desktop links; say where a flow has no desktop frame now
   (error, chat actions, connect). Update `themes.md` (light rows sit under the dark rows on
   desktop) and the Design section of `aulo-chat/README.md`.
5. `docs/design.md` and a row in `docs/README.md`.

Check: every `node-id` in `aulo-chat/design/` and `docs/design.md` resolves to a frame on the
page (read-only Figma script), and no desktop link points at a deleted frame.
