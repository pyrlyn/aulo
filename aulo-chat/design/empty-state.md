# Empty state

Figma: [iOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-18410) · [Android](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-18926) · [macOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19308) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20360)) · [Windows 11](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21573) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22661)) · [Linux](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23915) ([light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25011)) · [Web](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-26310)

On desktop the empty state belongs to a new bot ("Untitled bot"); see `desktop.md`.

## Purpose

What the chat screen shows when the open chat has no messages: a new chat, or the first
start of the app. It tells the user what the screen is for and offers three ready prompts so
the first message is one tap away.

## Layout

Dark theme only. The chat screen as in `chat.md` (top bar with the title "New chat", the
model picker and the connection label; the composer at the bottom), with the message list
replaced by a block centred in the free space:

1. A round badge with a sparkle icon in the accent colour.
2. Heading "What can I help with?", centred.
3. Body "Ask anything. Answers stream in as they are written.", muted, centred.
4. Three suggestion chips: "Summarise a document", "Draft a reply", "Explain an error".
   Phones stack them in a column; desktop and web put them in one row.

On desktop and web the conversation list sidebar is still there and no row is selected.

## Components

| Component | What it is |
| --- | --- |
| Empty state | The badge, heading and body as one centred block. |
| Suggestion chip | A pill with the prompt text (Android adds a small sparkle icon). States: default, hover or pressed. |
| Composer, top bar | As in `chat.md`. |

## States

| State | What the user sees |
| --- | --- |
| Empty, online | The block above; composer enabled with the placeholder "Message aulo"; Send disabled until there is text. |
| Loading | Nothing in the middle (no block yet) and the progress line under the top bar: the empty state appears only once it is known that the chat has no messages. |
| Empty, offline | The same block; the offline banner above it; the composer disabled with "Reconnect to send a message". Suggestion chips still fill the input, but nothing can be sent. |
| First message sent | The block is replaced by the message list. |

## Behaviour

- Tapping a suggestion puts its text into the composer. It does not send: the user can edit
  it first.
- The block scrolls when the space is too small for it (for example when a phone keyboard is
  up), so every suggestion stays reachable.
- The chat's title stays "New chat" until its first message.

## Errors and offline

- A notice or the offline banner appears above the block, as on the chat screen; the block
  itself never shows an error.
- With no connection and nothing cached (the very first start without a server) the app has
  no chat to open: the screen shows the empty block, the offline banner with Retry, and a
  disabled composer.

## Code

Implemented: `EmptyState` in `composeApp/src/commonMain/kotlin/aulo/chat/ui/ChatScreen.kt`,
shown when `ChatUiState.empty` is true (`!loading && messages.isEmpty()`). The code has the
heading, the body and the three suggestions in a column on every platform; the badge with
the sparkle icon is not drawn.
