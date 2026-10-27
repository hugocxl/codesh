# Browser

The desktop app has a built-in browser. A browser is a pane like a terminal: it
opens as a split in the project's workspace, and its page keeps running while
hidden.

## Open a page

- **File → New Browser Tab** (`Cmd+Opt+B` on macOS, `Ctrl+Alt+B` on Linux),
  **New Browser** at the bottom of the sidebar, or the command palette.
- `codesh browser open <url>` from a terminal. See [Codesh CLI](codesh-cli.md#browser-control).
- Command-click a link in a terminal, when **Settings → Browser → Open terminal
  links in the browser** is on.
- A page that opens a new window gets a new browser pane instead.

Type an address or a search in the address bar. Matching history shows below
it. Links to other apps, such as `mailto:`, open in those apps.

## In a page

| Action | macOS | Linux |
| --- | --- | --- |
| Address bar | `Cmd+L` | `Ctrl+L` |
| Reload | `Cmd+R` | `Ctrl+R` |
| Back / forward | `Cmd+[` / `Cmd+]` | `Alt+Left` / `Alt+Right` |
| Find on page | `Cmd+F` | `Ctrl+Shift+F` |
| Zoom in / out / reset | `Cmd+=` / `Cmd+-` / `Cmd+0` | `Ctrl+=` / `Ctrl+-` / `Ctrl+0` |
| Copy the page's address | `Cmd+Shift+C` | `Ctrl+Shift+C` |
| Web Inspector | `Cmd+Opt+I` | `Ctrl+Shift+I` |

The toolbar also opens the page in another browser and switches the pane's
profile.

## Profiles

Each profile keeps its own cookies, site data, history, and logins. New panes use
the default profile. Manage profiles in **Settings → Browser**, where you can
also import history, bookmarks, and cookies from another browser on this
computer.

## Saved and not saved

A browser pane remembers its address and profile across restarts. Its title,
zoom, and back/forward history start fresh.

## Turning it off

Turn off **Settings → Browser → Enable built-in browser** to disable browser
panes and `codesh browser`. Terminal links then open in your system browser.

## Linux

The browser uses WebKitGTK 4.1, version 2.42 or newer. It shares the Linux
browser renderer's [input and clipboard limits](../tech/constraints.md#experimental-linux-desktop).
Navigation, profiles, history, find, Web Inspector and CLI automation work through
this renderer, including screenshots of hidden tabs. The app still starts without
WebKitGTK installed; opening a browser reports the missing engine.
Right-click a page and choose **Web Inspector** to open developer tools, or use
`Ctrl+Shift+I`. Pages with their own context menu retain it.

Cookie import from Firefox is available on Linux. Importing encrypted Chromium
cookies requires macOS Keychain and is unavailable on Linux. Website file pickers,
JavaScript dialogs, downloads, camera, microphone and location requests are not
supported by the Linux host.
