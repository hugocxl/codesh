# Codesh CLI

The `codesh` command talks to the [server](server.md) from a shell. It manages
projects, terminal sessions, worktrees, server settings, and phone access. No
desktop app is needed, so it works on Linux and over SSH too.

Commands work with what the server owns: projects and sessions. Layouts, panes,
and workspaces belong to each app, so there are no commands for them. Browser
panes are the exception: they live in the desktop app, and `codesh browser` talks to
the app on this computer. The [terminal UI](terminal-ui.md) is an app of its
own, with its own layout.

## Install

- **With the desktop app:** choose **Install Command Line Tool…** from the app
  menu. It links `~/.local/bin/codesh` to the copy inside the app.
- **On its own:** see [Getting started](../user-guide/getting-started.md#cli-and-server-only).

Make sure `~/.local/bin` is on your `PATH`.

## Basics

```bash
codesh                 # open the terminal UI
codesh .               # open this folder in the desktop app or the terminal UI
codesh project list    # list projects
codesh --help          # all commands; codesh <command> --help for details
```

- Commands start the server they use if it isn't running, including a remote
  one with `--host`. `codesh server status` and `codesh server stop` never start
  it.
- Management commands take `--json` for machine-readable output, except
  `codesh mobile`.
- Ending or discarding a session and deleting a project or worktree need
  `--yes`.
- A project can be named by its ID, its name if unique, or its folder.
- `--host <destination>` runs the command against the server on another
  computer. It must come first: `codesh --host devbox project list`. See
  [Remote servers](remote-servers.md).

## Commands

| Command | What it does |
| --- | --- |
| `codesh` | Opens the [terminal UI](terminal-ui.md). |
| `codesh <folder>` | Opens the folder as a project in the desktop app. On Linux, over SSH, or without the desktop app, opens it in the terminal UI. |
| `codesh server start\|status\|stop` | Starts, inspects, or stops the server. `stop` refuses while terminals run; add `--force` to end them. |
| `codesh project list\|add\|rename\|delete` | Manages projects. Deleting ends its terminals but keeps files on disk. |
| `codesh session list\|create\|send\|send-keys\|read-screen\|history\|search\|wait\|end\|discard` | Creates terminals, sends input, reads output, and waits for text or exit. |
| `codesh worktree list\|create\|checkout-pr\|register\|remove` | Manages Git worktree projects, including checking out pull requests. |
| `codesh settings get\|set` | Reads or changes [server settings](server.md#settings). |
| `codesh activity list\|ack` | Reads or clears [AI agent](ai-agents.md) activity. |
| `codesh browser open\|list\|read\|click\|…` | Drives browser tabs in the desktop app on this computer. See [Browser control](#browser-control). |
| `codesh exec <project> -- <program> [args]` | Runs a program in the project folder, without a shell. |
| `codesh git`, `codesh files` | Run raw server Git and file actions as JSON. Advanced; the JSON shape follows the protocol and may change. |
| `codesh mobile` | Turns on phone access and pairs phones. See [Mobile](mobile.md). |
| `codesh install-skills` | Installs the Codesh CLI skill for the AI agents found in your home folder. `--dir <path>` adds another skills folder. |
| `codesh stdio` | Joins stdin and stdout to the server. Used by `--host` over SSH; you don't run it yourself. |

## Sessions from a script

```bash
id=$(codesh session create my-app)
codesh session send "$id" "npm test"
codesh session send-keys "$id" Enter
codesh session read-screen "$id" --lines 20
codesh session end "$id" --yes
```

- `send` types text without pressing Return. `send-keys` sends `Enter`, `Tab`,
  `Escape`, `Backspace`, `Ctrl+C`, `Ctrl+D`, or `Ctrl+Z`.
- `read-screen` returns the last visible rows. `history` and `search` page
  through scrollback as JSON.
- `--saved` reads a session's last saved screen, even after it ended.
- `end` stops the program and keeps its saved output. `discard` also deletes
  the output.
- Sessions created here outlive the command and appear in every app under
  **Existing Terminals**.
- Inside a Codesh terminal, session commands default to that terminal, and
  `create` to its project.
- `create` can take a `--directory` and a command to run after `--`.
- `wait --text <text>` waits for text on the screen, and `wait --exit` for the
  shell to exit. Both give up after 30 seconds by default.

## Browser control

```bash
tab=$(codesh browser open localhost:3000)
codesh browser wait-for-navigation "$tab"
codesh browser fill "$tab" "#email" "me@example.com"
codesh browser click "$tab" "button[type=submit]"
codesh browser wait "$tab" --text "Welcome"
codesh browser read "$tab"
codesh browser screenshot "$tab" | base64 -D > page.png
codesh browser close "$tab"
```

- Browser commands need the desktop app and don't take `--host`. `open` starts
  the app if it isn't running.
- `open` prints the new tab's ID. Pass it to the other commands.
- `snapshot` lists a page's interactive elements, each with a selector for
  `click`, `fill`, or `type`.
- `codesh browser --help` lists every command.

## Exit codes

`0` on success. On any error, `codesh` prints `codesh: <message>` to stderr and exits
with `1`, including when a program run with `codesh exec` fails.

## Profiles

`CODESH_DIR` points the CLI, and the server it starts, at another profile folder.
Use it to run a separate, throwaway server. See [Server](server.md#files).
