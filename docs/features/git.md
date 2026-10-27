# Source Control

For Git projects, the native Source Control panel shows the working tree and
commits it. Git runs on the project's server, so it works for
[remote projects](remote-servers.md) too.

## Source Control panel

`Cmd+Shift+G` (or the Source Control button at the top of the sidebar) toggles
the panel, pinned on the right of the current project. It shares that spot with
the [Files panel](files.md).

- The header shows the current branch, with **Refresh**.
- Changes are grouped as **Conflicts**, **Staged Changes**, **Changes**, and
  **Untracked**. Stage, unstage, or discard a file, or use **Stage All** and
  **Unstage All**.
- Write a message and choose **Commit**, or press `Cmd+Return` in the message
  field. **Amend** amends the last commit instead.
- The panel refreshes when the worktree changes.

## Diffs

Click a changed file to open its diff in a pane beside the focused one. A diff
for a file that already has one reuses its pane, and open diffs refresh when the
working tree changes. Very long diffs are cut short.

## Worktrees and pull requests

The sidebar's worktree cards show each worktree's branch and, when it has one,
its open pull request number. Pull request information needs the
[`gh`](https://cli.github.com) CLI, installed and signed in on the project's
server (`gh auth login`). See [Projects](projects.md#worktrees) for creating and
removing worktrees.
