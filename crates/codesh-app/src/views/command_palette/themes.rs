//! Change Theme: every theme, applied as it is chosen while the palette
//! stays open.

#![expect(
    clippy::unused_self,
    reason = "Page dispatch uses uniform model method signatures"
)]

use std::rc::Rc;

use codesh_ui::command_palette::{Item, PageContent, PageSpec, Tile};
use codesh_ui::fuzzy;
use gpui::{App, Context, Window};

use super::{Handler, Kind, PaletteEntry};
use crate::model::AppModel;
use crate::views::settings::Change;

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
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        let dark = self.dark;
        let active = self.themes.active_name(&self.appearance, dark);
        let mut entries: Vec<_> = self.themes.entries.iter().collect();
        entries.sort_by_cached_key(|entry| (entry.name.to_lowercase(), entry.name.clone()));
        let ranked = if query.is_empty() {
            entries
                .into_iter()
                .map(|item| fuzzy::Ranked {
                    item,
                    score: 0,
                    ranges: Vec::new(),
                })
                .collect()
        } else {
            fuzzy::rank(entries, |entry| &entry.name, query, usize::MAX)
        };
        let items: Vec<_> = ranked
            .into_iter()
            .map(|found| {
                let entry = found.item;
                let name = entry.name.clone();
                let apply: Handler = Rc::new(move |model, _, cx| {
                    model.change_preference(Change::Theme(dark, name.clone()), cx);
                    if let Some(palette) = model.palette_entity() {
                        let active = model.themes.active_name(&model.appearance, model.dark);
                        palette.update(cx, |palette, cx| {
                            palette.set_current("themes", Some(&active), cx);
                        });
                    }
                });
                Item::apply(entry.name.clone(), entry.name.clone(), apply)
                    .kind(Kind::Theme.label())
                    .tile(Tile::swatches(entry.swatches()))
                    .highlights(found.ranges)
                    .current(entry.name == active)
            })
            .collect();
        if items.is_empty() {
            return PageContent::empty("No themes found");
        }
        PageContent::items(items)
    }

    pub(super) fn themes_opened(&mut self, cx: &mut Context<Self>) {
        self.reload_themes(cx);
    }

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
