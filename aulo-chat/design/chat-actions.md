# Chat actions

Figma, chat actions: [iOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-11885) · [Android](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-12319) · [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=68-8957) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=68-7322) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=69-8676) · [Web](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-15428)

Figma, rename chat (phones): [iOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-11992) · [Android](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-12358)

Figma, delete chat: [iOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-12029) · [Android](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-12386) · [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=68-8984) · [Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=68-7433) · [Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=69-8785) · [Web](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=70-15523)

## Purpose

Rename a chat or delete it, from the conversation list or from the open chat.

## Layout

- **Where the menu opens.** Phones: a long press on a conversation row (iOS context menu,
  Android dropdown menu anchored to the row) and the overflow button in the chat's top bar.
  Desktop and web: a right click on a sidebar row, or the "⋯" button that appears on the
  hovered or selected row, and the same button in the chat header.
- **Menu items:** "Rename", a separator, "Delete" in the danger colour.
- **Rename.** Phones: an alert dialog with a text field prefilled with the title, "Cancel"
  and "Save". Desktop and web: the row's title turns into an inline text field; Enter saves,
  Escape cancels.
- **Delete.** A confirmation dialog in the platform kit's style: title "Delete this chat?",
  body "<Chat title> and its messages are deleted on the server and on this device. This
  cannot be undone.", buttons "Cancel" and "Delete" (destructive). On macOS it is a sheet
  attached to the window; on Windows a ContentDialog; on Linux an Adwaita message dialog; on
  Android a Material dialog; on iOS an action sheet; on Web a modal dialog.

## Components

| Component | What it is |
| --- | --- |
| Context menu | The kit's menu: items with icons (pencil, trash), destructive item in the danger colour. |
| Inline rename field | Conversation row with its title replaced by a single-line text field. |
| Confirmation dialog | The kit's dialog: title, body, secondary and destructive buttons. |

## States

| State | What the user sees |
| --- | --- |
| Menu open | The row stays highlighted while its menu is open. |
| Renaming | Inline field or alert with the title selected. |
| Saving a rename | The new title shows once the server accepts it; on failure the old title stays and an error banner says why. |
| Confirming delete | The dialog; the default button is Cancel, so Enter does not delete. |
| Deleted | The row disappears; if it was the open chat, the next chat opens, or the empty state. |

## Behaviour

- Delete always needs a click or key on the dialog's Delete button: never a spoken yes, never
  a single key without the dialog.
- A title is trimmed and at most 200 bytes (`RenameChatRequest`); Save is disabled while the
  field is blank, because the server rejects an empty title.
- Rename and delete go to the server (`ChatService.RenameChat`, `ChatService.DeleteChat`), then
  update the local cache.

## Errors and offline

Offline, Rename and Delete are disabled in the menu with the muted hint "Needs the server".
A server refusal shows an error banner and leaves the chat as it was.

## Keys

F2 renames the open chat (desktop). Delete has no key of its own: a Delete key would also fire
while typing in the composer. The "Del" hint drawn in the Windows and Linux menus is therefore
not shown in the app; delete starts from the menu only.

## Code

- Menu: `ChatMenu` in `composeApp/.../ui/Sidebar.kt`, opened from a row (right click or "⋯")
  and from the chat header in `ChatWindow.kt`.
- Inline rename: `RenameField` in `Sidebar.kt`. Delete dialog: `ConfirmDialog.kt`, where
  Cancel is the focused button.
- State: `ChatViewModel.rename` and `delete`, then `ChatRepository` and the server's
  `renameChat` and `deleteChat` in `Services.kt`.
- Phones get the long-press menu and the rename alert with their conversation list.
