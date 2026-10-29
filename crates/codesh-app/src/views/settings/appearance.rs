use super::Snapshot;
use super::{Category, Change, PickerKind, SettingsEvent, SettingsView};
use codesh_app_core::settings::{AgentsPanelGrouping, CloseBehavior, SidebarCollapsedStyle};
use codesh_ui::controls::{self, Choice};
use gpui::{AnyElement, Context};

#[allow(
    clippy::too_many_lines,
    reason = "Declarative appearance settings rows"
)]
pub(super) fn rows(
    pane: &SettingsView,
    category: Category,
    cx: &mut Context<SettingsView>,
) -> Vec<AnyElement> {
    let mut rows = Vec::new();
    if category == Category::General && pane.matches(category, "When closing tabs or panes") {
        let selected = match pane.snapshot.settings.window.close_behavior {
            CloseBehavior::CloseSession => "close",
            CloseBehavior::Detach => "detach",
        };
        rows.push(pane.row(
            "close-behavior",
            "When closing tabs or panes",
            controls::segmented(
                pane.style(),
                "close-behavior",
                &[
                    Choice::new("close", "Close sessions"),
                    Choice::new("detach", "Detach"),
                ],
                selected,
                cx.listener(|_, selected: &gpui::SharedString, _, cx| {
                    cx.emit(SettingsEvent::Change(Change::CloseBehavior(
                        if selected.as_ref() == "detach" {
                            CloseBehavior::Detach
                        } else {
                            CloseBehavior::CloseSession
                        },
                    )));
                }),
            ),
        ));
    }

    let appearance = &pane.snapshot.settings.appearance;
    for (dark, label, value) in [
        (false, "Light theme", &appearance.light_theme),
        (true, "Dark theme", &appearance.dark_theme),
    ] {
        if category == Category::Appearance && pane.matches(category, label) {
            rows.push(pane.row(
                if dark { "dark-theme" } else { "light-theme" },
                label,
                pane.picker(PickerKind::Theme(dark), value, cx),
            ));
        }
    }
    for (id, label, value, change) in [
        (
            "sidebar",
            "Expand sidebar",
            appearance.sidebar_expanded,
            Change::Sidebar(!appearance.sidebar_expanded),
        ),
        (
            "status-bar",
            "Show status bar",
            appearance.status_bar_visible,
            Change::StatusBar(!appearance.status_bar_visible),
        ),
        (
            "confirm-process",
            "Confirm before closing a running process",
            pane.snapshot.settings.window.confirm_running_process,
            Change::ConfirmProcess(!pane.snapshot.settings.window.confirm_running_process),
        ),
    ] {
        let target = if id == "confirm-process" {
            Category::General
        } else {
            Category::Appearance
        };
        if category == target && pane.matches(category, label) {
            rows.push(pane.row(id, label, pane.toggle(id, value, change, cx)));
        }
    }
    if category == Category::Appearance && pane.matches(category, "Collapsed sidebar style") {
        rows.push(collapsed_sidebar_style(pane, cx));
    }
    if category == Category::Appearance && pane.matches(category, "Sidebar vibrancy") {
        rows.push(pane.row(
            "sidebar-vibrancy",
            "Sidebar vibrancy",
            pane.toggle(
                "sidebar-vibrancy",
                appearance.sidebar_vibrancy,
                Change::SidebarVibrancy(!appearance.sidebar_vibrancy),
                cx,
            ),
        ));
    }
    if category == Category::Appearance && pane.matches(category, "Sidebar vibrancy level") {
        rows.push(pane.row(
            "sidebar-vibrancy-level",
            "Sidebar vibrancy level",
            super::terminal::slider(
                pane,
                "sidebar-vibrancy-level",
                f32::from(appearance.sidebar_vibrancy_level.min(100)),
                (0.0, 100.0),
                1.0,
                "",
                cx,
            ),
        ));
    }
    if category == Category::Appearance && pane.matches(category, "Show tips") {
        rows.push(pane.row(
            "tips",
            "Show tips",
            pane.toggle(
                "tips",
                appearance.tips_visible,
                Change::Tips(!appearance.tips_visible),
                cx,
            ),
        ));
    }
    for (id, label, value) in [
        (
            "auto-expand-worktrees",
            "Expand worktrees when switching projects",
            appearance.auto_expand_worktrees,
        ),
        (
            "worktree-unread",
            "Show unread worktree indicators",
            appearance.worktree_show_unread,
        ),
        (
            "worktree-recent-order",
            "Order the worktree switcher by recent use",
            appearance.worktree_recent_order,
        ),
    ] {
        if category == Category::Appearance && pane.matches(category, label) {
            rows.push(pane.row(
                id,
                label,
                pane.toggle(id, value, Change::Worktrees(id, !value), cx),
            ));
        }
    }
    if category == Category::Appearance && pane.matches(category, "Show agents panel") {
        rows.push(pane.row(
            "agents-panel",
            "Show agents panel",
            pane.toggle(
                "agents-panel",
                appearance.agents_panel_visible,
                Change::AgentsPanel(!appearance.agents_panel_visible),
                cx,
            ),
        ));
    }
    if category == Category::Appearance && pane.matches(category, "Agents panel rows") {
        rows.push(agents_panel_grouping(pane, cx));
    }
    if category == Category::General && pane.matches(category, "Open files with") {
        rows.push(pane.row(
            "file-opener",
            "Open files with",
            pane.picker(PickerKind::FileOpener, &file_opener(&pane.snapshot), cx),
        ));
    }
    for (id, label) in [
        ("width", "Default window width"),
        ("height", "Default window height"),
        ("worktree-template", "Default worktree path template"),
        ("worktree-folder", "Default worktree folder"),
    ] {
        if category == Category::General && pane.matches(category, label) {
            rows.push(pane.row(id, label, pane.field(id)));
        }
    }
    rows
}

