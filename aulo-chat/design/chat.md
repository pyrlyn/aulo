# Chat

Figma: [iOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=50-6029) · [Android](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=50-7498) · [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=118-16430) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29549)) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=128-18675) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-25955)) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=130-21223) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-23596)) · [Web](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=55-3348)

The desktop frames show a bot's chat; see `desktop.md` for the bot-based window.

## Purpose

One conversation with the assistant. The user writes a message, optionally adds attachments,
and sends it; the answer appears token by token and can be stopped at any moment. The screen
also shows which model answers and whether the server is reachable. It is the app's home
screen and the only one implemented in code so far.

## Layout

Dark theme only. Top to bottom:

1. **Top bar** (60 dp). Chat title, one line, ellipsised; "New chat" until the first message
   gives the chat a title. Connection label: an 8 dp dot and "Online" (green dot),
   "Connecting…" or "Offline" (muted dot). Model picker button: the current model's display
   name and a chevron. New chat button.
2. **Progress line** (2 dp, indeterminate) while the chat loads or the app connects.
3. **Banners**, full width, when present: the offline banner, then one notice banner.
4. **Message list.** Fills the rest. Content is centred and at most 760 dp wide; a bubble is
   at most 560 dp wide and keeps 48 dp free on its far side. User messages sit right in the
   accent bubble, assistant messages left on the raised surface. The list is anchored to the
   bottom: the newest message is always the lowest one.
5. **Composer.** A row of staged attachment chips (only when there are any, scrolls
   sideways), then: attach button, text field (grows to 6 lines), Send button. While an
   answer is being written the Send button is replaced by Stop.

Phones (iOS, Android): the top bar has a back button to the conversation list and the title;
the model picker and the connection dot sit in a row under it. Desktop and web (macOS,
Windows 11, Linux, Web): the conversation list is a sidebar on the left (see
`conversation-list.md`) and this screen is the pane on the right.

## Components

| Component | What it is |
| --- | --- |
| Message bubble | Role `user` or `assistant`; state `complete`, `streaming`, `failed`, `interrupted`. Holds attachment chips (above the text), the text, and for `failed` the error line and a Retry text button. |
| Typing indicator | Three 7 dp dots pulsing in turn; shown inside an assistant bubble that has no text yet. |
| Streaming caret | A 2 dp bar blinking after the last character of a streaming answer. |
| Attachment chip | Kind icon (text file, image, voice note), `name · detail` (size or duration). Staged chips have a remove button; chips inside a sent message do not. |
| Composer | Attach button, text field, Send or Stop. |
| Model picker | Button in the top bar; opens the model list (see `model-settings.md`). |
| Banner | Text (up to 3 lines) and one text action. Info, warning and error colours. |

## States

| State | What the user sees |
| --- | --- |
| Loading | Progress line, empty list, composer disabled. |
| Connecting | Progress line, "Connecting…" in the top bar and as the composer placeholder, composer disabled. |
| Idle, online | Messages, placeholder "Message aulo". Send is disabled until there is text or an attachment. |
| Waiting for the first token | The user's message, then an assistant bubble with the typing indicator. Stop replaces Send. |
| Streaming | The assistant bubble's text grows; the caret follows it. Stop replaces Send. |
| Complete | Plain bubble, Send is back. |
| Stopped | The partial answer stays; a muted "Stopped" caption sits under the bubble. |
| Failed | See `error.md`. |
| No messages | See `empty-state.md`. |
| Offline | See `offline.md`. |

## Behaviour

- **Send.** The Send button; on a hardware keyboard Enter sends and Shift+Enter breaks the
  line (on touch keyboards Enter is a line break). The input and the staged attachments are
  cleared at once, so a second tap cannot send twice. Send is enabled only when the server
  is online, a chat is open, no answer is running, and there is text or an attachment.
- **Streaming.** Tokens are appended to the last bubble as they arrive; the list stays
  pinned to the bottom. A new message scrolls the list to itself.
- **Stop.** Interrupts the running answer. What has arrived stays in the chat, marked
  "Stopped", on the server and on the device.
- **Memory.** The server keeps the dialog; every turn is answered with the earlier turns in
  view. Reopening the chat reloads its history.
- **Model switch.** Takes effect from the next turn; the server confirms with an info
  banner such as "Stub Slow answers from the next turn on."
- **Appearance of messages.** A new message fades in and the others slide to make room.
- **Untrusted text.** Whatever the model writes is drawn as plain text: no markdown, no
  links, nothing interpreted. One message is capped at 256 KiB on the device; a turn the
  user sends is capped at 64 KiB.
- **Attachments.** Text files travel inside the message text. Images and voice notes are
  staged and shown but not transmitted yet; after sending, an info banner says "Images and
  voice notes are not sent yet; the text was."

## Errors and offline

- A turn that fails leaves a `failed` assistant bubble with the reason and Retry; a notice
  from the server shows as a banner with Dismiss. Details in `error.md`.
- When the server cannot be reached the offline banner appears, the cached messages stay on
  screen and the composer, the attach button, the model picker and New chat are disabled.
  Details in `offline.md`.

## Code

Implemented: `composeApp/src/commonMain/kotlin/aulo/chat/ui/ChatScreen.kt` (`ChatRoute`,
`ChatScreen`, top bar, banners, list), `Composer.kt`, `MessageBubble.kt`; state in
`shared/.../presentation/ChatViewModel.kt` (`ChatUiState`). The code has no conversation
list yet, so the back button and the sidebar of the mockups are absent, and the top bar
keeps the model picker and New chat on every platform. The attach button is a plus icon in
code and a paperclip in the mockups.
