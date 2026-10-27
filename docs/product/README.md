# Codesh

Codesh is a terminal workspace for running agents and shells. You organize your
work into projects and worktrees, each with one workspace of split panes:
terminals, editors, diffs, and browsers. Terminals run in a background server,
so they keep running when you close the app, and several apps can show the
same terminal at once.

```mermaid
flowchart LR
    subgraph COMPUTER["Your computer"]
        SERVER["codesh-server<br/>projects · terminals · history"]
        DESKTOP["Desktop app"] <--> SERVER
        TUI["codesh terminal UI"] <--> SERVER
    end
    PHONE["Phone client"] <-->|"paired · local network or VPN"| SERVER
    DESKTOP <-->|"SSH"| REMOTE["codesh-server<br/>on another computer"]
```

Three ideas explain most of Codesh:

- **The server owns the work.** Projects and terminal sessions live in the
  server, not in the apps.
- **Each app owns its view.** Split layouts, workspaces, and project order are
  private to each app.
- **Terminals outlive apps.** Quitting or crashing an app never ends a terminal.
  Closing the last pane that shows it does.

## Read next

1. [Product model](./product-model.md): projects, workspaces, worktrees, panes,
   and sessions.
2. [App model](./app-model.md): the apps, navigation, settings, and updates.
3. [Server model](./server-model.md): how terminals live, are shared, and end.

How it is built: [technical design](../tech/README.md).