/// Openers for files clicked in a terminal, as `(setting value, label key)`.
pub(super) const FILE_OPENERS: [(&str, &str); 4] = [
    (codesh_app_core::settings::CODESH_EDITOR, "Codesh editor"),
    ("system.editor", "Project editor"),
    ("system.finder", crate::opener::FILE_MANAGER),
    ("system.application", "Default application"),
];

pub(super) fn chosen_file_opener(value: &str) -> &str {
    if FILE_OPENERS.iter().any(|(id, _)| *id == value) {
        value
    } else {
        codesh_app_core::settings::CODESH_EDITOR
    }
}

fn file_opener(snapshot: &Snapshot) -> String {
    let value = chosen_file_opener(&snapshot.settings.openers.file);
    FILE_OPENERS
        .iter()
        .find(|(id, _)| *id == value)
        .map(|&(_, label)| label.to_string())
        .unwrap_or_default()
}

fn agents_panel_grouping(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let appearance = &pane.snapshot.settings.appearance;
    pane.row(
        "agents-panel-grouping",
        "Agents panel rows",
        controls::segmented(
            pane.style(),
            "agents-panel-grouping",
            &[
                Choice::new("agents", "Agents"),
                Choice::new("worktrees", "Worktrees"),
            ],
            match appearance.agents_panel_grouping {
                AgentsPanelGrouping::Agents => "agents",
                AgentsPanelGrouping::Worktrees => "worktrees",
            },
            cx.listener(|_, selected: &gpui::SharedString, _, cx| {
                cx.emit(SettingsEvent::Change(Change::AgentsPanelGrouping(
                    if selected.as_ref() == "worktrees" {
                        AgentsPanelGrouping::Worktrees
                    } else {
                        AgentsPanelGrouping::Agents
                    },
                )));
            }),
        ),
    )
}

fn collapsed_sidebar_style(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let appearance = &pane.snapshot.settings.appearance;
    pane.row(
        "sidebar-collapsed-style",
        "Collapsed sidebar style",
        controls::segmented(
            pane.style(),
            "sidebar-collapsed-style",
            &[
                Choice::new("icons", "Icons"),
                Choice::new("hidden", "Hidden"),
            ],
            match appearance.sidebar_collapsed_style {
                SidebarCollapsedStyle::Icons => "icons",
                SidebarCollapsedStyle::Hidden => "hidden",
            },
            cx.listener(|_, selected: &gpui::SharedString, _, cx| {
                cx.emit(SettingsEvent::Change(Change::SidebarCollapsedStyle(
                    if selected.as_ref() == "hidden" {
                        SidebarCollapsedStyle::Hidden
                    } else {
                        SidebarCollapsedStyle::Icons
                    },
                )));
            }),
        ),
    )
}
