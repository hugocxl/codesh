# Files and editor

Files, Go to File, Find in Files, and the editor are built into the desktop
app. They work the same for local and [remote](remote-servers.md) projects:
file operations run on the project's server.

## Files panel

`Cmd+E` (or the Files button at the top of the sidebar) toggles the Files panel,
pinned on the right of the current project. It shares that spot with
[Source Control](git.md); opening one closes the other.

- Click a file to open it in an editor pane.
- The panel header has **New File**, **New Folder**, **Collapse All**, and
  **Refresh**.
- Right-click an entry to open, rename, or delete it, copy its absolute or
  relative path, or reveal it in Finder (local projects).
- The tree follows changes on disk.

## Editor

Files open in an editor pane, split to the right of the focused pane. An editor
for a file that is already open reuses its pane.

- Syntax highlighting covers common languages.
- The pane title is marked while there are unsaved changes. Closing a pane or
  quitting asks whether to save them.
- If the file changes on disk, an editor without unsaved changes reloads it; one
  with unsaved changes offers to take the disk's version or keep yours.
- Binary and very large files are not opened for editing.
- **Settings → General → Open files with** chooses whether files clicked in a
  terminal open in this editor or in another app. Paths with `:line:column` open
  at that position.

## Go to File

`Cmd+P` opens the command palette on its Go to File page: type to filter,
then press Return to open one. Typing two or more characters in the palette's
root search shows the best file matches too, with **See all** for the full
list. The list comes from `rg --files` on the project's server, or from
`git ls-files` when [ripgrep](https://github.com/BurntSushi/ripgrep) is not
installed there. Hidden files are included and `.git` is skipped.

## Find in Files

`Cmd+Shift+F` opens the palette's Find in Files page, which searches the
project's files with ripgrep (`rg`), installed on the project's server. The
**Aa** and **.\*** toggles beside the search field switch case-sensitive and
regular-expression matching. Results are grouped by file, up to 200 matches per
file; choose one to open the file at that line.
