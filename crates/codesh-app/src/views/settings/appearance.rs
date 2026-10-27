use super::Snapshot;
use super::{Category, Change, PickerKind, SettingsEvent, SettingsView};
use codesh_app_core::settings::{AgentsPanelGrouping, CloseBehavior, SidebarCollapsedStyle};
use codesh_ui::controls::{self, Choice};
use codesh_ui::l10n::{tr_key, translate};
use codesh_ui::tr;
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
    if category == Category::General
        && pane.matches(category, tr_key!("When closing tabs or panes"))
    {
        let selected = match pane.snapshot.settings.window.close_behavior {
            CloseBehavior::CloseSession => "close",
            CloseBehavior::Detach => "detach",
        };
        rows.push(pane.row(
            "close-behavior",
            tr_key!("When closing tabs or panes"),
            controls::segmented(
                pane.style(),
                "close-behavior",
                &[
                    Choice::new("close", tr!("Close sessions")),
                    Choice::new("detach", tr!("Detach")),
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
        (false, tr_key!("Light theme"), &appearance.light_theme),
        (true, tr_key!("Dark theme"), &appearance.dark_theme),
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
            tr_key!("Expand sidebar"),
            appearance.sidebar_expanded,
            Change::Sidebar(!appearance.sidebar_expanded),
        ),
        (
            "status-bar",
            tr_key!("Show status bar"),
            appearance.status_bar_visible,
            Change::StatusBar(!appearance.status_bar_visible),
        ),
        (
            "confirm-process",
            tr_key!("Confirm before closing a running process"),
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
    if category == Category::Appearance
        && pane.matches(category, tr_key!("Collapsed sidebar style"))
    {
        rows.push(collapsed_sidebar_style(pane, cx));
    }
    if category == Category::Appearance && pane.matches(category, tr_key!("Sidebar vibrancy")) {
        rows.push(pane.row(
            "sidebar-vibrancy",
            tr_key!("Sidebar vibrancy"),
            pane.toggle(
                "sidebar-vibrancy",
                appearance.sidebar_vibrancy,
                Change::SidebarVibrancy(!appearance.sidebar_vibrancy),
                cx,
            ),
        ));
    }
    if category == Category::Appearance && pane.matches(category, tr_key!("Sidebar vibrancy level"))
    {
        rows.push(pane.row(
            "sidebar-vibrancy-level",
            tr_key!("Sidebar vibrancy level"),
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
    if category == Category::Appearance && pane.matches(category, tr_key!("Show tips")) {
        rows.push(pane.row(
            "tips",
            tr_key!("Show tips"),
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
            tr_key!("Expand worktrees when switching projects"),
            appearance.auto_expand_worktrees,
        ),
        (
            "worktree-unread",
            tr_key!("Show unread worktree indicators"),
            appearance.worktree_show_unread,
        ),
        (
            "worktree-recent-order",
            tr_key!("Order the worktree switcher by recent use"),
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
    if category == Category::Appearance && pane.matches(category, tr_key!("Show agents panel")) {
        rows.push(pane.row(
            "agents-panel",
            tr_key!("Show agents panel"),
            pane.toggle(
                "agents-panel",
                appearance.agents_panel_visible,
                Change::AgentsPanel(!appearance.agents_panel_visible),
                cx,
            ),
        ));
    }
    if category == Category::Appearance && pane.matches(category, tr_key!("Agents panel rows")) {
        rows.push(agents_panel_grouping(pane, cx));
    }
    if category == Category::General && pane.matches(category, tr_key!("Open files with")) {
        rows.push(pane.row(
            "file-opener",
            tr_key!("Open files with"),
            pane.picker(PickerKind::FileOpener, &file_opener(&pane.snapshot), cx),
        ));
    }
    for (id, label) in [
        ("width", tr_key!("Default window width")),
        ("height", tr_key!("Default window height")),
        (
            "worktree-template",
            tr_key!("Default worktree path template"),
        ),
        ("worktree-folder", tr_key!("Default worktree folder")),
    ] {
        if category == Category::General && pane.matches(category, label) {
            rows.push(pane.row(id, label, pane.field(id)));
        }
    }
    rows
}

/// Openers for files clicked in a terminal, as `(setting value, label key)`.
pub(super) const FILE_OPENERS: [(&str, &str); 4] = [
    (
        codesh_app_core::settings::CODESH_EDITOR,
        tr_key!("Codesh editor"),
    ),
    ("system.editor", tr_key!("Project editor")),
    ("system.finder", crate::opener::FILE_MANAGER),
    ("system.application", tr_key!("Default application")),
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
        .map(|&(_, label)| translate(label).to_string())
        .unwrap_or_default()
}

fn agents_panel_grouping(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let appearance = &pane.snapshot.settings.appearance;
    pane.row(
        "agents-panel-grouping",
        tr_key!("Agents panel rows"),
        controls::segmented(
            pane.style(),
            "agents-panel-grouping",
            &[
                Choice::new("agents", tr!("Agents")),
                Choice::new("worktrees", tr!("Worktrees")),
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
        tr_key!("Collapsed sidebar style"),
        controls::segmented(
            pane.style(),
            "sidebar-collapsed-style",
            &[
                Choice::new("icons", tr!("Icons")),
                Choice::new("hidden", tr!("Hidden")),
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
