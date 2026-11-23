# Desktop screens

The desktop app (macOS, Windows 11, Linux) is drawn around **bots**. A bot is a named agent
with its own avatar, model tier and chat. The window lists the bots on the left and shows the
open bot's chat on the right; while a bot works on the computer, a Computer panel on the far
right shows its screen. Phones and Web keep the older chat-list design described in the other
files.

Figma: section [macOS](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19258) ·
[Windows 11](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21517) ·
[Linux](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23860) in the
Aulo file.

## Window

Every desktop frame is 1360 × 820 and has the same shell:

1. **Sidebar** (left): search field, New bot, one Bot row per bot (avatar, name, last line,
   time), and a footer with the connection ("aulod · this Mac" / "this PC" / "this computer")
   and Settings.
2. **Bot header**: sidebar toggle, the bot's avatar, name and role, the model picker, the
   screen toggle (eye) and a More menu. On Linux the window controls sit at the right end of
   this row; macOS and Windows keep their own title bar controls.
3. **Messages**: user bubbles, assistant bubbles, Tool call cards, Voice markers, Approval
   cards and the Activity line ("Opening Finder…"), anchored to the bottom.
4. **Composer**: attach, text field, Send or Stop, and push to talk.
5. **Computer panel** (right, only while a bot uses the computer): the live screen, Take over
   and Stop, where it runs (this machine, a server, off) and the voice engines. Its footnote
   says "Every tool call passes policy before it runs."

Sample bots: Juno (Computer), Wren, Ops, Scribe, Tally and an "Untitled bot". Every bot has an
avatar; a new bot gets the default one (a sparkle on a neutral tile) until it has a name.

## Screens

Dark frames form the first row of each section, light twins (`… · Light`) the row under it, in
this order:

