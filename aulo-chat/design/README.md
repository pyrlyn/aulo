# aulo-chat design

Markdown descriptions of every screen of the chat client, with links to the Figma frames.
Read the file of a screen before building or changing it; the frame is the picture, the file
is the contract (layout, components, states, behaviour, what the code has).

Figma: [Pyrlyn UIkit](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit),
page [aulo-chat · Screens](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=49-227).
The human-facing overview is [docs/design.md](../../docs/design.md).

## Two designs

| Platforms | Design | Start with |
| --- | --- | --- |
| macOS, Windows 11, Linux | Bot-based window: bot sidebar, chat with tool calls and approvals, Computer panel, settings window with seven tabs | [desktop.md](desktop.md) |
| iOS, Android, Web | Chat list and chat, settings sheet | [chat.md](chat.md), [conversation-list.md](conversation-list.md) |

## Files

| File | Screen |
| --- | --- |
| [desktop.md](desktop.md) | Desktop window, link table of all desktop frames, kit components |
| [chat.md](chat.md) | Chat |
| [approval.md](approval.md) | Approval card in the chat (desktop) |
| [empty-state.md](empty-state.md) | A chat or bot with no messages |
| [voice.md](voice.md) | Push to talk with live transcript (desktop) |
| [quick-talk.md](quick-talk.md) | Floating quick-talk window on a global hotkey (desktop) |
| [screen-viewer.md](screen-viewer.md) | A bot's screen with Take over and Stop (desktop) |
| [attachments.md](attachments.md) | Attaching files and voice notes |
| [search.md](search.md) | Search results |
| [offline.md](offline.md) | Server unreachable |
| [error.md](error.md) | A failed turn and server notices |
| [conversation-list.md](conversation-list.md) | Chat list (phones, Web) and the desktop sidebar |
| [model-settings.md](model-settings.md) | Model picker |
| [chat-actions.md](chat-actions.md) | Rename and delete a chat |
| [connect.md](connect.md) | Connect on first start |
| [settings.md](settings.md) | Settings on phones and Web |
| [settings-desktop.md](settings-desktop.md) | Desktop settings: Providers, Models, Voice, Devices, Extensions, Rules, Connections |
| [themes.md](themes.md) | Dark and light frames, variable modes |

## Working with the frames

- **Links.** A link is `https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=<id>`
  with the node id's colon written as a hyphen (`118:16430` → `118-16430`). The second line
  of each screen file links its frames.
- **Names.** Frames are named `<Platform> / <Screen>` and `<Platform> / <Screen> · Light`.
  Platforms: `iOS`, `Android`, `macOS`, `Windows 11`, `Linux`, `Web`.
- **Grid.** One section per platform. Desktop sections: dark row at y 120, light row at
  y 1060, columns every 1440 in the order of `desktop.md`.
- **Components.** Screens use only components from the kit pages (the `aulo-chat` section of
  each platform's kit page plus the kit's own components). A missing component is added to
  that kit page, rebuilt natively per platform, never drawn locally in a screen and never
  instanced from another platform's kit.
- **Tokens.** Fills, strokes, radii and text styles are bound to the kit's variables and
  styles. A light frame differs from its dark frame only in explicit variable modes
  (`themes.md`).
- **Bots.** Every bot has its own avatar or the default one. Names in mockups are invented
  (Juno, Wren, Ops, Scribe, Tally, "Untitled bot").
- **Copy.** English. It must not contradict aulo's hard rules: policy decides, not the model;
  voice never approves destructive, payment or credential actions; secrets appear only as
  handles; broken extensions are skipped.
- **Changing a screen.** Change the frame and its light twin together, then the screen's
  file here. A new screen gets a new file and a row in this table and, on desktop, a row in
  `desktop.md`. Deleting a frame means fixing every link to it.
