# Settings

Open Settings with `Cmd+,`. It is laid out like macOS System Settings: pick a
category in the sidebar, or use the search field to find a setting. Changes
apply immediately, and Settings works while the server is offline.

## Sections

| Section | What's in it |
| --- | --- |
| General | Closing behavior, the app that opens files, window size, worktree locations, [dictation](../features/dictation.md) |
| Appearance | Light and dark themes, sidebar, agents panel |
| Keyboard | Every shortcut. See [Keyboard shortcuts](keyboard-shortcuts.md) |
| Commands | Your own commands, each run in a new pane, with optional shortcuts |
| Terminal | Fonts, spacing, transparency, vibrancy, colors, cursor, padding, selection, scrolling, Option keys, key bindings |
| Browser | The [built-in browser](../features/browser.md): terminal links, search engine, home page, profiles, import |
| Server | This computer's [server](../features/server.md#settings): shell, history, shell integration, stop and restart |
| Mobile | [Phone access](../features/mobile.md) and paired phones |
| AI | AI command-line tools and prompts for commit and pull request text |
| Backup & Restore | Export and restore |

## Files

Settings are plain files in Codesh's
[profile folder](../features/server.md#files). **Edit in…** at the top of a
section opens its file.

| File | Holds |
| --- | --- |
| `settings.toml` | App preferences, shortcuts (`[keymap]`), commands, remote servers |
| `terminal.toml` | Terminal preferences and key bindings |
| `themes/` | Your own themes |
| `server.toml` | Server settings |

- Restart the app after editing `settings.toml` by hand. If the file has an
  error or an unknown key, the app won't open; see
  [Troubleshooting](troubleshooting.md#the-app-doesnt-open).
- Hand edits to `terminal.toml` apply when Codesh or Settings becomes active. If
  the file has an error, Settings shows it and keeps the last good values.

## Terminal configuration

Use **Settings → Terminal** for:

- Font family, fallback fonts, bold and italic fonts, size, line and character
  spacing, ligatures and other font features, fonts for Unicode ranges, thicker
  strokes, and bright colors for bold text.
- Background transparency and vibrancy sliders, optionally including colored cells.
  Vibrancy uses the native macOS window material, including its blur and desktop
  tinting. Increasing vibrancy reveals more of that material by reducing the
  terminal's solid color overlay; it also works with transparency set to zero.
  Reduce Transparency and Increase Contrast disable this material.
- Cursor shape, blinking, opacity, and thickness. Thickness accepts an adjustment in
  physical pixels or a percentage: `1` adds one pixel and `100%` doubles it.
  It applies to bar, underline, and outlined cursors. Terminal programs can
  override cursor shape and blinking.
- Separate sliders for left, right, top, and bottom padding, with balanced
  unused space and a padding color.
- Background, text, cursor, selection, and the 16 ANSI colors. Each replaces
  the theme's color; leave it empty to use the theme.
- Copy on select, clearing selection when typing or copying, mouse reporting,
  separate trackpad and mouse wheel speeds, and scrolling to live output.
- Whether both, neither, left, or right Option keys act as Alt.
- The working directory for new panes.
- Key bindings the terminal handles while focused: record a key, then pick an
  action such as sending text. They win over app shortcuts. You can also turn
  off the built-in terminal bindings.

Numeric controls use sliders, on/off options use switches, and fixed choices use
dropdowns. Sliders preview changes while dragging and save when released or when
Settings closes. Spacing and thickness offer a pixels/percent dropdown; changing
units keeps the numeric adjustment within the supported range.

**Reset terminal settings**, at the bottom of the section, returns everything
on it, including key bindings, to its default after you confirm.

Themes remain in **Settings → Appearance**. Shell and history limits remain
in **Settings → Server**.

## Themes

Codesh ships about 490 themes. Pick the app's light and dark themes in
**Settings → Appearance**, or press `Cmd+Shift+K`. Codesh follows the macOS
appearance.

Custom themes in the profile's `themes/` folder show up too. The chosen theme
colors the app and its terminals.

## Backup and restore

- **Export…** saves a `.codesh` file with your settings, `terminal.toml`, themes,
  server settings, remote server list, and local
  projects with their layouts.
- **Choose file…** under **Restore backup** restores on the next launch. Codesh
  keeps a recovery copy in the profile's `Backups/` folder. Existing projects
  are kept and get the saved layouts.
- Backups never include running terminals, paired phones,
  remote projects, or project files.
