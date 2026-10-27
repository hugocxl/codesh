# Privacy

Codesh is a desktop app and command-line tool, developed and maintained by Hugo
Corta. It has no account, no sign-up, and no Codesh-operated service. It
includes no analytics, crash reporting, advertising, or tracking code.

## Data on your computer

Codesh keeps its data in its profile folder:

| Platform | Stable | Beta |
| --- | --- | --- |
| macOS | `~/Library/Application Support/Codesh` | `~/Library/Application Support/Codesh Beta` |
| Linux | `$XDG_STATE_HOME/codesh` (`~/.local/state/codesh`) | `$XDG_STATE_HOME/codesh-beta` |

`CODESH_DIR` overrides the location. The folder holds settings, the project
list, terminal session history and scrollback, logs, browser profiles and
history, remote server definitions, and mobile pairing records. Nothing in it
is uploaded anywhere. See [Server files](docs/features/server.md#files).

Diagnostic logs (`server.log`, and `app-diagnostics-<pid>.log` when you start
the app with `CODESH_DIAGNOSTICS=1`) stay in the profile folder.

## Network connections

Codesh itself connects to the network only for:

- **Update checks.** Released desktop builds check
  `github.com/hugocxl/codesh` releases for an `update.json` feed at launch and
  about once an hour, and download updates from there. **Check for Updates**
  checks immediately. These are ordinary HTTPS requests to GitHub; Codesh sends
  no identifier beyond a `Codesh/<version>` User-Agent.
- **Remote servers you add.** Codesh connects over SSH to the machines you
  configure, using your SSH keys, agent, or a password kept only in memory. When
  you choose to install Codesh on such a machine, that machine downloads the
  installer from `github.com/hugocxl/codesh` releases.
- **The built-in browser.** Browser panes load the pages and search engine you
  choose, along with site icons for those pages. Each browser profile keeps its
  own cookies, site data and history locally.
- **Mobile access, if you turn it on.** `codesh-server` then listens for paired
  devices on your network (port `7419` by default). It is off by default.
- **A proxy, if you set one up.** Traffic you route through it goes to the proxy
  you configure.

## Tools Codesh runs for you

Some features run programs you already have, which make their own network
connections under their own policies:

- Git and the GitHub CLI (`gh`) for fetch, push, pull and pull request actions
  you start.
- AI command-line tools (such as Claude Code or Codex) you pick in
  **Settings → AI**. Codesh gives them the prompt and repository context they
  need for the text they write.
- Programs you run in terminals.

Dictation on macOS uses on-device speech recognition; on Linux it uses a local
model you install. Audio is not sent over the network.

## Questions

Ask on [GitHub Issues](https://github.com/hugocxl/codesh/issues).
