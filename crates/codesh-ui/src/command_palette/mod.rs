//! A Raycast-style command palette: a stack of searchable pages whose rows the
//! host supplies.
//!
//! The palette holds no app data. For every page it emits
//! [`PaletteEvent::QueryChanged`] and the host answers with
//! [`Palette::set_content`]. It never closes itself: it emits
//! [`PaletteEvent::Dismissed`] and leaves the rest to the host.

mod actions_panel;
pub mod keycaps;
mod layout;
mod legacy;
mod model;
mod row;
#[cfg(test)]
mod tests;
mod tile;

use codesh_core::shortcuts::ShortcutId;
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, KeyContext, ListAlignment, ListState, MouseButton,
    ParentElement, Pixels, Point, Render, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Window, actions, div, px,
};

pub use legacy::{Command, CommandPalette, CommandPaletteEvent, Registry};
pub use model::{
    Accessory, ActionId, Item, ItemAction, ItemId, PageContent, PageId, PageSpec, Section, Status,
    Target,
};
pub use tile::{Tile, TileGlyph, TileHue};

use actions_panel::{ActionsPanel, Confirmation};
use model::{ContentChange, PageState, Row};

use crate::components::IconGlyph;
use crate::icon::Icon;
use crate::picker::{
    Confirm, Dismiss, NavigateBack, SecondaryConfirm, SelectNext, SelectPrevious, TabPressed,
};
use crate::scrollbar::ListScrollbar;
use crate::text_input::{self, InputEvent, TextInput};
use crate::theme::{Elevation, Metrics, Theme};

/// The `Picker` part makes every `popover.*` shortcut apply; `CommandPalette`
/// scopes the palette's own shortcuts.
const KEY_CONTEXT: &str = "Picker CommandPalette";
const LOADING: &str = "Loading…";

actions!(command_palette, [ToggleActions]);

pub fn register_shortcuts(registry: &mut crate::shortcuts::Registry<'_>) {
    registry.register(ShortcutId::CommandPaletteActions, &ToggleActions);
}

pub fn key_bindings() -> Vec<gpui::KeyBinding> {
    let mut registry = crate::shortcuts::Registry::new(&codesh_core::shortcuts::Defaults);
    register_shortcuts(&mut registry);
    registry.into_bindings()
}

/// Sizes are logical points, scaled by the palette's [`Metrics`].
#[derive(Clone, Debug, PartialEq)]
#[must_use]
pub struct PaletteConfig {
    pub id: SharedString,
    pub root: PageSpec,
    /// Drawn at the footer's left edge, such as the app icon.
    pub footer_glyph: Option<TileGlyph>,
    pub width: f32,
    pub max_height: f32,
    pub top: f32,
}

impl PaletteConfig {
    pub fn new(id: impl Into<SharedString>, root: PageSpec) -> Self {
        Self {
            id: id.into(),
            root,
            footer_glyph: None,
            width: layout::WIDTH,
            max_height: layout::MAX_HEIGHT,
            top: layout::TOP,
        }
    }

