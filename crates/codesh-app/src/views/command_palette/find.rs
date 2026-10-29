//! Find in Files: ripgrep over the current project, one section per file.

#![expect(clippy::unused_self, reason = "A stub until its page is implemented")]

use codesh_ui::command_palette::{Accessory, PageContent, PageSpec};
use gpui::{App, Context, Task, Window};

use super::{Handler, PaletteEntry};
use crate::model::AppModel;

/// Find in Files' options and its debounced search, kept while the palette
/// is closed so reopening it searches the same way.
#[derive(Default)]
pub(crate) struct FindState {
    pub(crate) case_sensitive: bool,
    pub(crate) regex: bool,
    #[expect(dead_code, reason = "Find in Files' page debounces with it")]
    pub(crate) pending: Option<Task<()>>,
}

impl AppModel {
    /// Opens Find in Files over the current project, or closes it.
    pub(crate) fn toggle_find_in_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(PaletteEntry::Find, window, cx);
    }

    pub(super) fn find_spec(&self, _cx: &App) -> PageSpec {
        let find = &self.command_palette.find;
        PageSpec::new(PaletteEntry::Find.id(), "Find in files…")
            .breadcrumb("Find in Files")
            .accessory(Accessory::Toggle {
                id: "case".into(),
                label: "Aa".into(),
                active: find.case_sensitive,
            })
            .accessory(Accessory::Toggle {
                id: "regex".into(),
                label: ".*".into(),
                active: find.regex,
            })
    }

    pub(super) fn find_items(
        &mut self,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
    }

    pub(super) fn find_opened(&mut self, _cx: &mut Context<Self>) {}

    pub(super) fn find_action(
        &mut self,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    /// Searches for `query` once typing pauses.
    pub(super) fn find_query_changed(&mut self, _query: &str, _cx: &mut Context<Self>) {}

    pub(super) fn find_toggled(&mut self, _id: &str, _active: bool, _cx: &mut Context<Self>) {}

    pub(crate) fn refresh_find_page(&mut self, cx: &mut Context<Self>) {
        if self.palette_page(cx) == Some(PaletteEntry::Find) {
            self.refresh_palette(cx);
        }
    }
}
