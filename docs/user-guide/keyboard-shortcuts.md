# Keyboard Shortcuts

Defaults for the desktop app. Change any of them in **Settings → Keyboard**.
For the terminal UI, see [Terminal UI keys](../features/terminal-ui.md#keys).

## Panes

Each project has one workspace of split panes.

| Action | Shortcut |
| --- | --- |
| New terminal pane | `Cmd+T` |
| New terminal in Home | `Cmd+N` |
| Close pane | `Cmd+W` |
| Close Tab: all of the project's panes | `Cmd+Shift+W` |
| Split right | `Cmd+D` |
| Split down | `Cmd+Shift+D` |
| Focus pane left / right / up / down | `Cmd+Opt+Arrows` |
| Zoom pane | `Cmd+Shift+Return` |
| Equalize splits | `Cmd+Ctrl+=` |
| Existing terminals | `Cmd+Opt+T` |

## Files and Source Control

| Action | Shortcut |
| --- | --- |
| Toggle Files | `Cmd+E` |
| Toggle Source Control | `Cmd+Y` |
| Go to File | `Cmd+P` |
| Find in Files | `Cmd+Shift+F` |
| Commit (in the message field) | `Cmd+Return` |

## Projects and navigation

| Action | Shortcut |
| --- | --- |
| Add project | `Cmd+O` |
| New terminal in project… | `Cmd+Shift+N` |
| New worktree | `Cmd+Opt+N` |
| Switch worktree | `Cmd+Opt+W` |
| Next / previous project | `Ctrl+]` / `Ctrl+[` |
| Go to project 1–9 | `Ctrl+1`–`Ctrl+9` |
| Back / forward | `Cmd+Ctrl+Left` / `Cmd+Ctrl+Right` |
| Toggle sidebar | `Cmd+B` |
| Agents panel | `Cmd+Opt+A` |
| Command palette | `Cmd+Shift+P` |
| Theme picker | `Cmd+Shift+K` |
| Settings | `Cmd+,` |
| Full screen | `Cmd+Ctrl+F` |

## Terminal

| Action | Shortcut |
| --- | --- |
| Copy / paste | `Cmd+C` / `Cmd+V` |
| Find / next / previous | `Cmd+F` / `Cmd+G` / `Cmd+Shift+G` |
| Previous / next prompt | `Cmd+Up` / `Cmd+Down` |
| Font size bigger / smaller / reset | `Cmd+=` / `Cmd+-` / `Cmd+0` |
| Clear screen | `Cmd+K` |
| Scroll to top / bottom | `Cmd+Home` / `Cmd+End` |
| Delete line | `Cmd+Backspace` |
| Start / end of line | `Cmd+Left` / `Cmd+Right` |
| Line feed (`Ctrl+J`) | `Shift+Return` |

A line feed is a new line in many AI tools, but shells treat it like Return.

## Browser

| Action | Shortcut |
| --- | --- |
| New browser tab | `Cmd+Opt+B` |
| Address bar | `Cmd+L` |
| Reload | `Cmd+R` |
| Back / forward | `Cmd+[` / `Cmd+]` |
| Find on page | `Cmd+F` |
| Zoom in / out / reset | `Cmd+=` / `Cmd+-` / `Cmd+0` |
| Copy address | `Cmd+Shift+C` |
| Web Inspector | `Cmd+Opt+I` |

## Dictation

`Cmd+Shift+I` starts dictation into the active live terminal. See
[Dictation](../features/dictation.md).

## Changing shortcuts

- In **Settings → Keyboard**, click a shortcut and press the new keys.
  **Reset** restores the default. Codesh refuses keys that are already taken.
- Changes are saved to `settings.toml` under `[keymap]`, for example
  `split_right = "cmd-alt-d"`.
- Terminal keys such as clear screen, line editing, and scrolling have built-in
  bindings. Add your own in **Settings → Terminal → Key bindings**; they take
  precedence while a terminal is focused.
- Commands from **Settings → Commands** can have shortcuts too.
