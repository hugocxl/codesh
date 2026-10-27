# Terminal

Codesh's terminals run in the [server](server.md), powered by
[Ghostty](https://github.com/ghostty-org/ghostty). They keep running after you
quit the app, and every app shows the same screen and history.

## Panes and splits

Each project has one workspace of split panes. Every pane is a card with a
header showing its title and actions; the actions appear on the active or
hovered pane.

| Action | Default shortcut |
| --- | --- |
| New terminal pane | `Cmd+T` |
| New terminal in Home | `Cmd+N` |
| Split right / down | `Cmd+D` / `Cmd+Shift+D` |
| Move focus between panes | `Cmd+Opt+Arrows` |
| Zoom the pane | `Cmd+Shift+Return` |
| Close pane | `Cmd+W` |

- New terminals open to the right of the focused pane, and splits are kept
  equal-sized.
- New panes open in the project folder. Set **Settings → Terminal → New pane
  directory** to open them in the current pane's folder instead.
- Drag the gap between panes to resize, or even them out again with
  **Equalize Splits** (`Cmd+Ctrl+=`, or a pane's right-click menu). `Cmd`-drag
  a pane onto another pane to swap them, or onto an edge to dock it there.

See [Keyboard shortcuts](../user-guide/keyboard-shortcuts.md) for the full
list.

## Closing and detaching

- Closing a pane ends its terminal, unless another pane in any app still shows
  it.
- Codesh asks first when a program other than the shell is running.
- **Detach Terminal** in the pane's right-click menu removes the pane but keeps
  the terminal running.
- **Existing Terminals** (`Cmd+Opt+T`, or the project menu) lists the project's
  terminals that this app isn't showing, with the app that owns each, and opens
  one in a new pane.
- Set **Settings → General → When closing tabs or panes** to **Detach** to make
  every pane close a detach.
- Quitting the app or closing its window never ends a terminal. **End All
  Sessions and Quit** in the app menu ends every terminal on this computer,
  without asking first.

## Find

`Cmd+F` searches the focused terminal, including all of its history. `Return`
or `Cmd+G` goes to the next match, `Cmd+Shift+G` to the previous. Search is
plain text and case-insensitive; **Aa** makes it case-sensitive.

## Copy and paste

- `Cmd+C` copies the selection. `Ctrl+C` goes to the program.
- Turn on **Settings → Terminal → Copy on select** to copy when you select.
- When a program uses the mouse, hold `Shift` to select text or open the
  right-click menu.
- Pasting an image into a local terminal sends `Ctrl+V`, so AI tools can read
  it from the clipboard.

## Links and files

- `Cmd`-click a URL or file path to open it. Paths with `:line:column` jump to
  that line in supported editors.
- **Settings → General → Open files with** picks Codesh's
  [editor](files.md#editor) or another app for files.
- Web links open in your system browser, or in the
  [built-in browser](browser.md) when **Settings → Browser** says so.
- Dropping files on a terminal pastes their paths.
- On a [remote server](remote-servers.md), dropped and pasted files are
  uploaded first and the remote path is pasted. `Cmd`-clicking a remote file
  offers a read-only copy.

## Shell integration

With [shell integration](server.md#shell-integration), Codesh tracks each
terminal's folder, and you can:

- Jump between prompts with `Cmd+Up` and `Cmd+Down`.
- Select a command's output with **Select Command Output** in the Edit menu or
  the right-click menu.

## Appearance and configuration

Fonts, colors, transparency, vibrancy, cursor, padding, selection, scrolling,
and key bindings are in **Settings → Terminal**, saved to `terminal.toml`.
Themes are in **Settings → Appearance**. See
[Settings](../user-guide/settings.md#terminal-configuration).

- `Cmd+=` and `Cmd+-` change the focused pane's font size. `Cmd+0` resets it.
- `Option` acts as `Alt` by default (configurable in Terminal settings).
- Images use the Kitty graphics protocol. Sixel and iTerm2 images are not
  supported.
