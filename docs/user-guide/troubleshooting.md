# Troubleshooting

Common problems and fixes. If yours isn't here, please
[open an issue](https://github.com/hugocxl/codesh/issues).

## Logs

| What | Where |
| --- | --- |
| Server | `server.log` in the [profile folder](../features/server.md#files) |
| Desktop app | Start it with `CODESH_DIAGNOSTICS=1` to write `app-diagnostics-<pid>.log` to the profile folder |
| CLI and terminal UI | Printed in the terminal |

`codesh --build-info` and `codesh server status` show the versions in use.

## The app doesn't open

Usually `settings.toml` has an error or an unknown key. Start the app from a
terminal to see why:

```bash
"/Applications/Codesh.app/Contents/MacOS/codesh-app"
```

Fix the line it names, or move `settings.toml` out of the profile folder to
start with defaults.

## The server won't start

If `codesh` reports `request timed out`, run the server in the foreground to see
the error, often a typo in `server.toml`:

```bash
"/Applications/Codesh.app/Contents/MacOS/codesh-server"   # or codesh-server, if installed on its own
```

## `codesh` is not found, or is the wrong version

The desktop app and the install script put `codesh` in `~/.local/bin`. Add it to
your `PATH`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

If `codesh --version` shows an unexpected version, another `codesh` comes first
on your `PATH`; check with `which -a codesh`.

## A remote server won't connect

- **Host key not trusted:** run `ssh <host>` once and accept the key.
- **Login refused:** check that `ssh <host>` works with your key or agent. Only
  the desktop app supports passwords.
- **Codesh isn't installed:** choose **Install Codesh…** in the server's menu,
  or run the installer there. See [Remote servers](../features/remote-servers.md).
- **Incompatible version:** install the same version on both computers. If the
  version is right but the old server is still running, run
  `pkill -x codesh-server` there. This ends its terminals.
- **Terminals end when SSH disconnects (Linux):** run `loginctl enable-linger`.

## A phone can't connect

- Check **Settings → Mobile → Status** says it is listening.
- On macOS, allow `codesh-server` if the firewall asks.
- An iOS client needs local network permission.
- The phone must reach the computer: same network, a VPN, or an address added
  with `codesh mobile pair --address`.

## No agent notifications

- Allow notifications in **System Settings → Notifications → Codesh**.
- Codesh doesn't notify for the pane you're looking at, or for detached
  terminals.
- Recognition is best effort. `codesh activity list` shows what the server sees.

## Source Control or search doesn't work

- Source Control shows **Not a Git repository** outside a Git work tree.
- Pull request information needs `gh` installed and signed in on the project's
  server: `gh auth login`.
- Find in Files needs [ripgrep](https://github.com/BurntSushi/ripgrep) (`rg`)
  on the project's server. Go to File falls back to `git ls-files` without it.

## Keys don't work as expected

- `Option` acts as `Alt` in terminals. Choose **Neither** under
  **Settings → Terminal → Option as Alt** to type special characters.
- Key bindings in **Settings → Terminal → Key bindings** win over app
  shortcuts while a terminal is focused.

## Start fresh

1. Stop the server, which ends every terminal: `codesh server stop --force`.
2. Quit Codesh.
3. Move the [profile folder](../features/server.md#files) somewhere else.

The next launch starts with default settings and no projects.

## Reporting a bug

Include your macOS or Linux version, the output of `codesh --build-info`, steps
to reproduce, and the relevant lines of `server.log`.
