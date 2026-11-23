# Error

Figma: [iOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=50-6308) · [Android](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=50-8035) · [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=50-7274) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=51-2463) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=51-3611) · [Web](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=56-3843)

## Purpose

How the chat screen looks when something went wrong while the server is still reachable: a
turn was refused or failed, an answer was stopped, or the server sent a notice. Losing the
server altogether is a different screen (`offline.md`).

## Layout

Dark theme only. The chat screen as in `chat.md`, with up to three additions, all shown in
the mockups at once:

1. **Notice banner** under the top bar, full width of the chat pane: an alert icon, the
   message (up to 3 lines), and a "Dismiss" action. Error colours in the mockups: "Rate
   limit reached for this chat; try again in 12 s."
2. **Failed bubble.** An assistant bubble whose content is the reason in the error colour
   and a "Retry" text button under it: "The model is unavailable after 3 attempts: stub
   provider overloaded". If part of the answer had arrived, that text stays above the
   reason.
3. **Stopped answer.** A normal assistant bubble with the partial text and a muted "Stopped"
   caption under it.

The composer stays enabled: an error never locks the chat.

## Components

| Component | What it is |
| --- | --- |
| Banner | Levels: info (blue), warning (amber), error (red). Icon, text, one text action ("Dismiss"). |
| Message bubble, `failed` | Reason line in the error colour and a Retry text button. On desktop kits the bubble also gets an error-coloured outline or tint. |
| Message bubble, `interrupted` | Normal bubble with the "Stopped" caption. |

## States

| Cause | Banner | Bubble |
| --- | --- | --- |
| Rate limit (a burst of 5 turns, then 30 per minute, per chat) | Warning: "Rate limit reached for this chat; try again in N s." | Failed, same text, Retry. |
| Server queue full (2 turns run at once, 8 wait) | Warning: "The server is busy (8 turns waiting); try again shortly." | Failed, same text, Retry. |
| Waiting in the queue | Info: "Queued: position N." | Typing indicator. |
| The model failed temporarily and is being retried | Warning: "<reason>; retry N of 2 in M ms." | Typing indicator; the turn continues. |
| The model failed for good (3 attempts, or after the first token) | — | Failed: "The model is unavailable after 3 attempts: <reason>", Retry. |
| The model refused the turn | — | Failed with the model's reason, Retry. |
| Connection lost in the middle of an answer | The offline banner (`offline.md`) | Failed: "The connection was lost before the answer finished." |
| The user pressed Stop | — | Partial text, "Stopped". |
| A second message while an answer is running | Warning: "A turn is already running; stop it before sending another." | — |
| Message larger than 64 KiB | Error: "The message is N KiB; the limit is 64 KiB." | Nothing is sent; the text returns to the composer. |
| Unknown model chosen | Error: "Unknown model <id>; the chat keeps its model." | — |

## Behaviour

- **Retry** on a failed bubble sends the user message that the failed answer belonged to
  once more, as a new turn; the failed bubble stays in the history.
- **Dismiss** removes the banner. There is one notice at a time: a newer notice replaces the
  older one, and sending a message clears it.
- The server retries a temporary model failure itself, with a growing pause, but only while
  no token has reached the client: after the first token a retry would print the answer
  twice, so the turn fails instead.
- Error texts come from the server: draw them as plain text. A notice is capped at 500
  characters on the device.
- Nothing here is modal. There is no error dialog and no full-screen error.

## Errors and offline

This screen is the error behaviour. If the failure is that the server cannot be reached,
the chat goes to the offline state (`offline.md`): the composer is disabled there, while on
this screen it stays enabled.

## Code

Implemented: `NoticeBanner` and `Banner` in
`composeApp/src/commonMain/kotlin/aulo/chat/ui/ChatScreen.kt`; the failed and stopped
variants in `MessageBubble.kt`; `ChatViewModel.retry` and `dismissNotice`. In code the
banner has no icon and the failed bubble has no outline, and the rate-limit banner is amber
(the server sends it at warning level) where the mockups draw it red. Server side: `RateLimiter`,
`RequestQueue` and `retryTransient` in `server/src/main/kotlin/aulo/chat/server/Pipeline.kt`.
