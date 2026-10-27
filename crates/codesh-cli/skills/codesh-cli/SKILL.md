---
name: codesh-cli
description: How to drive Codesh from a shell with the `codesh` command — start terminals that keep running, type into them, read and wait on their output, and manage projects and Git worktrees. Use this when you are an agent running inside a Codesh terminal (or any script on the same computer) and want work to run in its own terminal instead of blocking yours, or to drive the desktop app's built-in browser.
---

# Codesh CLI

Codesh keeps terminals in a background server. The `codesh` command talks to that
server directly; the desktop app doesn't need to be open. Terminals you start
keep running after your command returns, and the user can open them in the
desktop app or the terminal UI from **Existing Terminals**.

Run `codesh --help` and `codesh <command> --help` for every option. Add `--json`
to any management command for machine-readable output.

## When to use it

- Start a long-running process (a dev server, a test watcher, a log tail) in its
  own terminal, then check on it later.
- Wait until that process prints something, or exits, before going on.
- Create a Git worktree for a task, with the user's configured location.

Don't use it for work a plain command can do in your own terminal.

## Inside a Codesh terminal

Codesh sets `CODESH_SESSION_ID` and `CODESH_SERVER_ID` in every terminal. Session
commands then default to your own terminal, and `session create` to your
project, so most commands need no IDs.

## Start a terminal and capture its ID

`session create` prints the new session ID. Everything after `--` is typed into
the new terminal, followed by Return. Capture the ID; never guess one.

```bash
WEB=$(codesh session create -- npm run dev)
TESTS=$(codesh session create MyProject --directory ./packages/api -- exec npm test)
```

Outside a Codesh terminal, name the project: an ID, a unique name, or its folder.

The words after `--` are joined with spaces, so quote the whole command to keep
its own quotes, pipes, or `;`:

```bash
codesh session create -- 'grep -rn "TODO list" src | wc -l'
```

## Wait instead of polling

```bash
codesh session wait "$WEB" --text "ready" --timeout-ms 120000   # prints the matching line
codesh session wait "$TESTS" --exit                              # prints how it ended
```

`wait` fails with a nonzero exit if the timeout passes (default 30 s) or, for
`--text`, if the terminal ends without showing the text. It looks at the
visible screen, not scrollback. Don't wait on your own terminal: it is busy
running `codesh`.

`--exit` waits for the terminal's shell to end. Use `exec` for a one-shot command,
as in the test example, so the program replaces the shell and its exit ends the
session. Without `exec`, the shell stays open after the program finishes.

## Read output

```bash
codesh session read-screen "$WEB" --lines 40     # last visible rows, not scrollback
codesh session search "$WEB" "error" --ignore-case
codesh session history "$WEB" --limit 200        # JSON, paged with --before
```

After a terminal ends, add `--saved` to read its last saved output.

## Send input deliberately

`send` types text without pressing Return; `send-keys` presses one key
(Enter, Tab, Escape, Backspace, Ctrl+C, Ctrl+D, Ctrl+Z).

```bash
codesh session send-keys "$WEB" Ctrl+C
codesh session read-screen "$WEB"
codesh session send "$WEB" "npm run dev"
codesh session send-keys "$WEB" Enter
```

You are typing into a live terminal. Read the screen first if you are unsure
what is running.

## End terminals you started

```bash
codesh session list --project MyProject     # id, project-id, status, attached, directory
codesh session end "$WEB" --yes
```

Only end terminals you created. A terminal the user is looking at closes in
their app too.

## Projects and worktrees

```bash
codesh project list
codesh project add ~/code/app --reuse                 # prints the existing project if there is one
codesh worktree create MyProject login                # branch "login" from HEAD, default location
codesh worktree create MyProject fix --branch fix/x --base main --hooks
codesh worktree list MyProject
codesh worktree remove login --yes
```

`--hooks` runs the setup or teardown commands from `.codesh/worktree.json`; ask
the user before using it. `worktree remove` refuses uncommitted changes unless
`--force` is given; never pass `--force` without the user's consent.

## Browser

`codesh browser` drives tabs in the desktop app's built-in browser: open a page,
read it, click and type, take screenshots. It talks to the desktop app on this
Mac, so it needs the app (`open` starts it) and never takes `--host`.

```bash
TAB=$(codesh browser open http://localhost:3000)    # prints the new tab's ID
codesh browser wait-for-navigation "$TAB"
codesh browser snapshot "$TAB"                       # interactive elements, each with a selector
codesh browser fill "$TAB" "#email" "me@example.com"
codesh browser click "$TAB" "button[type=submit]"
codesh browser wait "$TAB" --text "Welcome" --timeout-ms 10000
codesh browser read "$TAB"                           # title, URL and page text
```

- Capture the tab ID that `open` prints, or find it with `codesh browser list`;
  never guess one.
- Wait after anything that loads a page (`open`, `navigate`, a submitting
  `click`, `back`): `wait-for-navigation`, `wait-for <selector>`, or `wait`.
- `snapshot` lists the interactive elements, each with a `selector` you can
  pass to `click`, `fill`, `type`, and the other element commands. They print
  `false` when nothing matches.
- `eval` runs JavaScript in the page and prints the result as JSON.
- Screenshots work on background tabs:
  `codesh browser screenshot "$TAB" | base64 -D > page.png`.
- Close only tabs you opened.

## Other computers

Put `--host user@computer` before a server-backed command, such as `project`,
`session`, or `worktree`, to use another computer's server over SSH. Directories
are then absolute paths on that computer.

Folder-opening commands, `browser`, and `install-skills` do not support
`--host`.
`install-skills --dir DIR` always installs into a local directory.
