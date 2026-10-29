//! The ⌘K panel: the selected item's actions, filtered as the user types.

use std::ops::Range;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Corner, Entity, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, ScrollHandle, SharedString, StatefulInteractiveElement,
    Styled, Subscription, anchored, deferred, div, point, px,
};

use super::model::{ActionId, Item, ItemAction, ItemId, PageId};
use super::{Palette, keycaps, layout};
use crate::fuzzy;
use crate::text_input::{self, InputEvent, TextInput};
use crate::theme::{Elevation, Metrics, Theme};

/// One row of the panel. `action` is `None` for the item's primary target,
/// which Enter on the row runs like Enter on the item.
#[derive(Clone, Debug)]
struct Entry {
    action: Option<ActionId>,
    title: SharedString,
    keycaps: Vec<SharedString>,
    destructive: bool,
    keep_open: bool,
}

/// What Enter in the panel resolved to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Confirmation {
    Nothing,
    /// A destructive action now waits for a second Enter.
    Armed,
    Run {
        action: Option<ActionId>,
        keep_open: bool,
    },
}

pub(super) struct ActionsPanel {
    pub(super) page: PageId,
    pub(super) item: ItemId,
    pub(super) input: Entity<TextInput>,
    entries: Vec<Entry>,
    visible: Vec<(usize, Vec<Range<usize>>)>,
    highlighted: usize,
    armed: Option<usize>,
    scroll: ScrollHandle,
    _subscription: Subscription,
}

impl std::fmt::Debug for ActionsPanel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionsPanel")
            .field("page", &self.page)
            .field("item", &self.item)
            .field("entries", &self.entries)
            .field("highlighted", &self.highlighted)
            .field("armed", &self.armed)
            .finish_non_exhaustive()
    }
}

