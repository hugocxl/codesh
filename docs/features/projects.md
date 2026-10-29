# Projects

A project is a folder on a server. In the desktop app it has one workspace of
split panes, and its terminals belong to it. Projects are shared by every app
on that server; splits, workspaces, and project order are kept by each app.

## Projects

- Add one with **Add Project** at the bottom of the sidebar, **File → Open
  Project…**, or `Cmd+O`, which open the command palette's Open Project page.
  Type to search folders by name, or type a path (`/` or `~/`) to browse: `Tab`
  completes the highlighted folder and lists it, Return opens it, `Cmd+Return`
  adds it as a project, and `Cmd+K` offers Reveal in Finder, Copy Path and the
  parent folder. The page also has **Choose in Finder…** and **Change Search
  Location…**. With remote servers set up, **Add Project on <server>…** in the
  palette browses that server instead.
- `codesh <folder>` in a shell opens a folder in the desktop app.
- The gear next to a project's name opens its menu: rename it, add it to
  workspaces, show its worktrees, or copy its path.
- A project shows its own icon next to its name when it has one: an `.icns`,
  `.ico`, or an `icon`/`logo` PNG at its root or in a folder such as `assets`,
  `resources`, `public`, or `packaging`. Git-ignored folders aren't searched.
- **Remove Project…** removes it from every app and ends its terminals. Files
  stay on disk.
- If a project's folder disappears, it shows as missing and can only be
  removed.

**Home** is the built-in project for your home folder. It is always first and
can't be removed. `Cmd+N` opens a terminal in Home.

## Sidebar

- `Cmd+B` shows or hides the sidebar.
- Projects are listed with their worktrees always expanded beneath them. Each
  worktree, and each project's primary checkout, is an activity card: its name,
  [AI agent](ai-agents.md) state and last activity; the running agent's title,
  latest session, open pull request, or last commit; and its branch.
- Drag projects to reorder them.
- The footer has the host selector (All, Local, or a server), a name filter,
  **Add Project**, and **New Browser**. `Escape` clears the filter.
- The buttons at the top toggle the [Files](files.md) and
  [Source Control](git.md) panels.
- `Ctrl+1`–`Ctrl+9` jumps to a project. Hold a modifier to see the numbers.

## Workspaces

A workspace is a named filter for the sidebar. Create one from the workspace
menu in the sidebar header. Add projects from a project's **Workspaces** menu.
A project can be in any number of workspaces. Deleting a workspace never
removes its projects.

## Worktrees

For Git projects, worktrees appear under their project and have their own
workspace of panes. Click **+** next to a Git project's name to create one, or
hide them with **Show Worktrees** in the project menu.
Worktrees created outside Codesh show up on their own.

**Switch Worktree…** (`Cmd+Opt+W`) lists the current project and its
worktrees: the current one first, then the most recently used, so `Down` and
Return goes back to the previous one. Type to filter, then press Return to
switch. Each worktree can be renamed or removed from its row, and **New
Worktree…** sits at the bottom. Turn off **Settings → Appearance → Order the
worktree switcher by recent use** to list them in sidebar order.

**New Worktree…** (`Cmd+Opt+N`, from the current project or worktree) asks
for:

- A name, and a new branch (from a base branch) or an existing one.
- A location: **Default** (`~/.codesh/worktrees/<project>/<name>`), a
  **Template**, or a **Folder**. Templates must contain `{branch}` and can use
  `{base-dir}` and `{project-name}`, for example `../{base-dir}.{branch}`.
  Set the default in **Settings → General**; each project remembers its choice.

**Remove Worktree and Files…** deletes the worktree folder and its project, and
ends its terminals. Codesh warns about uncommitted changes.

### Setup and teardown commands

Codesh can run commands when it creates or removes a worktree. List them in
`.codesh/worktree.json` in the project, or for every project in
`~/.config/codesh/worktree.json` (`$XDG_CONFIG_HOME/codesh/worktree.json`):

```json
{
  "setup": ["npm ci", { "name": "Start services", "command": "docker compose up -d" }],
  "teardown": ["docker compose down"]
}
```

- Codesh shows the commands and runs them only if you turn them on in the dialog.
- Setup runs your own commands first, then the project's. Teardown runs the
  project's first.
- Commands run in the worktree with `CODESH_PROJECT_PATH`, `CODESH_WORKTREE_ID`,
  `CODESH_WORKTREE_PATH`, `CODESH_WORKTREE_NAME`, and `CODESH_WORKTREE_BRANCH`. They
  share a five-minute limit.
- A failed setup keeps the new worktree. A failed teardown stops the removal.
