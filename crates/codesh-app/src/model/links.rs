use codesh_app_core::{
    Direction, PaneId, ProjectStatus,
    opener::{FileLocation, OpenContext, OpenRequest, Target},
};
use gpui::Context;

use super::AppModel;
use crate::views::{
    menu::{Command, Item},
    terminal::pane::PaneState,
};

impl AppModel {
    pub(super) fn opener_context(&self, pane: PaneId) -> Option<OpenContext> {
        let project = self.state.projects().iter().find(|project| {
            project
                .tabs
                .iter()
                .any(|tab| tab.panes.iter().any(|p| p.id == pane))
        })?;
        Some(OpenContext {
            project: project.id,
            pane,
            server: project.server_id,
            directory: project.directory.clone(),
            project_directory: project.directory.clone(),
        })
    }

    pub(super) fn open_terminal_link(
        &mut self,
        pane: PaneId,
        target: Target,
        cx: &mut Context<Self>,
    ) {
        let Some(context) = self
            .terminal(&pane)
            .and_then(|pane| pane.view.read(cx).link_context())
        else {
            return;
        };
        if let Target::Url(url) = &target
            && self.browser_enabled()
            && self.settings.browser.open_terminal_links
        {
            self.open_url_in_browser(url.clone(), cx);
            return;
        }
        if let Target::File(file) = &target
            && self.open_file_link(&context, file, cx)
        {
            return;
        }
        if let Target::File(file) = &target
            && !context.server.is_local()
        {
            self.remote_link_menu(pane, context.server, file, cx);
            return;
        }
        let settings = self.settings.openers.clone();
        let result = crate::opener::submit(move || {
            crate::opener::open(&OpenRequest { target, context }, &settings)
        });
        match result {
            Ok(result) => cx
                .spawn(async move |model, cx| {
                    if let Ok(Err(error)) = result.recv().await {
                        let _ = model.update(cx, |model, cx| {
                            model.fail(format!("Could not open link: {error}"), cx);
                        });
                    }
                })
                .detach(),
            Err(error) => self.fail(format!("Could not open link: {error}"), cx),
        }
    }

    /// Opens a file link inside the terminal's project in an editor pane,
    /// at its line and column, when files open in Codesh's editor. Returns
    /// whether it did.
    pub(crate) fn open_file_link(
        &mut self,
        context: &OpenContext,
        file: &FileLocation,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.settings.openers.file.starts_with("system.") {
            return false;
        }
        let Some(relative) = file
            .path
            .strip_prefix(&context.project_directory)
            .ok()
            .and_then(|relative| relative.to_str())
            .filter(|relative| !relative.is_empty())
        else {
            return false;
        };
        let relative = if std::path::MAIN_SEPARATOR == '/' {
            relative.to_owned()
        } else {
            relative.replace(std::path::MAIN_SEPARATOR, "/")
        };
        let position = file.line.map(|line| (line, file.column.unwrap_or(1)));
        self.open_project_file(context.project, &relative, position, cx);
        true
    }

    pub(super) fn terminal_menu(
        &self,
        id: PaneId,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(pane) = self.terminal(&id) else {
            return;
        };
        let project = self.state.current_project();
        let Some(tab) = project
            .tabs
            .iter()
            .find(|tab| tab.panes.iter().any(|pane| pane.id == id))
        else {
            return;
        };
        let pane = pane.view.read(cx);
        let mut copy = Item::action("Copy", Command::TerminalCopy(id));
        if pane.selection.is_none() {
            copy = copy.disabled();
        }
        let mut paste = Item::action("Paste", Command::TerminalPaste(id));
        if pane.state != PaneState::Live {
            paste = paste.disabled();
        }
        let mut all = Item::action("Select All", Command::TerminalSelectAll(id));
        if pane.displayed_grid().is_none() {
            all = all.disabled();
        }
        let mut output = Item::action(
            "Select Command Output",
            Command::TerminalSelectCommandOutput(id),
        );
        if pane.state != PaneState::Live {
            output = output.disabled();
        }
        let mut detach = Item::action("Detach Terminal", Command::DetachTerminal(id));
        if !self.can_detach_terminal(id) {
            detach = detach.disabled();
        }
        let mut split_right =
            Item::action("Split Right", Command::SplitPane(id, Direction::Right)).separated();
        let mut split_down = Item::action("Split Down", Command::SplitPane(id, Direction::Down));
        let mut zoom = Item::action(
            if tab.zoomed == Some(id) {
                "Restore Pane"
            } else {
                "Maximize Pane"
            },
            Command::ToggleZoomPane(id),
        );
        let mut equalize = Item::action("Equalize Splits", Command::EqualizeSplits(id));
        let mut close = Item::action("Close Pane", Command::ClosePane(id));
        if project.status() == ProjectStatus::Missing {
            split_right = split_right.disabled();
            split_down = split_down.disabled();
            equalize = equalize.disabled();
            close = close.disabled();
        }
        if tab.panes.len() < 2 && tab.zoomed.is_none() {
            zoom = zoom.disabled();
        }
        if tab.panes.len() < 2 {
            equalize = equalize.disabled();
        }
        if tab.pinned && tab.panes.len() == 1 {
            close = close.disabled();
        }
        let items = vec![
            copy,
            paste,
            all,
            output,
            split_right,
            split_down,
            zoom,
            equalize,
            self.proxy_menu_item(id).separated(),
            detach,
            close,
        ];
        let model = cx.entity().downgrade();
        let window = self.window;
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = model.update(cx, |model, cx| {
                    if model.grids.contains_key(&id) {
                        model.open_menu(items, position, window, cx);
                    }
                });
            });
        });
    }
}
