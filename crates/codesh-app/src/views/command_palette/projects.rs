//! Switch Project, Switch Workspace, and New Terminal in Project: every
//! top-level project, this computer's then other computers', where a new
//! terminal tab can open.

#![expect(clippy::unused_self, reason = "A stub until its page is implemented")]

use codesh_ui::command_palette::{Item, PageContent, PageSpec};
use gpui::{App, Context, Window};

use super::{Handler, PaletteEntry};
use crate::model::AppModel;

impl AppModel {
    /// Opens New Terminal in Project, or closes it when it is open.
    pub(crate) fn toggle_new_tab_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(PaletteEntry::NewTerminal, window, cx);
    }

    pub(super) fn projects_spec(&self, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Projects.id(), "Search projects…").breadcrumb("Switch Project")
    }

    pub(super) fn projects_items(
        &mut self,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
    }

    pub(super) fn projects_opened(&mut self, _cx: &mut Context<Self>) {}

    pub(super) fn projects_action(
        &mut self,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    /// Root search rows: the best projects, and whether there are more.
    pub(super) fn projects_hits(
        &self,
        _query: &[char],
        _limit: usize,
        _cx: &App,
    ) -> (Vec<Item<Handler>>, bool) {
        (Vec::new(), false)
    }

    pub(super) fn workspaces_spec(&self, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Workspaces.id(), "Search workspaces…")
            .breadcrumb("Switch Workspace")
    }

    pub(super) fn workspaces_items(
        &mut self,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
    }

    pub(super) fn workspaces_opened(&mut self, _cx: &mut Context<Self>) {}

    pub(super) fn workspaces_action(
        &mut self,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    pub(super) fn new_terminal_spec(&self, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::NewTerminal.id(), "Search projects…")
            .breadcrumb("New Terminal in Project")
    }

    pub(super) fn new_terminal_items(
        &mut self,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
    }

    pub(super) fn new_terminal_opened(&mut self, _cx: &mut Context<Self>) {}

    pub(super) fn new_terminal_action(
        &mut self,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    /// Shows new projects or workspaces on an open project page.
    pub(crate) fn refresh_projects_page(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.palette_page(cx),
            Some(PaletteEntry::Projects | PaletteEntry::Workspaces | PaletteEntry::NewTerminal)
        ) {
            self.refresh_palette(cx);
        }
    }
}
