//! Go to File: the current project's files, fuzzy ranked by path.

#![expect(
    clippy::unused_self,
    reason = "Every page function takes the model, used or not"
)]

use std::rc::Rc;

use codesh_app_core::ProjectId;
use codesh_ui::{
    command_palette::{Item, ItemAction, PageContent, PageSpec},
    fuzzy,
};
use gpui::{App, Context, Window};

use super::{Handler, Kind, PaletteEntry};
use crate::model::AppModel;

const FILE_LIMIT: usize = 100;

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
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        let project = self.state.current_project().id;
        let Some(list) = self.file_list(project) else {
            return PageContent::empty("No matching files");
        };
        if list.loading() && list.files().is_empty() {
            return PageContent::loading("Listing files…");
        }
        let items = file_items(project, list.files(), query, FILE_LIMIT);
        if items.is_empty() {
            return PageContent::empty("No matching files");
        }
        PageContent::items(items)
    }

    pub(super) fn files_opened(&mut self, cx: &mut Context<Self>) {
        self.load_file_list(self.state.current_project().id, cx);
    }

    pub(super) fn files_action(
        &mut self,
        item: &str,
        action: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if action != "open" {
            return;
        }
        let project = self.state.current_project().id;
        if !self
            .file_list(project)
            .is_some_and(|list| list.files().iter().any(|path| path == item))
        {
            return;
        }
        self.open_project_file(project, item, None, cx);
    }

    pub(crate) fn refresh_files_page(&mut self, cx: &mut Context<Self>) {
        if self.palette_page(cx) == Some(PaletteEntry::Files) {
            self.refresh_palette(cx);
        }
    }

    /// Root search rows: the best files for `query`, and whether there are more.
    pub(super) fn files_hits(
        &self,
        query: &[char],
        limit: usize,
        _cx: &App,
    ) -> (Vec<Item<Handler>>, bool) {
        if query.len() < 2 {
            return (Vec::new(), false);
        }
        let project = self.state.current_project().id;
        let Some(list) = self.file_list(project) else {
            return (Vec::new(), false);
        };
        let mut items = file_items(
            project,
            list.files(),
            &query.iter().collect::<String>(),
            limit.saturating_add(1).min(FILE_LIMIT),
        );
        let more = items.len() > limit;
        items.truncate(limit);
        (items, more)
    }
}

fn file_items(
    project: ProjectId,
    files: &[String],
    query: &str,
    limit: usize,
) -> Vec<Item<Handler>> {
    fuzzy::rank(files.iter(), |path| path.as_str(), query, limit)
        .into_iter()
        .map(|found| {
            let path = found.item;
            let base = path.rfind('/').map_or(0, |index| index + 1);
            let highlights = found
                .ranges
                .into_iter()
                .filter(|range| range.start >= base)
                .map(|range| range.start - base..range.end - base)
                .collect();
            let open_path = path.clone();
            let open: Handler =
                Rc::new(move |model, _, cx| model.open_project_file(project, &open_path, None, cx));
            Item::run(path.clone(), path[base..].to_owned(), open)
                .subtitle(path[..base].to_owned())
                .highlights(highlights)
                .kind(Kind::File.label())
                .tile(Kind::File.tile())
                .actions(vec![ItemAction::new("open", "Open File")])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_rows_shift_unicode_basename_highlights() {
        let files = ["src/Éclair.rs", "src/other.rs"].map(str::to_owned);
        let project = ProjectId::new();
        let items = file_items(project, &files, "src/éc", 100);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "src/Éclair.rs");
        assert_eq!(items[0].title, "Éclair.rs");
        assert_eq!(items[0].subtitle.as_ref().expect("directory"), "src/");
        assert_eq!(items[0].highlights, [0..2, 2..3]);
        assert!(matches!(
            items[0].target,
            codesh_ui::command_palette::Target::Run(_)
        ));
    }
}
