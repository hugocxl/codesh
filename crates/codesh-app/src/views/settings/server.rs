use super::{Category, Change, SettingsEvent, SettingsView};
use codesh_ui::controls;
use gpui::{AnyElement, Context, IntoElement};

pub(super) fn rows(pane: &SettingsView, cx: &mut Context<SettingsView>) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    let connected = pane.snapshot.connection == crate::model::ConnectionState::Ready;
    if let Some(description) = &pane.snapshot.server_update
        && pane.matches(Category::Server, "Server update version")
    {
        rows.push(pane.note(description, false).into_any_element());
    }
    if pane.matches(Category::Server, "Current device") {
        rows.push(
            pane.row(
                "server",
                "Current device",
                controls::button(
                    pane.style(),
                    "server-read",
                    if connected {
                        "Reload settings"
                    } else {
                        "Connect"
                    },
                    !pane.snapshot.server_busy,
                    cx.listener(move |_, _, _, cx| {
                        cx.emit(if connected {
                            SettingsEvent::ReadServer
                        } else {
                            SettingsEvent::Connect
                        });
                    }),
                )
                .into_any_element(),
            ),
        );
    }
    if !connected {
        if pane.matches(Category::Server, "Server disconnected") {
            let note =
                "Server disconnected. App preferences still work. Connect to edit server settings.";
            rows.push(pane.note(note, false).into_any_element());
        }
        return rows;
    }
    if let Some(server) = &pane.snapshot.server {
        for (id, label) in [
            ("default-shell", "Default shell (executable path)"),
            ("history-budget", "History budget per session (MiB)"),
        ] {
            if pane.matches(Category::Server, label) {
                rows.push(pane.row(id, label, pane.field(id)));
            }
        }
        if pane.matches(Category::Server, "Shell integration") {
            rows.push(pane.row(
                "shell-integration",
                "Shell integration",
                pane.toggle(
                    "shell-integration",
                    server.shell_integration,
                    Change::ShellIntegration(!server.shell_integration),
                    cx,
                ),
            ));
        }
    } else if pane.matches(Category::Server, "Loading settings") {
        let note = "Load the current device's settings to edit them.";
        rows.push(pane.note(note, false).into_any_element());
    }
    for (restart, label) in [(false, "Stop Server"), (true, "Restart Server")] {
        if pane.matches(Category::Server, label) {
            rows.push(
                pane.row(
                    label,
                    label,
                    controls::button(
                        pane.style(),
                        label,
                        label,
                        !pane.snapshot.server_busy,
                        cx.listener(move |_, _, _, cx| {
                            cx.emit(SettingsEvent::ServerControl { restart });
                        }),
                    )
                    .into_any_element(),
                ),
            );
        }
    }
    rows
}
