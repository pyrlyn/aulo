# Model settings

Figma: [iOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=50-8364) · [Android](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=50-7686) · [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=120-17491) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-29921)) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=129-19772) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=133-27435)) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=131-22296) ([light](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=132-24008)) · [Web](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=55-3566)

On desktop the model tiers are a settings tab (linked: Settings · Models); the per-bot picker sits in the bot header. See `settings-desktop.md`.

## Purpose

Choose which model answers in the open chat. The choice belongs to the chat, not to the app,
and can be changed in the middle of a conversation.

## Layout

Dark theme only. The screen is the chat with a surface on top of it, opened from the model
picker button:

- **Phones (iOS, Android):** a bottom sheet with a drag handle; the chat behind it is dimmed.
- **Desktop and web:** a popover anchored under the model picker button, about 310–380 dp
  wide. On Linux the popover is glass: it blurs what is behind it. On Web the page behind is
  dimmed.

Inside, top to bottom:

1. Title "Model".
2. Caption "Applies from the next turn." (muted).
3. One model row per model the server lists.
4. A "Done" button, bottom right, that closes the surface.

## Components

| Component | What it is |
| --- | --- |
| Model row | Radio mark, display name (for example "Stub Echo"), and a muted second line: `provider/model · one-line description` (for example "stub/echo · Streams at reading speed"). States: default, selected (accent tint and a filled radio mark), disabled (half opacity, second line ends with "Not served by this server"). |
| Model picker | The button that opens this surface: current model name and a chevron. Shown pressed while the surface is open. |
| Button | The platform kit's primary button, label "Done". |

The mockups list four rows: Stub Echo (selected), Stub Slow ("Takes its time, easy to
stop"), Stub Flaky ("Fails twice before each answer") and Local Llama (disabled, "Not served
by this server").

## States

| State | What the user sees |
| --- | --- |
| Open | The list with the chat's current model selected. |
| A model that is listed but not served | Its row is disabled and cannot be chosen. |
| Switching | The surface closes on choice; the picker button shows the new name at once. |
| Confirmed | An info banner in the chat: "<Model name> answers from the next turn on." |
| Offline | The picker button is disabled and shows the chat's stored model; the surface cannot be opened. |

## Behaviour

- Choosing a row switches the chat's model and closes the surface; choosing the selected row
  only closes it. "Done", a tap outside, or Escape close it without a change.
- The switch never touches an answer that is being written: it applies from the next turn.
- The list comes from the server (`ConfigService.ListModels`) every time the app connects;
  it is not cached. If the chat's model is missing from the list, the button shows its id
  (`provider/model`).
- Model names and descriptions come from the server: draw them as plain text, one line each.

## Errors and offline

- The server refuses an unknown model with an error banner: "Unknown model <id>; the chat
  keeps its model."
- If the connection is lost during the switch, a warning banner "The server cannot be
  reached." appears and the chat keeps its previous model.
- Offline there is no model list at all; see `offline.md`.

## Code

Implemented as a dropdown menu, not as the sheet or popover of the mockups: `ModelPicker` in
`composeApp/src/commonMain/kotlin/aulo/chat/ui/ChatScreen.kt`. Each item shows the display
name and the model id (or "Unavailable" when not served) and a check mark on the current
one; there is no title, caption, description or Done button. Logic:
`ChatViewModel.selectModel`, `SwitchModel` use case, `Submission.SwitchModel` on the
`Converse` stream.