    pub fn footer_glyph(mut self, glyph: TileGlyph) -> Self {
        self.footer_glyph = Some(glyph);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaletteEvent<A> {
    /// The query of `page` changed, or the page was just pushed with an empty query.
    QueryChanged {
        page: PageId,
        query: SharedString,
    },
    /// Enter on an item whose target is [`Target::OpenPage`]; the host answers
    /// with [`Palette::push_page`].
    OpenPage {
        page: PageId,
        parent: ItemId,
    },
    /// `page` left the stack.
    PagePopped {
        page: PageId,
    },
    /// The user moved the selection, or new content moved it.
    SelectionChanged {
        page: PageId,
        item: Option<ItemId>,
    },
    /// The host closes the palette, then runs `payload`.
    Run {
        page: PageId,
        item: ItemId,
        payload: A,
    },
    /// The host runs `payload`; the palette stays open.
    Apply {
        page: PageId,
        item: ItemId,
        payload: A,
    },
    /// A secondary action from ⌘K or ⌘↩. Items whose target is a page carry
    /// no payload, so their secondary actions are not offered.
    RunAction {
        page: PageId,
        item: ItemId,
        action: ActionId,
        payload: A,
    },
    AccessoryToggled {
        page: PageId,
        id: SharedString,
        active: bool,
    },
    /// Tab on a page whose spec `completes_on_tab`.
    CompletionRequested {
        page: PageId,
        query: SharedString,
        item: Option<ItemId>,
    },
    Dismissed,
}

struct Page<A> {
    state: PageState<A>,
    list: ListState,
    scrollbar: ListScrollbar,
}

impl<A> Page<A> {
    fn new(spec: PageSpec, content: PageContent<A>, metrics: Metrics) -> Self {
        let state = PageState::new(spec, content);
        let list = ListState::new(
            state.rows.len(),
            ListAlignment::Top,
            metrics.scaled(layout::ROW * 4.0),
        );
        Self {
            state,
            list,
            scrollbar: ListScrollbar::default(),
        }
    }

    fn reveal_selection(&self) {
        if let Some(row) = self.state.selected_row() {
            self.list.scroll_to_reveal_item(row);
        }
    }

    fn relayout(&self) {
        self.list.reset(self.state.rows.len());
        self.scrollbar.reset();
        self.reveal_selection();
    }
}

/// The palette entity. `A` is the payload a host runs for an item.
pub struct Palette<A> {
    config: PaletteConfig,
    theme: Theme,
    metrics: Metrics,
    input: Entity<TextInput>,
    root: Page<A>,
    stack: Vec<Page<A>>,
    actions: Option<ActionsPanel>,
    /// Where the click that closed the ⌘K panel landed, so the footer button it
    /// also hits does not reopen the panel.
    actions_dismissed_at: Option<Point<Pixels>>,
    focused: bool,
    _subscription: Subscription,
}

impl<A> std::fmt::Debug for Palette<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Palette")
            .field("config", &self.config)
            .field("root", &self.root.state)
            .field(
                "stack",
                &self
                    .stack
                    .iter()
                    .map(|page| &page.state)
                    .collect::<Vec<_>>(),
            )
            .field("actions", &self.actions)
            .finish_non_exhaustive()
    }
}

impl<A: Clone + 'static> EventEmitter<PaletteEvent<A>> for Palette<A> {}