| Screen | Described in | macOS | Windows 11 | Linux |
| --- | --- | --- | --- | --- |
| Chat | [chat.md](chat.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19259) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20311) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21518) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22606) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23861) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24957) |
| Approval | [approval.md](approval.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21363) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21440) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23694) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23777) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-26053) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-26132) |
| Empty state | [empty-state.md](empty-state.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19308) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20360) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21573) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22661) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23915) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25011) |
| Voice | [voice.md](voice.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19331) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20383) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21600) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22688) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23944) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25040) |
| Quick talk | [quick-talk.md](quick-talk.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19496) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20548) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21787) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22875) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24138) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25234) |
| Screen viewer | [screen-viewer.md](screen-viewer.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19359) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20411) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21632) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22720) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23978) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25074) |
| Attachments | [attachments.md](attachments.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19411) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20463) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21690) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22778) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24035) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25131) |
| Search | [search.md](search.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19441) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20493) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21724) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22812) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24071) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25167) |
| Offline | [offline.md](offline.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19469) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20521) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21756) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22844) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24105) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25201) |
| Settings · Providers | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19518) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20570) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21805) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22893) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24159) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25255) |
| Settings · Models | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19631) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20683) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21919) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23007) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24273) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25369) |
| Settings · Voice | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19742) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20794) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22032) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23120) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24385) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25481) |
| Settings · Devices | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19855) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20907) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22146) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23234) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24499) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25595) |
| Settings · Extensions | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-19972) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21024) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22263) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23351) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24616) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25712) |
| Settings · Rules | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20085) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21137) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22377) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23465) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24730) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25826) |
| Settings · Connections | [settings-desktop.md](settings-desktop.md) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-20202) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-21254) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-22494) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-23582) | [dark](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-24847) · [light](https://www.figma.com/design/ZC7LyO9Op91vXWL3M7riF5/Aulo?node-id=11-25943) |

The flows "Error", "Chat actions", "Delete chat" and "Connect" have no desktop frame in this
design: errors show inline in the chat and in the offline banner, chat actions live in the
More menu, and connections are a settings tab. Their files keep the phone and Web links.

## Components

Each kit page has an `aulo-chat` section with the components these screens are built from.
Each platform has its own copy built from its kit's variables and text styles; a component is
never instanced across platforms.

| Component | macOS | Windows 11 | Linux |
| --- | --- | --- | --- |
| `aulo-chat/Bot avatar` | [77:2011](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=77-2011) | [95:1474](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1474) | [96:993](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-993) |
| `aulo-chat/Bot row` | [77:2032](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=77-2032) | [95:1494](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1494) | [96:1012](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1012) |
| `aulo-chat/Bot header` | [79:2039](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=79-2039) | [95:1545](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1545) | [96:1061](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1061) |
| `aulo-chat/Tool call` | [79:1965](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=79-1965) | [95:1510](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1510) | [96:1028](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1028) |
| `aulo-chat/Voice marker` | [79:1975](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=79-1975) | [95:1511](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1511) | [96:1029](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1029) |
| `aulo-chat/Activity line` | [79:1981](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=79-1981) | [95:1517](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1517) | [96:1033](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1033) |
| `aulo-chat/Approval card` | [79:2008](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=79-2008) | [95:1522](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=95-1522) | [96:1038](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=96-1038) |
| `aulo-chat/Settings row` | [115:2052](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=115-2052) | [125:1516](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=125-1516) | [125:2336](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=125-2336) |
| `aulo-chat/Settings tab` | [115:1988](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=115-1988) | — | — |
| `aulo-chat/Switch` | [115:1983](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=115-1983) | — | — |

Kit sections: [macOS](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=67-3614) ·
[Windows 11](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=67-1308) ·
[Linux](https://www.figma.com/design/DYzhrhrlKOy09vNjp6JgAz/Pyrlyn-UIkit?node-id=68-7964).
Windows and Linux settings use their kits' own switch and navigation item, so they have no
`Settings tab` or `Switch` of their own.

| Component | Properties |
| --- | --- |
| Bot avatar | `Initial`, `Avatar` (Default, Coral, Amber, Mint, Teal, Violet, Rose), `Size` (Small, Medium, Large) |
| Bot row | `Name`, `Preview`, `Time`, `Show time`, `State` (Default, Selected) |
| Bot header | `Name`, `Role` |
| Tool call | `Tool`, `Detail`, `Show detail`, `State` (Done, Running, Failed) |
| Voice marker | `Label` ("Voice · 0:06", "Listening · 0:04") |
| Activity line | `Text` ("Opening Finder…") |
| Approval card | `Title`, `Body`, `Detail`, `Hint`, buttons Allow once, Always allow in …, Deny |
| Settings row | `Control` (None, Value, Status, Switch on, Switch off, Picker, Button); `Title`, `Detail`, `Show detail`, `Show avatar`, `Value` |

Avatar colours are kit variables (`avatar/coral`, `amber`, `mint`, `teal`, `violet`, `rose` on
macOS and Windows; `color/tone/*` with `color/on-tone` on Linux).

## Rules the screens follow

- **Policy runs outside the model.** Tool calls appear as cards with their state; risky ones
  stop at an Approval card. Nothing in the UI suggests the model decides on its own.
- **Voice never widens access.** Approval cards for destructive, payment and credential
  actions say "Needs a click. Voice cannot approve this." and have no spoken path.
- **Secrets never reach the model.** Providers and servers show a key or token *handle*
  ("handle anthropic-main"); the key itself is never on screen.
- **Fail open on extensions.** A server or plugin that fails to start shows "Skipped"; voice
  engines list a fallback.
- **Untrusted text.** Model output, tool output and screenshots are content, drawn plainly.

## Code

The desktop build of `aulo-chat` still follows the earlier chat-list design (T12.23): a
conversation sidebar, the chat, and a settings window with server address, appearance and
default model. Bots, tool calls, approvals, voice, quick talk, the screen viewer and the
seven settings tabs are not built; each screen file says what exists.
