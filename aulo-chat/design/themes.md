# Themes

Every screen exists in a dark and a light theme. The screens in the other files are drawn
dark; the frames named "<Platform> / <Screen> · Light" on the page "aulo-chat · Screens" are
the same layers with each kit's colour collection switched to its Light mode.

## Rules

- Colours, radii and text styles in the frames are bound to the kit's variables and styles;
  a frame never holds a hardcoded colour. A light frame differs from its dark frame only in
  the variable mode.
- Settings → Appearance chooses System, Light or Dark (`settings.md`). System follows the OS
  setting and changes live.
- The code reads the same roles (`ChatColors` in `composeApp/.../theme/PlatformTokens.kt`)
  with a dark and a light value per platform, taken from that kit's variables.
- Glass surfaces (Linux popovers, macOS sidebar) keep their blur in both themes.
