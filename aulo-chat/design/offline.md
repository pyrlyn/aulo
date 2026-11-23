# Offline

Figma: [iOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-18435) · [Android](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-18944) · [macOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19469) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20521)) · [Windows 11](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21756) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22844)) · [Linux](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24105) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25201)) · [Web](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-26337)

On desktop the banner reads "Can't reach aulod on this Mac" and the chat stays read-only; see `desktop.md`.

## Purpose

How the chat screen looks when the server cannot be reached: at start, or after the
connection drops. The user can still read the recent chats that are saved on the device,
but cannot send anything until the connection is back.

## Layout

Dark theme only. The chat screen as in `chat.md`, with these differences:

1. **Top bar.** The connection label shows a muted dot and "Offline".
2. **Offline banner** under the top bar, full width of the chat pane, warning (amber)
   colours: an offline icon, "Offline — showing chats saved on this device.", and a "Retry"
   action.
3. **Message list.** The messages of the open chat as they were last saved on the device.
   An answer that was cut off by the disconnect ends in a failed bubble: "The connection was
   lost before the answer finished."
4. **Composer.** Disabled: the placeholder reads "Reconnect to send a message", and the
   attach and Send buttons are dimmed.

On desktop and web the sidebar keeps showing the cached conversation rows.

## Components

| Component | What it is |
| --- | --- |
| Banner, warning | Icon, text, the "Retry" action. |
| Connection label | Dot and "Offline". |
| Composer, disabled | Dimmed field and buttons, the offline placeholder. |
| Message bubble | As in `chat.md`; no streaming variant can appear here. |

## States

| State | What the user sees |
| --- | --- |
| Offline with a cached chat | The banner, the cached messages, the disabled composer. |
| Offline with nothing cached | The banner, the empty state (`empty-state.md`), the disabled composer. |
| Reconnecting (after Retry or at start) | The progress line, "Connecting…" in the top bar and as the composer placeholder; the banner is hidden while the attempt runs. |
| Back online | The banner disappears, the history is reloaded from the server, the model list is loaded again, the composer is enabled. |
| Retry failed | The banner returns. |

## Behaviour

- **Retry** tries the server again and reopens the chat. The app does not reconnect by
  itself; Retry is the only way back.
- **What is saved.** The 20 most recent chats, with up to 500 messages each, in a SQLite
  database on the device (SQLDelight). The cache is written as messages arrive, so what was
  on screen before the drop is there after it. Older chats are dropped from the device, not
  from the server.
- **What is disabled.** The text field, Send, the attach button, the model picker and New
  chat. Reading and scrolling stay available. Text typed before the drop stays in the field.
- **Model picker.** There is no model list offline; the button shows the model stored with
  the chat (its `provider/model` id) and does not open.
- **A message being sent when the connection drops** is not lost silently: its text and
  attachments return to the composer with a warning banner "The server cannot be reached."

## Errors and offline

This screen is the offline behaviour. Errors that happen while the server is reachable
(rate limit, model failure, a stopped answer) are in `error.md`; the difference to remember
is that an error leaves the composer enabled and offline disables it.

## Code

Implemented: the banner in `ChatScreen` (`ConnectionState.Offline`), `ConnectionLabel`, the
composer's placeholder and disabled state in `Composer.kt`, `ChatViewModel.connect` behind
Retry. Data: `DefaultChatRepository` falls back to `ChatCache`
(`shared/src/commonMain/sqldelight/aulo/chat/data/cache/Chat.sq`). The code's banner has no
icon. `EndToEndTest` covers the round trip: the server is shut down, the cached chat stays
on screen, the server returns, Retry brings the chat back.