impl<A: Clone + 'static> Focusable for Palette<A> {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl<A: Clone + 'static> Palette<A> {
    /// Opens on the root page. Its empty [`PaletteEvent::QueryChanged`] is
    /// emitted once the current update ends, so a host that subscribes right
    /// after creating the palette receives it.
    pub fn new(
        config: PaletteConfig,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(layout::search_style(&theme, metrics), cx)
                .with_key_context(text_input::BARE_CONTEXT)
                .with_placeholder(config.root.placeholder.clone())
        });
        let subscription = cx.subscribe(&input, |palette: &mut Self, input, event, cx| {
            if !matches!(event, InputEvent::Changed) {
                return;
            }
            let query = input.read(cx).text().to_owned();
            let page = palette.current_mut();
            if page.state.query == query {
                return;
            }
            page.state.query.clone_from(&query);
            let page = page.state.id().clone();
            cx.emit(PaletteEvent::QueryChanged {
                page,
                query: query.into(),
            });
        });
        let root = Page::new(config.root.clone(), PageContent::loading(LOADING), metrics);
        let page = root.state.id().clone();
        let palette = cx.weak_entity();
        cx.defer(move |cx| {
            palette
                .update(cx, |_, cx| {
                    cx.emit(PaletteEvent::QueryChanged {
                        page,
                        query: SharedString::default(),
                    });
                })
                .ok();
        });
        Self {
            config,
            theme,
            metrics,
            input,
            root,
            stack: Vec::new(),
            actions: None,
            actions_dismissed_at: None,
            focused: false,
            _subscription: subscription,
        }
    }

    pub fn page(&self) -> &PageId {
        self.current().state.id()
    }

    pub fn depth(&self) -> usize {
        self.stack.len() + 1
    }

    pub fn is_page(&self, id: &str) -> bool {
        self.page() == id
    }

    pub fn query(&self) -> &str {
        &self.current().state.query
    }

    pub fn selected_item(&self) -> Option<&ItemId> {
        self.current().state.selected()
    }

    pub fn actions_open(&self) -> bool {
        self.actions.is_some()
    }

    /// The current page's items in display order.
    #[cfg(any(test, feature = "test-support"))]
    pub fn items(&self) -> Vec<&Item<A>> {
        let state = &self.current().state;
        state
            .rows
            .iter()
            .filter_map(|row| state.row_item(row))
            .collect()
    }

    /// Replaces the current page's query and emits [`PaletteEvent::QueryChanged`].
    pub fn set_query(&mut self, query: impl Into<SharedString>, cx: &mut Context<Self>) {
        let query = query.into();
        self.input
            .update(cx, |input, cx| input.set_text(query.clone(), cx));
        let page = self.current_mut();
        page.state.query = query.to_string();
        let page = page.state.id().clone();
        cx.emit(PaletteEvent::QueryChanged { page, query });
    }

    /// Shows `content` on `page`, a no-op when `page` is not on the stack.
    /// Content that differs from the shown content only in payloads swaps
    /// them in without a redraw.
    pub fn set_content(&mut self, page: &str, content: PageContent<A>, cx: &mut Context<Self>) {
        let Some((target, current)) = self.page_slot(page) else {
            return;
        };
        let ContentChange::Rows { selection_changed } = target.state.set_content(content) else {
            return;
        };
        target.relayout();
        if !current {
            return;
        }
        let page = target.state.id().clone();
        let selected = target.state.selected().cloned();
        if selection_changed {
            cx.emit(PaletteEvent::SelectionChanged {
                page,
                item: selected,
            });
        }
        self.refresh_actions(cx);
        cx.notify();
    }

    pub fn set_status(&mut self, page: &str, status: Status, cx: &mut Context<Self>) {
        let Some((target, current)) = self.page_slot(page) else {
            return;
        };
        if !target.state.set_status(status) || !current {
            return;
        }
        target.relayout();
        self.refresh_actions(cx);
        cx.notify();
    }

    /// Marks `item` as the current one on `page`, such as the active theme.
    pub fn set_current(&mut self, page: &str, item: Option<&str>, cx: &mut Context<Self>) {
        let Some((target, current)) = self.page_slot(page) else {
            return;
        };
        if target.state.set_current(item) && current {
            cx.notify();
        }
    }

    /// Completion text drawn after the query in `page`'s search field.
    pub fn set_ghost(
        &mut self,
        page: &str,
        ghost: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let ghost = ghost.into();
        let Some((target, current)) = self.page_slot(page) else {
            return;
        };
        target.state.ghost = ghost.clone();
        if current {
            self.input
                .update(cx, |input, cx| input.set_ghost(ghost, cx));
        }
    }

    pub fn set_accessory_active(
        &mut self,
        page: &str,
        id: &str,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        let Some((target, current)) = self.page_slot(page) else {
            return;
        };
        if target.state.set_accessory_active(id, active) && current {
            cx.notify();
        }
    }

    /// Selects `item` on `page` when it is there and enabled.
    pub fn select(&mut self, page: &str, item: &str, cx: &mut Context<Self>) -> bool {
        let Some((target, current)) = self.page_slot(page) else {
            return false;
        };
        if !target.state.select(item) {
            return false;
        }
        target.reveal_selection();
        if current {
            cx.notify();
        }
        true
    }

    /// Pushes a page showing "Loading…" until the host sets its content, and
    /// emits its empty [`PaletteEvent::QueryChanged`].
    pub fn push_page(&mut self, spec: PageSpec, cx: &mut Context<Self>) {
        self.drop_actions();
        let page = Page::new(spec, PageContent::loading(LOADING), self.metrics);
        let id = page.state.id().clone();
        self.stack.push(page);
        self.show_current(cx);
        cx.emit(PaletteEvent::QueryChanged {
            page: id,
            query: SharedString::default(),
        });
        cx.notify();
    }

    /// Returns to the previous page, emitting [`PaletteEvent::PagePopped`].
    /// Returns false on the root page.
    pub fn pop_page(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(page) = self.stack.pop() else {
            return false;
        };
        self.drop_actions();
        self.show_current(cx);
        cx.emit(PaletteEvent::PagePopped {
            page: page.state.id().clone(),
        });
        cx.notify();
        true
    }

    pub fn pop_to_root(&mut self, cx: &mut Context<Self>) {
        while self.pop_page(cx) {}
    }

    pub fn open_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.actions.is_some() {
            return;
        }
        let page = self.current();
        let Some(item) = page.state.selected_item() else {
            return;
        };
        let panel = ActionsPanel::new(page.state.id().clone(), item, &self.theme, self.metrics, cx);
        window.focus(&panel.input.focus_handle(cx));
        self.actions = Some(panel);
        self.actions_dismissed_at = None;
        cx.notify();
    }

    pub fn close_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.actions.take().is_none() {
            return;
        }
        window.focus(&self.input.focus_handle(cx));
        cx.notify();
    }

    pub fn set_appearance(&mut self, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) {
        let search = layout::search_style(&theme, metrics);
        let filter = layout::filter_style(&theme, metrics);
        self.theme = theme;
        self.metrics = metrics;
        self.input
            .update(cx, |input, cx| input.set_style(search, cx));
        if let Some(panel) = &self.actions {
            panel
                .input
                .update(cx, |input, cx| input.set_style(filter, cx));
        }
        cx.notify();
    }

    fn current(&self) -> &Page<A> {
        self.stack.last().unwrap_or(&self.root)
    }

    fn current_mut(&mut self) -> &mut Page<A> {
        self.stack.last_mut().unwrap_or(&mut self.root)
    }

    /// The topmost page with `id`, and whether it is the current page.
    fn page_slot(&mut self, id: &str) -> Option<(&mut Page<A>, bool)> {
        let depth = self.stack.len();
        if let Some(index) = self.stack.iter().rposition(|page| page.state.id() == id) {
            return Some((&mut self.stack[index], index + 1 == depth));
        }
        (self.root.state.id() == id).then_some((&mut self.root, depth == 0))
    }

    /// Puts the current page's query, placeholder and ghost in the search field.
    fn show_current(&mut self, cx: &mut Context<Self>) {
        let state = &self.current().state;
        let query = state.query.clone();
        let placeholder = state.spec.placeholder.clone();
        let ghost = state.ghost.clone();
        self.input.update(cx, |input, cx| {
            input.set_text(query, cx);
            input.set_placeholder(placeholder);
            input.set_ghost(ghost, cx);
        });
    }

    /// Closes the ⌘K panel where no window is at hand; the search field takes
    /// focus back on the next render.
    fn drop_actions(&mut self) {
        if self.actions.take().is_some() {
            self.focused = false;
        }
    }

    /// Follows new content in the ⌘K panel, closing it when its item is gone.
    fn refresh_actions(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = &mut self.actions else {
            return;
        };
        let page = self.stack.last().unwrap_or(&self.root);
        let item = (page.state.id() == &panel.page)
            .then(|| page.state.item(&panel.item))
            .flatten()
            .filter(|item| !item.disabled && page.state.shows_rows());
        match item {
            Some(item) => panel.refresh(item, cx),
            None => self.drop_actions(),
        }
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Some(panel) = &mut self.actions {
            panel.move_highlight(delta);
            cx.notify();
            return;
        }
        let page = self.current_mut();
        if !page.state.move_selection(delta) {
            return;
        }
        page.reveal_selection();
        let event = PaletteEvent::SelectionChanged {
            page: page.state.id().clone(),
            item: page.state.selected().cloned(),
        };
        cx.emit(event);
        cx.notify();
    }

    fn emit_primary(&self, item: &ItemId, cx: &mut Context<Self>) {
        let state = &self.current().state;
        let Some(item) = state.item(item).filter(|item| !item.disabled) else {
            return;
        };
        let page = state.id().clone();
        let id = item.id.clone();
        cx.emit(match &item.target {
            Target::Run(payload) => PaletteEvent::Run {
                page,
                item: id,
                payload: payload.clone(),
            },
            Target::Apply(payload) => PaletteEvent::Apply {
                page,
                item: id,
                payload: payload.clone(),
            },
            Target::OpenPage(target) => PaletteEvent::OpenPage {
                page: target.clone(),
                parent: id,
            },
        });
    }

    fn emit_action(&self, item: &ItemId, action: ActionId, cx: &mut Context<Self>) {
        let state = &self.current().state;
        let Some(item) = state.item(item).filter(|item| !item.disabled) else {
            return;
        };
        let Some(payload) = item.target.payload() else {
            return;
        };
        cx.emit(PaletteEvent::RunAction {
            page: state.id().clone(),
            item: item.id.clone(),
            action,
            payload: payload.clone(),
        });
    }

    fn confirm_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel) = &mut self.actions else {
            return;
        };
        let item = panel.item.clone();
        match panel.confirm() {
            Confirmation::Nothing => {}
            Confirmation::Armed => cx.notify(),
            Confirmation::Run { action, keep_open } => {
                if !keep_open {
                    self.close_actions(window, cx);
                }
                match action {
                    Some(action) => self.emit_action(&item, action, cx),
                    None => self.emit_primary(&item, cx),
                }
            }
        }
    }

    fn click_item(&mut self, item: &ItemId, cx: &mut Context<Self>) {
        let page = self.current_mut();
        let moved = page.state.selected() != Some(item);
        if !page.state.select(item) {
            return;
        }
        if moved {
            let page = page.state.id().clone();
            cx.emit(PaletteEvent::SelectionChanged {
                page,
                item: Some(item.clone()),
            });
            cx.notify();
        }
        self.emit_primary(item, cx);
    }

    fn toggle_accessory(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        let page = self.current_mut();
        let Some(active) =
            page.state
                .spec
                .accessories
                .iter()
                .find_map(|accessory| match accessory {
                    Accessory::Toggle {
                        id: candidate,
                        active,
                        ..
                    } if candidate == id => Some(!*active),
                    _ => None,
                })
        else {
            return;
        };
        page.state.set_accessory_active(id, active);
        let page = page.state.id().clone();
        cx.emit(PaletteEvent::AccessoryToggled {
            page,
            id: id.clone(),
            active,
        });
        cx.notify();
    }

    fn on_select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.move_selection(1, cx);
    }

    fn on_select_previous(&mut self, _: &SelectPrevious, _: &mut Window, cx: &mut Context<Self>) {
        self.move_selection(-1, cx);
    }

    fn on_confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if self.actions.is_some() {
            self.confirm_actions(window, cx);
            return;
        }
        if let Some(item) = self.selected_item().cloned() {
            self.emit_primary(&item, cx);
        }
    }

    fn on_secondary_confirm(
        &mut self,
        _: &SecondaryConfirm,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.actions.is_some() {
            return;
        }
        let state = &self.current().state;
        let Some(item) = state.selected_item() else {
            return;
        };
        let Some(action) = item.actions.get(1) else {
            return;
        };
        let (item, action) = (item.id.clone(), action.id.clone());
        self.emit_action(&item, action, cx);
    }

    fn on_dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.actions.is_some() {
            self.close_actions(window, cx);
            return;
        }
        if self.pop_page(cx) {
            return;
        }
        cx.emit(PaletteEvent::Dismissed);
    }

    fn on_tab(&mut self, _: &TabPressed, _: &mut Window, cx: &mut Context<Self>) {
        let state = &self.current().state;
        if self.actions.is_some() || !state.spec.completes_on_tab {
            self.move_selection(1, cx);
            return;
        }
        cx.emit(PaletteEvent::CompletionRequested {
            page: state.id().clone(),
            query: state.query.clone().into(),
            item: state.selected().cloned(),
        });
    }

    fn on_navigate_back(&mut self, _: &NavigateBack, window: &mut Window, cx: &mut Context<Self>) {
        self.close_actions(window, cx);
        self.pop_page(cx);
    }

    fn on_toggle_actions(
        &mut self,
        _: &ToggleActions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.actions.is_some() {
            self.close_actions(window, cx);
        } else {
            self.open_actions(window, cx);
        }
    }

    /// Backspace on an empty query pops a pushed page instead of editing.
    fn on_backspace(&mut self, _: &text_input::Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.actions.is_some() || self.stack.is_empty() || !self.query().is_empty() {
            return;
        }
        self.pop_page(cx);
        cx.stop_propagation();
    }

    fn toggle_keycaps(window: &Window) -> Vec<SharedString> {
        let Ok(context) = KeyContext::parse(KEY_CONTEXT) else {
            return Vec::new();
        };
        window
            .bindings_for_action_in_context(&ToggleActions, context)
            .last()
            .map(|binding| {
                binding
                    .keystrokes()
                    .iter()
                    .map(gpui::KeybindingKeystroke::unparse)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .map(|chord| keycaps::parse(&chord))
            .unwrap_or_default()
    }

    fn render_header(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = &self.theme;
        let metrics = self.metrics;
        let state = &self.current().state;
        let leading = if self.stack.is_empty() {
            div()
                .flex_none()
                .child(IconGlyph::new(
                    Icon::Search,
                    metrics.scaled(18.0),
                    theme.fg_muted,
                ))
                .into_any_element()
        } else {
            div()
                .id("palette-back")
                .debug_selector(|| "palette-back".into())
                .flex()
                .flex_none()
                .items_center()
                .gap(metrics.spacing2())
                .h(metrics.scaled(26.0))
                .px(metrics.spacing4())
                .rounded(metrics.radius_md())
                .bg(theme.fg_alpha(0.08))
                .cursor_pointer()
                .hover(|style| style.bg(theme.fg_alpha(0.12)))
                .text_size(metrics.font_body())
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.fg_muted)
                .on_click(cx.listener(|palette, _, window, cx| {
                    palette.close_actions(window, cx);
                    palette.pop_page(cx);
                }))
                .child(IconGlyph::new(
                    Icon::ChevronLeft,
                    metrics.icon_sm(),
                    theme.fg_muted,
                ))
                .when_some(state.spec.breadcrumb.clone(), ParentElement::child)
                .into_any_element()
        };
        let mut header = div()
            .flex()
            .flex_none()
            .items_center()
            .h(metrics.scaled(layout::HEADER))
            .px(metrics.spacing7())
            .gap(metrics.spacing5())
            .border_b_1()
            .border_color(theme.border)
            .child(leading)
            .child(div().flex_grow().min_w(px(0.0)).child(self.input.clone()));
        for (index, accessory) in state.spec.accessories.iter().enumerate() {
            header = header.child(match accessory {
                Accessory::Toggle { id, label, active } => {
                    let toggled = id.clone();
                    let selector = id.clone();
                    div()
                        .id(("palette-accessory", index))
                        .debug_selector(move || format!("palette-accessory-{selector}"))
                        .flex()
                        .flex_none()
                        .items_center()
                        .h(metrics.control_medium())
                        .px(metrics.spacing4())
                        .rounded_full()
                        .cursor_pointer()
                        .text_size(metrics.font_body())
                        .font_weight(FontWeight::MEDIUM)
                        .when(*active, |pill| {
                            pill.bg(theme.accent_soft).text_color(theme.accent)
                        })
                        .when(!*active, |pill| {
                            pill.border_1()
                                .border_color(theme.border)
                                .text_color(theme.fg_muted)
                                .hover(|style| style.bg(theme.hover))
                        })
                        .on_click(cx.listener(move |palette, _, _, cx| {
                            palette.toggle_accessory(&toggled, cx);
                        }))
                        .child(label.clone())
                        .into_any_element()
                }
                Accessory::Hint { label, keycap } => div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(metrics.spacing3())
                    .text_size(metrics.font_body())
                    .text_color(theme.fg_muted)
                    .child(label.clone())
                    .when_some(keycap.as_ref(), |hint, chord| {
                        hint.child(keycaps::keycaps(&keycaps::parse(chord), theme, metrics))
                    })
                    .into_any_element(),
            });
        }
        header
    }

    fn render_body(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = &self.theme;
        let metrics = self.metrics;
        let page = self.current();
        let message = match &page.state.content.status {
            Status::Ready if page.state.rows.is_empty() => {
                Some((page.state.spec.empty_message.clone(), theme.fg_muted))
            }
            Status::Ready => None,
            Status::Loading(message) | Status::Empty(message) => {
                Some((message.clone(), theme.fg_muted))
            }
            Status::Error(message) => Some((message.clone(), theme.danger)),
        };
        if let Some((message, color)) = message {
            return row::status_message(message, color, metrics);
        }
        let list = gpui::list(
            page.list.clone(),
            cx.processor(|palette, index: usize, _, cx| palette.render_row(index, cx)),
        )
        .size_full()
        .py(metrics.scaled(layout::LIST_INSET_Y));
        div()
            .relative()
            .flex_grow()
            .min_h(px(0.0))
            .child(list)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .h_full()
                    .child(page.scrollbar.element(&page.list, theme.fg_alpha(0.3))),
            )
            .into_any_element()
    }

    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = &self.current().state;
        let Some(row) = state.rows.get(index) else {
            return div().into_any_element();
        };
        let keycaps = match row {
            Row::Section(section) => {
                let title = state
                    .content
                    .sections
                    .get(*section)
                    .and_then(|section| section.title.clone())
                    .unwrap_or_default();
                return row::section_header(title, &self.theme, self.metrics);
            }
            Row::Item { keycaps, .. } => keycaps,
        };
        let Some(item) = state.row_item(row) else {
            return div().into_any_element();
        };
        let id = item.id.clone();
        row::item(
            index,
            item,
            keycaps,
            state.selected() == Some(&item.id),
            &self.theme,
            self.metrics,
            cx.listener(move |palette, _: &ClickEvent, _, cx| palette.click_item(&id, cx)),
        )
    }

    fn render_footer(&self, window: &Window, cx: &mut Context<Self>) -> gpui::Div {
        let theme = &self.theme;
        let metrics = self.metrics;
        let selected = self
            .current()
            .state
            .selected_item()
            .filter(|item| !item.disabled);
        let primary = selected.map(Item::primary_title);
        let glyph = self.config.footer_glyph.clone().map(|glyph| {
            tile::render(
                &Tile {
                    glyph,
                    hue: TileHue::None,
                },
                theme,
                metrics,
            )
        });
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(metrics.scaled(layout::FOOTER))
            .pl(metrics.spacing6())
            .pr(metrics.spacing4())
            .gap(metrics.spacing2())
            .border_t_1()
            .border_color(theme.border)
            .when_some(glyph, ParentElement::child)
            .child(div().flex_grow())
            .when_some(primary, |footer, title| {
                footer
                    .child(
                        row::footer_button(
                            "palette-primary",
                            title,
                            &keycaps::parse("enter"),
                            true,
                            theme,
                            metrics,
                        )
                        .on_click(cx.listener(|palette, _, window, cx| {
                            palette.on_confirm(&Confirm, window, cx);
                        })),
                    )
                    .child(row::footer_separator(theme, metrics))
            })
            .child(
                row::footer_button(
                    "palette-actions-button",
                    "Actions".into(),
                    &Self::toggle_keycaps(window),
                    selected.is_some(),
                    theme,
                    metrics,
                )
                .on_click(cx.listener(
                    |palette, event: &ClickEvent, window, cx| {
                        let dismissed_at = palette.actions_dismissed_at.take();
                        if let ClickEvent::Mouse(click) = event
                            && dismissed_at == Some(click.down.position)
                        {
                            return;
                        }
                        palette.on_toggle_actions(&ToggleActions, window, cx);
                    },
                )),
            )
    }
}

