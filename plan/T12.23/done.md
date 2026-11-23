# T12.23. Done

Branch `t12.23-desktop-design`, stacked on `t12.22-aulo-chat` (pyrlyn/aulo#98) and merged with
it in `a9247ec`. Commits: `63f5b70`, `0c92742`, `b1dc145`, `20f4ba5`, `f76afb5`, `28c29df`,
`4db6618`, `488ef38`. The PR opens against `main` once #98 is merged.

## What was done

- Figma, page `aulo-chat · Screens` of Pyrlyn UIkit:
  - The new flows on all six platforms (iOS, Android, macOS, Windows 11, Linux, Web): settings,
    chat actions, delete confirmation, connect on first start and search results, plus a rename
    sheet on the phones.
  - Every platform section has a light row to the right of its dark row.
  - Repeated parts were extracted into components on each kit's page, and fills were bound to
    the kits' variables.
- `aulo-chat/design/`: `settings.md`, `chat-actions.md`, `connect.md`, `search.md` and
  `themes.md`. Each file links its frames on the six platforms and says what the code has.
- `shared`:
  - Rename, delete and message search in `ChatRepository`.
  - A settings table for the server address, the appearance and the default model, with a
    migration for caches written by T12.22.
  - A remote that switches servers only after the new one answers, so a mistyped address never
    drops the working connection.
  - The development server implements `SearchMessages`.
- `composeApp`:
  - Light and dark tokens for every platform, read from the kits' variables.
  - At 720 dp and wider, a two-pane window: a sidebar with search and conversation rows, and
    the chat with a header menu for rename and delete.
  - A confirmation dialog, a settings screen and a connect screen.
  - Untitled chats are called "New chat" everywhere.
- `aulo-chat/README.md`: the twelve screens and where each one lives in the code, the 720 dp
  layout, and the tests.

## How it was checked

- `./gradlew build :server:installDist :androidApp:assembleRelease
  :composeApp:linkDebugFrameworkIosSimulatorArm64` passes after the merge with #98, with
  dependency verification on. That covers `:shared:jvmTest`, `:composeApp:jvmTest` and
  `:server:test`.
- New tests:
  - `ChatWindowTest`: header menu, rename keys, offline actions, search, settings and the
    connect screen.
  - `SettingsTest`: addresses, the settings round trip, the cache migration, and a server that
    does not answer.
  - `ChatViewModelTest`: rename, search and the default model.
  - `SearchTest` on the server.
- The desktop app ran on macOS in light mode: connect screen, Connect, then the sidebar and
  empty state.

## Not done

- **Desktop beyond macOS.** The desktop app was not run on Windows or Linux, and the running
  app was not compared frame by frame with the Figma screens. Only the macOS run above was
  checked.
- **Figma, still to bind or add:**
  - The Web kit has no radius variables.
  - The radii of the Android and iOS kit components are not bound.
  - The iOS light scrims use `bg/base`.
  - The Linux kit has no window-shell component.
- **Code gaps:**
  - Opening a message hit from search opens its chat but does not scroll to the message.
  - Phones still show one chat, with no list, search, chat actions or settings entry.
  - The Delete key does not delete a chat. That is on purpose: deleting takes the menu and a
    confirmation.
