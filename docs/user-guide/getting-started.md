# Getting Started

Install: [macOS](#macos-desktop-app) · [CLI and server only](#cli-and-server-only) ·
[Linux and Intel Macs](#linux-and-intel-macs)

After installing: [First steps](#first-steps)

## Requirements

| Part | Runs on |
| --- | --- |
| Desktop app | macOS 14 or newer on Apple Silicon |
| `codesh` CLI and server | macOS 14 or newer on Apple Silicon |

Releases currently ship for Apple Silicon Macs only. Intel Macs and Linux
(glibc 2.35 or newer, x86_64 or ARM64) can still
[build from source](#linux-and-intel-macs).

## macOS desktop app

1. From the [latest release](https://github.com/hugocxl/codesh/releases/latest),
   download `Codesh-<version>-arm64.dmg`.
2. Drag **Codesh** to `/Applications` and open it.

To use [`codesh`](../features/codesh-cli.md) in your shell, choose **Install Command
Line Tool…** from the app menu. It links `~/.local/bin/codesh`.

The app updates itself from GitHub Releases. Running terminals keep going
through most updates.

## CLI and server only

For a Mac without the desktop app. This installs `codesh` and `codesh-server`.

### Install script

Set `VERSION` to the newest version on the
[releases page](https://github.com/hugocxl/codesh/releases/latest), without the
leading `v`:

```bash
VERSION=2.1.0
curl -fsSL "https://github.com/hugocxl/codesh/releases/download/v$VERSION/install-codesh.sh" \
  | sh -s -- --version "$VERSION"
export PATH="$HOME/.local/bin:$PATH"
```

The script verifies the download's checksum and installs both commands into
`~/.local/bin`. On a platform without a published build it stops before
downloading anything. Add the `export` line to your shell's startup file to keep
them on `PATH`.

- **Update:** run it again with the new version and `--replace`.
- **Another folder:** add `--install-dir PATH`.

### Check it

Run `codesh --version` and `codesh-server --version`, then `codesh` to open the
[terminal UI](../features/terminal-ui.md). Updating them never restarts a
running [server](../features/server.md#lifetime); stop it to switch to the new
version.

## Linux and Intel Macs

Builds for Linux and Intel Macs are not currently published. Build the CLI and
server from source with `cargo build --release -p codesh-cli -p codesh-server`
(see [CONTRIBUTING](../../CONTRIBUTING.md) for the toolchain). The experimental
Linux desktop app builds with `scripts/build-desktop-linux.sh`; see its
[platform limits](../tech/constraints.md#experimental-linux-desktop).

## First steps

1. **Add a project.** Choose **Add Project** at the bottom of the sidebar, or
   press `Cmd+O`, and pick a folder.
2. **Open a terminal.** `Cmd+T` adds a terminal pane to the project. `Cmd+N`
   opens one in Home.
3. **Split it.** `Cmd+D` splits right, `Cmd+Shift+D` splits down. Splits stay
   equal-sized.
4. **Open files.** `Cmd+E` shows Files, `Cmd+Y` Source Control, and
   `Cmd+P` Go to File.
5. **Quit freely.** Terminals keep running in the background. Reopen Codesh and
   everything is where you left it.

## Next

- [How Codesh works](../features/server.md): the server, sessions, and history.
- [Projects](../features/projects.md), [Terminal](../features/terminal.md),
  [Files and editor](../features/files.md), and
  [Keyboard shortcuts](keyboard-shortcuts.md).
- [Remote servers](../features/remote-servers.md) and
  [Mobile access](../features/mobile.md).
- [Settings](settings.md) and [Troubleshooting](troubleshooting.md).
