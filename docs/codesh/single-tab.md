# Single tab per project

In the desktop app, each non-Home project, including a worktree project, has zero or
one tab, and that tab holds the project's split layout. There is no tab strip.
`codesh_app_core::codesh::SINGLE_TAB` enables this policy; the terminal UI keeps
its own multi-tab layouts.

## Opening content

New Terminal (`Cmd+T`) always creates a standalone terminal tab in local Home.
Home tabs remain independent and appear as ungrouped cards above projects.
Each tab can contain splits. Home is exempt from the single-tab policy, including
normalization and restore.

New terminal requests within a project, adjacent-terminal requests, editor and diff panes,
browser opens, and the Remote Servers view all open panes in the project's
container. Editor and diff panes for the same file, and Remote Servers, reuse
their existing pane.

New content splits to the right of the focused pane, falling back to the last
layout leaf, and splits are equalized so panes stay equal-sized. Background
browser opens keep window selection and focus. All typed pane insertion goes
through `AppState::insert_split_pane`.

## Normalization

Loading saved state, restoring a backup, and applying a project layout merge
multiple tabs in order: the first tab survives and each later layout subtree is
appended on the right intact. Pane IDs, content, terminal sessions, and startup
commands are kept. Merging clears groups and zoom, repairs selected tab IDs,
and keeps a valid pane focused. Normalization runs on lifecycle boundaries,
never during rendering.

## Closing

Closing the last pane leaves the project empty. The next terminal opened in that project creates a
fresh container; closing never silently starts another terminal session.
Closing a standalone terminal removes its tab and returns to the most recently
focused surviving project or terminal. Home remains even when empty.
Custom commands and agent resumes title their new pane.
