//! Find in Files: ripgrep over the current project, one section per file.

use std::{rc::Rc, time::Duration};

use codesh_ui::command_palette::{Accessory, Item, ItemAction, PageContent, PageSpec, Section};
use gpui::{App, Context, Task, Window};

use super::{Handler, PaletteEntry};
use crate::model::AppModel;

/// Find in Files' options and its debounced search, kept while the palette
/// is closed so reopening it searches the same way.
#[derive(Default)]
pub(crate) struct FindState {
    pub(crate) case_sensitive: bool,
    pub(crate) regex: bool,
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
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        if query.is_empty() {
            return PageContent::empty("Type to search");
        }
        if self.command_palette.find.pending.is_some() || self.search_in_flight() {
            return PageContent::loading("Searching…");
        }
        if self.find_results().is_empty() {
            return PageContent::empty("No results");
        }
        let project = self.state.current_project().id;
        let mut results: Vec<_> = self.find_results().iter().collect();
        results.sort_unstable_by(|left, right| {
            (&left.path, left.line, left.column).cmp(&(&right.path, right.line, right.column))
        });
        let mut sections: Vec<Section<Handler>> = Vec::new();
        for result in results {
            let start = usize::try_from(result.column.saturating_sub(1)).unwrap_or(usize::MAX);
            let highlights = match_highlights(
                &result.preview,
                start,
                query,
                self.command_palette.find.regex,
            );
            let path = result.path.clone();
            let line = result.line;
            let column = result.character_column();
            let open: Handler = Rc::new(move |model, _, cx| {
                model.open_project_file(project, &path, Some((line, column)), cx);
            });
            let item = Item::run(
                format!("{}:{}:{}", result.path, result.line, result.column),
                result.preview.clone(),
                open,
            )
            .subtitle(format!("{}:{}", result.line, result.column))
            .highlights(highlights)
            .kind(super::Kind::File.label())
            .tile(super::Kind::File.tile())
            .actions(vec![ItemAction::new("open", "Open File")]);
            if sections
                .last()
                .and_then(|section| section.title.as_ref())
                .is_none_or(|title| title != result.path.as_str())
            {
                sections.push(Section::new(result.path.clone(), Vec::new()));
            }
            if let Some(section) = sections.last_mut() {
                section.items.push(item);
            }
        }
        PageContent::sections(sections)
    }

    pub(super) fn find_opened(&mut self, cx: &mut Context<Self>) {
        let query = self
            .palette_entity()
            .map(|palette| palette.read(cx).query().to_owned())
            .unwrap_or_default();
        self.find_query_changed(&query, cx);
    }

    pub(super) fn find_action(
        &mut self,
        item: &str,
        action: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if action != "open"
            || self.command_palette.find.pending.is_some()
            || self.search_in_flight()
        {
            return;
        }
        let Some(result) = self
            .find_results()
            .iter()
            .find(|result| format!("{}:{}:{}", result.path, result.line, result.column) == item)
        else {
            return;
        };
        let (path, line, column) = (result.path.clone(), result.line, result.character_column());
        let project = self.state.current_project().id;
        self.open_project_file(project, &path, Some((line, column)), cx);
    }

    /// Searches for `query` once typing pauses.
    pub(super) fn find_query_changed(&mut self, query: &str, cx: &mut Context<Self>) {
        self.command_palette.find.pending = None;
        let project = self.state.current_project().id;
        let case = self.command_palette.find.case_sensitive;
        let regex = self.command_palette.find.regex;
        self.search_files(project, "", case, regex, cx);
        if query.is_empty() {
            return;
        }
        let query = query.to_owned();
        self.command_palette.find.pending = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let _ = this.update(cx, |model, cx| {
                model.command_palette.find.pending = None;
                if model.palette_page(cx) != Some(PaletteEntry::Find)
                    || model.state.current_project().id != project
                    || model
                        .palette_entity()
                        .is_none_or(|palette| palette.read(cx).query() != query)
                {
                    return;
                }
                model.search_files(project, &query, case, regex, cx);
                model.refresh_find_page(cx);
            });
        }));
    }

    pub(super) fn find_toggled(&mut self, id: &str, active: bool, cx: &mut Context<Self>) {
        match id {
            "case" => self.command_palette.find.case_sensitive = active,
            "regex" => self.command_palette.find.regex = active,
            _ => return,
        }
        if let Some(palette) = self.palette_entity() {
            let query = palette.read(cx).query().to_owned();
            self.find_query_changed(&query, cx);
        }
    }

    pub(crate) fn refresh_find_page(&mut self, cx: &mut Context<Self>) {
        if self.palette_page(cx) == Some(PaletteEntry::Find) {
            self.refresh_palette(cx);
        }
    }
}

fn match_highlights(
    preview: &str,
    start: usize,
    query: &str,
    regex: bool,
) -> Vec<std::ops::Range<usize>> {
    if start >= preview.len() || !preview.is_char_boundary(start) {
        return Vec::new();
    }
    let length = if regex {
        1
    } else {
        query.chars().count().max(1)
    };
    let end = preview[start..]
        .char_indices()
        .nth(length)
        .map_or(preview.len(), |(offset, _)| start + offset);
    std::iter::once(start..end).collect()
}
