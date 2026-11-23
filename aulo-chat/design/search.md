# Search

Figma: [iOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-18599) · [Android](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19043) · [macOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19441) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20493)) · [Windows 11](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21724) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22812)) · [Linux](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24071) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25167)) · [Web](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-26432)

On desktop the results split into Bots and Messages in the sidebar; see `desktop.md`.

## Purpose

Find a chat by its title or by what was said in it.

## Layout

The conversation list (phones) or the sidebar (desktop, web) with a query in the search
field ("lisbon" in the mockups). The list is replaced by results in two groups:

1. **Chats** — conversation rows whose title matches, the match in bold.
2. **Messages** — one row per matching message: chat title (muted, small), then a one-line
   excerpt with the match in bold, and the time on the right.

A clear button (×) sits in the search field. With no results: the muted line
"No chats found" centred in the list.

## Components

| Component | What it is |
| --- | --- |
| Search field | The kit's search field, focused, with the query and a clear button. |
| Section header | The kit's list section header: "Chats", "Messages". |
| Conversation row | As in `conversation-list.md`, with the match highlighted. |
| Message hit row | Chat title, excerpt with the match highlighted, time. |

## States

| State | What the user sees |
| --- | --- |
| Typing | Title matches from the local cache appear at once. |
| Searching the server | A small spinner at the right of the field until message hits arrive. |
| Results | Both groups; an empty group is hidden. |
| No results | "No chats found". |
| Offline | Only the "Chats" group from the cache, and the muted line "Message search needs the server." |

## Behaviour

- Title matches come from the local cache; message hits from `ChatService.SearchMessages`,
  sent 300 ms after the last keystroke.
- Choosing a message hit opens its chat and scrolls to the message.
- Escape or the clear button empties the query and restores the list. Cmd+F or Cmd+K
  (macOS), Ctrl+F or Ctrl+K (Windows, Linux, Web) focuses the field.
- Excerpts come from the server: drawn as plain text, one line, cut with an ellipsis.

## Errors and offline

A failed server search keeps the title matches and shows "Message search failed: <reason>."
in place of the Messages group.

## Code

- Field and results: `Sidebar` and `SearchResults` in `composeApp/.../ui/Sidebar.kt`;
  `highlight` bolds every query word.
- State: `ChatViewModel.onQueryChange` and `openHit`; title matches from `ChatCache`,
  message hits from `ChatRemote.searchMessages`; the server side is `ChatStore.search`.
- Shortcuts: `windowShortcut` in `ChatWindow.kt`.
- Not done yet: opening a message hit opens its chat but does not scroll to the message.
  Search exists in the wide (desktop) layout only; phones get it with their conversation list.
