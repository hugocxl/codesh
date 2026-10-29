//! Change Theme: every theme, applied as it is chosen while the palette
//! stays open.

#![expect(clippy::unused_self, reason = "A stub until its page is implemented")]

use codesh_ui::command_palette::{PageContent, PageSpec};
use gpui::{App, Context, Window};

use super::{Handler, PaletteEntry};
use crate::model::AppModel;

impl AppModel {
    /// Opens Change Theme, or closes it when it is open.
    pub(crate) fn open_theme_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(PaletteEntry::Themes, window, cx);
    }

    pub(super) fn themes_spec(&self, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Themes.id(), "Search themes…").breadcrumb("Change Theme")
    }

    pub(super) fn themes_items(
        &mut self,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
    }

    pub(super) fn themes_opened(&mut self, _cx: &mut Context<Self>) {}

    pub(super) fn themes_action(
        &mut self,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    /// Marks the active theme on an open Change Theme page.
    pub(crate) fn refresh_themes_page(&mut self, cx: &mut Context<Self>) {
        if self.palette_page(cx) == Some(PaletteEntry::Themes) {
            self.refresh_palette(cx);
        }
    }
}
