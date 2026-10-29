//! Go to File: the current project's files, fuzzy ranked by path.

#![expect(clippy::unused_self, reason = "A stub until its page is implemented")]

use codesh_ui::command_palette::{Item, PageContent, PageSpec};
use gpui::{App, Context, Window};

use super::{Handler, PaletteEntry};
use crate::model::AppModel;

impl AppModel {
    /// Opens Go to File over the current project, or closes it.
    pub(crate) fn toggle_quick_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(PaletteEntry::Files, window, cx);
    }

    pub(super) fn files_spec(&self, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Files.id(), "Go to file…").breadcrumb("Go to File")
    }

    pub(super) fn files_items(
        &mut self,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
    }

    pub(super) fn files_opened(&mut self, _cx: &mut Context<Self>) {}

    pub(super) fn files_action(
        &mut self,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    pub(crate) fn refresh_files_page(&mut self, cx: &mut Context<Self>) {
        if self.palette_page(cx) == Some(PaletteEntry::Files) {
            self.refresh_palette(cx);
        }
    }

    /// Root search rows: the best files for `query`, and whether there are more.
    pub(super) fn files_hits(
        &self,
        _query: &[char],
        _limit: usize,
        _cx: &App,
    ) -> (Vec<Item<Handler>>, bool) {
        (Vec::new(), false)
    }
}
