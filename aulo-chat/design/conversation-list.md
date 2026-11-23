# Conversation list

Figma: [iOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-18392) · [Android](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-18866) · [macOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19259) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20311)) · [Windows 11](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21518) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22606)) · [Linux](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23861) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24957)) · [Web](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-26228)

On desktop the list is the bot sidebar, present in every desktop frame (linked: Chat); see `desktop.md`.

## Purpose

All of the user's chats, newest first: pick one to continue it, search them, or start a new
one. On phones it is a screen of its own, one level above the chat. On desktop and web it is
the left sidebar of the window and stays visible next to the chat (the frames show it with a
chat that has finished answering).

## Layout

Dark theme only.

- **Header.** Title "Chats". Phones: a large title (iOS) or a top app bar (Android).
  Desktop and web: the sidebar's first row; Windows 11, Linux and Web also show the app or
  window chrome there.
- **New chat.** The primary action. iOS: a filled pill under the title. Android: an extended
  floating button, bottom right. Desktop and web: a full-width button at the top of the
  sidebar (Linux: in the header row).
- **Search field.** Placeholder "Search chats". iOS: at the bottom, above the home
  indicator. Elsewhere: under the header.
- **List.** One row per chat.
- **Footnote.** "The 20 most recent chats are kept on this device." Muted, under the list
  (desktop: at the bottom of the sidebar).

Sidebar width on desktop and web is about 220–260 dp of a 1120 dp window.

## Components

| Component | What it is |
| --- | --- |
| Conversation row | Title (one line, semibold), preview of the last message (one line, muted, ellipsised), time of the last change (right: "now", "12 min", "1 h", "Yesterday", "Mon"), and a small tag with the chat's model name. States: default, hover or pressed, selected. |
| Search field | The platform kit's search field. |
| Button | The platform kit's primary button, label "New chat" (with a plus icon where the kit uses one). |
| Tag | The platform kit's tag, used for the model name. |

## States

| State | What the user sees |
| --- | --- |
| Loaded | Rows, newest first; the open chat's row is selected (desktop and web). |
| Loading | The rows from the device cache at once; they are replaced when the server answers. |
| No chats | No rows; the chat pane shows the empty state (see `empty-state.md`). On phones the list shows only the header, New chat and the footnote. |
| Searching | Only rows whose title or messages match the query; no match shows a muted "No chats found". |
| Offline | The cached rows only (at most 20); New chat is disabled. See `offline.md`. |

## Behaviour

- Choosing a row opens that chat. On phones the list is replaced by the chat and the chat's
  back button returns here; on desktop and web the row becomes selected and the chat pane
  changes.
- New chat creates an empty chat on the server and opens it with the empty state. A chat
  gets its title from its first message.
- Rows are ordered by the time of the last message. The row of a chat that is answering
  moves to the top when the answer is saved.
- A title or preview is model- or user-written text: draw it as plain text, one line.

## Errors and offline

- If the list cannot be loaded, the cached rows stay and the offline banner of the chat pane
  explains why (`offline.md`); the list itself shows no error.
- If New chat fails because the server went away, a warning banner "The server cannot be
  reached." appears in the chat and nothing is created.
- Only the 20 most recent chats, with up to 500 messages each, are stored on the device;
  older ones need the server.

## Code

- Desktop and other wide windows: `Sidebar` in `composeApp/.../ui/Sidebar.kt`, beside the
  chat in `ChatWindow.kt`. Rows show the title, the model's display name and a relative time
  (`relativeTime`), refreshed every minute.
- Data: `ChatRepository.chats` (a flow of `ChatSummary` from the SQLDelight cache, refreshed
  from `ChatService.ListChats`), `ChatViewModel.openChat` and `newChat`.
- Phones: not built yet. The app opens the most recent chat and offers New chat in the
  chat's top bar.