impl ActionsPanel {
    pub(super) fn new<A: Clone + 'static>(
        page: PageId,
        item: &Item<A>,
        theme: &Theme,
        metrics: Metrics,
        cx: &mut Context<Palette<A>>,
    ) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(layout::filter_style(theme, metrics), cx)
                .with_key_context(text_input::BARE_CONTEXT)
                .with_placeholder("Search for actions…")
        });
        let subscription = cx.subscribe(&input, |palette: &mut Palette<A>, input, event, cx| {
            if !matches!(event, InputEvent::Changed) {
                return;
            }
            let query = input.read(cx).text().to_owned();
            if let Some(panel) = &mut palette.actions {
                panel.filter(&query);
                cx.notify();
            }
        });
        let mut panel = Self {
            page,
            item: item.id.clone(),
            input,
            entries: Vec::new(),
            visible: Vec::new(),
            highlighted: 0,
            armed: None,
            scroll: ScrollHandle::new(),
            _subscription: subscription,
        };
        panel.rebuild(item, "");
        panel
    }

    /// Rebuilds the entries from `item`, keeping the filter.
    pub(super) fn refresh<A>(&mut self, item: &Item<A>, cx: &gpui::App) {
        let query = self.input.read(cx).text().to_owned();
        self.rebuild(item, &query);
    }

    fn rebuild<A>(&mut self, item: &Item<A>, query: &str) {
        let has_payload = item.target.payload().is_some();
        let primary = item.actions.first();
        let mut entries = vec![Entry {
            action: None,
            title: item.primary_title(),
            keycaps: keycaps::parse("enter"),
            destructive: primary.is_some_and(|action| action.destructive),
            keep_open: primary.is_some_and(|action| action.keep_open),
        }];
        if has_payload {
            let repeats_primary =
                |action: &&ItemAction| primary.is_some_and(|primary| primary.id == action.id);
            entries.extend(
                item.actions
                    .iter()
                    .skip(1)
                    .filter(|action| !repeats_primary(action))
                    .map(|action| Entry {
                        action: Some(action.id.clone()),
                        title: action.title.clone(),
                        keycaps: action
                            .shortcut
                            .as_ref()
                            .map(|chord| keycaps::parse(chord))
                            .unwrap_or_default(),
                        destructive: action.destructive,
                        keep_open: action.keep_open,
                    }),
            );
        }
        self.entries = entries;
        self.filter(query);
    }

    fn filter(&mut self, query: &str) {
        let query = fuzzy::normalize(query);
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                fuzzy::fuzzy(&entry.title, &query).map(|found| (index, found.ranges))
            })
            .collect();
        self.highlighted = 0;
        self.armed = None;
        self.scroll.scroll_to_item(0);
    }

    pub(super) fn move_highlight(&mut self, delta: isize) {
        let count = self.visible.len();
        if count == 0 {
            return;
        }
        let step = delta.unsigned_abs() % count;
        self.highlighted = if delta >= 0 {
            (self.highlighted + step) % count
        } else {
            (self.highlighted + count - step) % count
        };
        self.armed = None;
        self.scroll.scroll_to_item(self.highlighted);
    }

    pub(super) fn highlight(&mut self, row: usize) {
        if row < self.visible.len() && row != self.highlighted {
            self.highlighted = row;
            self.armed = None;
        }
    }

    pub(super) fn confirm(&mut self) -> Confirmation {
        let Some((index, _)) = self.visible.get(self.highlighted) else {
            return Confirmation::Nothing;
        };
        let Some(entry) = self.entries.get(*index) else {
            return Confirmation::Nothing;
        };
        if entry.destructive && self.armed != Some(*index) {
            self.armed = Some(*index);
            return Confirmation::Armed;
        }
        self.armed = None;
        Confirmation::Run {
            action: entry.action.clone(),
            keep_open: entry.keep_open,
        }
    }

    fn render_list<A: Clone + 'static>(
        &self,
        theme: &Theme,
        metrics: Metrics,
        cx: &mut Context<Palette<A>>,
    ) -> gpui::Stateful<gpui::Div> {
        let row_height = metrics.scaled(layout::ACTIONS_ROW);
        let mut list = div()
            .id("palette-actions-list")
            .flex()
            .flex_col()
            .flex_grow()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .p(metrics.scaled(layout::LIST_INSET_Y));
        if self.visible.is_empty() {
            list = list.child(
                div()
                    .h(row_height)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(metrics.font_body())
                    .text_color(theme.fg_muted)
                    .child("No matching actions"),
            );
        }
        for (row, (index, ranges)) in self.visible.iter().enumerate() {
            let Some(entry) = self.entries.get(*index) else {
                continue;
            };
            let highlighted = row == self.highlighted;
            let color = if entry.destructive {
                theme.danger
            } else {
                theme.fg
            };
            let selector = entry
                .action
                .clone()
                .unwrap_or_else(|| SharedString::from("primary"));
            list = list.child(
                div()
                    .id(("palette-action", row))
                    .debug_selector(move || format!("palette-action-{selector}"))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(metrics.spacing4())
                    .h(row_height)
                    .px(metrics.spacing4())
                    .rounded(metrics.radius_md())
                    .cursor_pointer()
                    .text_size(metrics.font_body())
                    .text_color(color)
                    .when(highlighted, |row| row.bg(theme.fg_alpha(0.08)))
                    .when(!highlighted, |row| row.hover(|style| style.bg(theme.hover)))
                    .on_click(cx.listener(move |palette, _, window, cx| {
                        if let Some(panel) = &mut palette.actions {
                            panel.highlight(row);
                        }
                        palette.confirm_actions(window, cx);
                    }))
                    .child(
                        div()
                            .min_w(px(0.0))
                            .flex_1()
                            .truncate()
                            .child(fuzzy::highlighted(
                                entry.title.clone(),
                                ranges,
                                theme.accent,
                            )),
                    )
                    .child(keycaps::keycaps(&entry.keycaps, theme, metrics)),
            );
        }
        list
    }

    fn armed_title(&self) -> Option<&SharedString> {
        Some(&self.entries.get(self.armed?)?.title)
    }

    /// The panel, anchored by its bottom-right corner at `corner`, a point
    /// relative to the palette's top-left corner.
    pub(super) fn render<A: Clone + 'static>(
        &self,
        corner: gpui::Point<Pixels>,
        theme: &Theme,
        metrics: Metrics,
        cx: &mut Context<Palette<A>>,
    ) -> AnyElement {
        let message = self
            .armed_title()
            .map(|title| SharedString::from(format!("Press ↩ again to {}", title.to_lowercase())));
        let height = layout::actions_height(self.visible.len(), message.is_some(), metrics);
        let row_height = metrics.scaled(layout::ACTIONS_ROW);
        let list = self.render_list(theme, metrics, cx);
        let panel = div()
            .id("palette-actions")
            .debug_selector(|| "palette-actions".into())
            .occlude()
            .flex()
            .flex_col()
            .w(metrics.scaled(layout::ACTIONS_WIDTH))
            .h(height)
            .rounded(metrics.radius_xl())
            .border_1()
            .border_color(theme.border)
            .bg(theme.bg.blend(theme.fg_alpha(0.06)))
            .shadow(Elevation::Modal.shadow(theme.bg))
            .overflow_hidden()
            .text_color(theme.fg)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(
                cx.listener(|palette, event: &gpui::MouseDownEvent, window, cx| {
                    palette.actions_dismissed_at = Some(event.position);
                    palette.close_actions(window, cx);
                }),
            )
            .child(list)
            .when_some(message, |panel, message| {
                panel.child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .h(row_height)
                        .px(metrics.spacing6())
                        .text_size(metrics.font_footnote())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.danger)
                        .child(message),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(row_height + metrics.scaled(layout::LIST_INSET_Y))
                    .px(metrics.spacing6())
                    .border_t_1()
                    .border_color(theme.border)
                    .child(div().flex_grow().min_w(px(0.0)).child(self.input.clone())),
            );
        div()
            .absolute()
            .top_0()
            .left_0()
            .child(deferred(
                anchored()
                    .position_mode(gpui::AnchoredPositionMode::Local)
                    .anchor(Corner::BottomRight)
                    .position(point(corner.x, corner.y))
                    .snap_to_window_with_margin(metrics.scaled(layout::VIEWPORT_MARGIN))
                    .child(panel),
            ))
            .into_any_element()
    }
}
