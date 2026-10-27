# Getting Started

Install: [macOS](#macos-desktop-app) · [Linux](#linux-desktop-app) ·
[CLI and server only](#cli-and-server-only)

After installing: [First steps](#first-steps) · [Beta builds](#beta)

## Requirements

| Part | Runs on |
| --- | --- |
| Desktop app | macOS 14 or newer, on Apple Silicon or Intel; experimental Linux packages target Ubuntu 24.04 on x86_64 and ARM64 |
| `codesh` CLI and server | macOS 14 or newer, or Linux with glibc 2.35 or newer, on x86_64 or ARM64. musl is unsupported. |

## macOS desktop app

1. From the [latest release](https://github.com/hugocxl/codesh/releases/latest),
   download `Codesh-<version>-arm64.dmg` for Apple Silicon or
   `Codesh-<version>-x86_64.dmg` for Intel.
2. Drag **Codesh** to `/Applications` and open it.

To use [`codesh`](../features/codesh-cli.md) in your shell, choose **Install Command
Line Tool…** from the app menu. It links `~/.local/bin/codesh`.

The app updates itself from GitHub Releases. Running terminals keep going
through most updates.

## Linux desktop app

Linux support is **experimental**, targeting Ubuntu 24.04 on x86_64 and ARM64,
with glibc 2.39 or newer and a working Vulkan driver. See the
[platform limits and WebKit sandbox setup](../tech/constraints.md#experimental-linux-desktop).

Choose the [quick installer](#quick-installer-with-in-app-updates) for in-app
updates, a [Debian package](#debian-package), or an [AppImage](#appimage).
All three include the desktop app, CLI and server. Use one format per channel
to avoid conflicting CLI commands.

### Quick installer with in-app updates

Install the Ubuntu dependencies once:

```sh
sudo apt update
sudo apt install curl python3 ca-certificates libfreetype6 libfontconfig1 libxcb1 \
  libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libvulkan1 mesa-vulkan-drivers \
  libwebkit2gtk-4.1-0 xdg-utils xdg-desktop-portal bubblewrap xdg-dbus-proxy
```

Install the **latest stable release** as your normal user, without `sudo`.
The installer detects your architecture and verifies the download:

```sh
curl -fsSL --proto '=https' --proto-redir '=https' \
  https://github.com/hugocxl/codesh/releases/latest/download/install-desktop-linux.py \
  | python3 - install
```

Open **Codesh** from Applications. The app, CLI and server install into
`~/.local/share/codesh-desktop` by default. Add `~/.local/bin` to your shell's
`PATH` to use `codesh <folder>` from a terminal.

### Debian package

Download `codesh-desktop_<version>_amd64.deb` or
`codesh-desktop_<version>_arm64.deb` from the
[latest stable release](https://github.com/hugocxl/codesh/releases/latest).
Use `dpkg --print-architecture` to check which you need. In your download
directory, replace the filename below with your download:

```sh
sudo apt install './codesh-desktop_<version>_amd64.deb'
```

APT installs the dependencies and adds **Codesh** to Applications, with `codesh`
and `codesh-server` in `/usr/bin`.

### AppImage

Install the [runtime libraries above](#quick-installer-with-in-app-updates), then
download `Codesh-<version>-linux-x86_64.AppImage` or
`Codesh-<version>-linux-arm64.AppImage` from the
[latest stable release](https://github.com/hugocxl/codesh/releases/latest).
Use `uname -m` to check: `aarch64` means ARM64. Replace the filename below with
your download and run it without `sudo`:

```sh
chmod +x './Codesh-<version>-linux-x86_64.AppImage'
'./Codesh-<version>-linux-x86_64.AppImage'
```

The first launch adds **Codesh** to Applications. AppImage uses **system WebKitGTK**.
If FUSE is unavailable, append `--appimage-extract-and-run` when launching it.

Codesh keeps an unpacked copy in
`${XDG_DATA_HOME:-$HOME/.local/share}/codesh-appimage/<bundle-id>` so terminals can
keep running after the window closes. This takes extra disk space. Use
`./Codesh-<version>-linux-x86_64.AppImage --cli <arguments>` for the bundled CLI,
or choose **Install Command Line Tool…** in the app to link it into `~/.local/bin`.

### Updates

- **Quick installer:** the app checks automatically; **Check for Updates** checks
  now. Stable installs stay on stable; beta installs stay on beta.
- **Debian package:** stop the server and quit Codesh, then download the newer `.deb`
  and install it with `sudo apt install`. There is no Codesh APT repository, so
  `apt upgrade` alone does not update Codesh.
- **AppImage:** quit the app, then download and run the newer file. Running
  terminals stay alive. If you linked the CLI, choose **Install Command Line Tool…**
  again from the new app.

Debian packages and AppImages do not use the in-app updater.

### Remove

Stop the local server in **Settings → Server**, then quit Codesh. Stopping the
server ends its running terminals. Removal keeps your settings, projects and
saved session data.

- **Quick installer:** run the command below. For beta, replace `codesh-desktop`
  with `codesh-desktop-beta`.

  ```sh
  python3 "${XDG_DATA_HOME:-$HOME/.local/share}/codesh-desktop/current/install.py" uninstall
  ```

- **Debian package:** run `sudo apt remove codesh-desktop` (or `codesh-desktop-beta`).
- **AppImage:** delete the downloaded file, its matching unpacked copy listed
  above, and any CLI link or Applications entry pointing into that copy. The entry
  is in `${XDG_DATA_HOME:-$HOME/.local/share}/applications/com.codesh.app.desktop`
  (`com.codesh-beta.app.desktop` for beta). Do not delete a copy still used by
  another running app or server.

## CLI and server only

For a Linux computer or Mac without the desktop app. This installs `codesh` and
`codesh-server`.

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

The script picks the build for your OS and architecture, verifies its checksum,
and installs both commands into `~/.local/bin`. Add the `export` line to your
shell's startup file to keep them on `PATH`.

- **Update:** run it again with the new version and `--replace`.
- **Another folder:** add `--install-dir PATH`.

### Check it

Run `codesh --version` and `codesh-server --version`, then `codesh` to open the
[terminal UI](../features/terminal-ui.md). Updating them never restarts a
running [server](../features/server.md#lifetime); stop it to switch to the new
version.

## Beta

The beta gets new features first. **Codesh Beta** installs alongside stable,
with separate settings and session data.

- **macOS:** download the newest `v<version>-beta.<N>` pre-release from the
  [releases page](https://github.com/hugocxl/codesh/releases). The app updates to
  each new beta.
- **Linux:** append `--channel beta` to the [quick-installer command](#quick-installer-with-in-app-updates)
  (`python3 - install --channel beta`), or download a beta `.deb` or AppImage
  from the releases page. Use the same [update method](#updates) as stable.
  The quick installer and Debian package provide `codesh-beta` and `codesh-server-beta`.
- **CLI and server only:** give the [install script](#install-script) a beta
  version, such as `2.1.0-beta.1143`.

## First steps

1. **Add a project.** Choose **Add Project** at the bottom of the sidebar, or
   press `Cmd+O`, and pick a folder.
2. **Open a terminal.** `Cmd+T` adds a terminal pane to the project. `Cmd+N`
   opens one in Home.
3. **Split it.** `Cmd+D` splits right, `Cmd+Shift+D` splits down. Splits stay
   equal-sized.
4. **Open files.** `Cmd+E` shows Files, `Cmd+Shift+G` Source Control, and
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