impl<A: Clone + 'static> Render for Palette<A> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused {
            self.focused = true;
            window.focus(&self.input.focus_handle(cx));
        }
        let metrics = self.metrics;
        let viewport = window.viewport_size();
        let margin = metrics.scaled(layout::VIEWPORT_MARGIN);
        let width = metrics
            .scaled(self.config.width)
            .min(viewport.width - margin * 2.0)
            .max(px(0.0));
        let top = metrics
            .scaled(self.config.top)
            .min((viewport.height - margin * 4.0).max(px(0.0)));
        let state = &self.current().state;
        let (sections, items) = state.counts();
        let height = layout::fitted_height(
            sections,
            items,
            state.shows_rows(),
            self.config.max_height,
            metrics,
        )
        .min((viewport.height - top - margin * 2.0).max(px(0.0)));
        let id = self.config.id.clone();
        let panel = div()
            .id(id.clone())
            .debug_selector(move || id.to_string())
            .occlude()
            .flex()
            .flex_col()
            .w(width)
            .h(height)
            .rounded(metrics.scaled(layout::PANEL_RADIUS))
            .border_1()
            .border_color(self.theme.border)
            .bg(self.theme.bg.blend(self.theme.fg_alpha(0.04)))
            .shadow(Elevation::Modal.shadow(self.theme.bg))
            .overflow_hidden()
            .text_color(self.theme.fg)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(self.render_header(cx))
            .child(self.render_body(cx))
            .child(self.render_footer(window, cx));
        let actions = self.actions.as_ref().map(|panel| {
            let corner = gpui::point(
                width - metrics.spacing4(),
                height - metrics.scaled(layout::FOOTER) - metrics.spacing2(),
            );
            panel.render(corner, &self.theme, metrics, cx)
        });
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .pt(top)
            .bg(gpui::hsla(0.0, 0.0, 0.0, 0.3))
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(Self::on_select_next))
            .on_action(cx.listener(Self::on_select_previous))
            .on_action(cx.listener(Self::on_confirm))
            .on_action(cx.listener(Self::on_secondary_confirm))
            .on_action(cx.listener(Self::on_dismiss))
            .on_action(cx.listener(Self::on_tab))
            .on_action(cx.listener(Self::on_navigate_back))
            .on_action(cx.listener(Self::on_toggle_actions))
            .capture_action(cx.listener(Self::on_backspace))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.emit(PaletteEvent::Dismissed)),
            )
            .child(
                div()
                    .relative()
                    .flex_none()
                    .w(width)
                    .h(height)
                    .child(panel)
                    .when_some(actions, ParentElement::child),
            )
    }
}
