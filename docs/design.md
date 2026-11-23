# Design

How aulo's client looks, where the design lives, and how it maps to the code.

## Where

All screens are in the Figma file
[Aulo](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=0-1), page "UI", one
section per platform. They are built only from instances of the published library
[Pyrlyn UIkit](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit), which holds
the UI kits: one page per platform (macOS, Windows 11, Linux, iOS, Android, Web), each with
its own variables, text styles and components. aulo's own components sit in an `aulo-chat`
section of each kit page.

Every screen is also described in Markdown in [aulo-chat/design/](../aulo-chat/design/README.md):
layout, components, states, behaviour, what the code has, and links to its frames. Agents
work from those files; the frames are the picture.

## Platforms

| Platforms | Design | Screens |
| --- | --- | --- |
| [macOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19258), [Windows 11](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21517), [Linux](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23860) | Bot-based window ([desktop.md](../aulo-chat/design/desktop.md)) | 16, each dark and light |
| [iOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-18376), [Android](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-18855), [Web](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-26211) | Chat list and chat | 12 and their flows, each dark and light |

Each platform keeps its own look: macOS glass and traffic lights, Windows 11 Mica, title bar
and cards, Linux (GNOME) header bars and grouped lists. Layout and copy are the same across
the three desktops; only what the OS dictates differs (key store, paths, Trash or Recycle
Bin, hotkey glyphs).

## The desktop app

The desktop design follows spec §12. A **bot** is a named agent with an avatar, a model tier
and a chat. The window shows:

- the bots on the left, with search and New bot;
- the open bot's chat: messages, tool-call cards, voice markers, approval cards and a line
  saying what the bot is doing;
- a Computer panel while the bot uses the computer: its live screen with Take over and Stop.

The sixteen desktop screens are Chat, Approval, Empty state, Voice, Quick talk, Screen
viewer, Attachments, Search, Offline and seven settings tabs (Providers, Models, Voice,
Devices, Extensions, Rules, Connections). [desktop.md](../aulo-chat/design/desktop.md) links
every frame.

## Safety in the UI

The screens show aulo's safety model ([threat-model.md](threat-model.md), spec §7) rather
than hide it:

- Tool calls are visible in the chat with their state; risky ones stop at an approval card,
  and nothing runs until the user decides.
- Destructive, payment and credential actions need a click; the card says that voice cannot
  approve them.
- Keys and tokens appear only as handles; the values stay in the OS key store.
- An MCP server, plugin or speech engine that fails is shown as skipped; the app goes on.
- Take over pauses a bot that controls the computer; Stop ends its run.

## Themes

Every frame has a light twin built from the same layers with the kit's colour collections
switched to Light. The app follows the OS appearance or the user's choice. Details in
[themes.md](../aulo-chat/design/themes.md).

## Status

The code is behind the desktop design: `aulo-chat` builds the earlier chat-list screens on
every platform. Bots, tool calls, approvals, voice, quick talk, the screen viewer and the
desktop settings tabs are not built yet. The language setting (English or Russian UI) has no
frame yet.
