//! The data a host hands the palette, and the per-page state built from it.

use std::fmt;
use std::ops::Range;

use gpui::SharedString;

use super::keycaps;
use super::tile::Tile;

pub type PageId = SharedString;
pub type ItemId = SharedString;
pub type ActionId = SharedString;

/// Whether a page shows its rows or a message in their place.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Status {
    #[default]
    Ready,
    Loading(SharedString),
    Empty(SharedString),
    Error(SharedString),
}

/// What Enter does on an item.
#[derive(Clone)]
pub enum Target<A> {
    /// The host closes the palette, then runs the payload.
    Run(A),
    /// The host runs the payload and the palette stays open.
    Apply(A),
    /// The palette asks the host to push this page.
    OpenPage(PageId),
}

impl<A> fmt::Debug for Target<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Run(_) => f.write_str("Run"),
            Self::Apply(_) => f.write_str("Apply"),
            Self::OpenPage(page) => f.debug_tuple("OpenPage").field(page).finish(),
        }
    }
}

impl<A> Target<A> {
    /// The payload of a `Run` or `Apply` target.
    pub fn payload(&self) -> Option<&A> {
        match self {
            Self::Run(payload) | Self::Apply(payload) => Some(payload),
            Self::OpenPage(_) => None,
        }
    }

    fn same_appearance(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Run(_), Self::Run(_)) | (Self::Apply(_), Self::Apply(_)) => true,
            (Self::OpenPage(left), Self::OpenPage(right)) => left == right,
            _ => false,
        }
    }
}

/// An entry of an item's ⌘K panel. `actions[0]` describes Enter and
/// `actions[1]` the secondary confirm (⌘↩).
#[derive(Clone, Debug, Eq, PartialEq)]
#[must_use]
pub struct ItemAction {
    pub id: ActionId,
    pub title: SharedString,
    /// Shown next to the action only; it does not bind a key.
    pub shortcut: Option<SharedString>,
    /// Drawn in the danger colour, and needs a second Enter in the ⌘K panel.
    pub destructive: bool,
    /// Keeps the ⌘K panel open after the action runs.
    pub keep_open: bool,
}

impl ItemAction {
    pub fn new(id: impl Into<ActionId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            shortcut: None,
            destructive: false,
            keep_open: false,
        }
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    pub fn keep_open(mut self) -> Self {
        self.keep_open = true;
        self
    }
}

/// One palette row.
#[derive(Clone)]
#[must_use]
pub struct Item<A> {
    pub id: ItemId,
    pub title: SharedString,
    /// Byte ranges of `title` drawn in the accent colour.
    pub highlights: Vec<Range<usize>>,
    pub subtitle: Option<SharedString>,
    /// The muted label at the row's right edge, such as "Command" or "File".
    pub kind: Option<SharedString>,
    pub badge: Option<SharedString>,
    pub tile: Tile,
    /// A chord such as `cmd-shift-p`, drawn as keycaps.
    pub shortcut: Option<SharedString>,
    pub actions: Vec<ItemAction>,
    pub disabled: bool,
    pub current: bool,
    pub target: Target<A>,
}

impl<A> fmt::Debug for Item<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Item")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("disabled", &self.disabled)
            .field("current", &self.current)
            .field("target", &self.target)
            .finish_non_exhaustive()
    }
}

impl<A> Item<A> {
    pub fn new(id: impl Into<ItemId>, title: impl Into<SharedString>, target: Target<A>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            highlights: Vec::new(),
            subtitle: None,
            kind: None,
            badge: None,
            tile: Tile::none(),
            shortcut: None,
            actions: Vec::new(),
            disabled: false,
            current: false,
            target,
        }
    }

    pub fn run(id: impl Into<ItemId>, title: impl Into<SharedString>, payload: A) -> Self {
        Self::new(id, title, Target::Run(payload))
    }

    pub fn apply(id: impl Into<ItemId>, title: impl Into<SharedString>, payload: A) -> Self {
        Self::new(id, title, Target::Apply(payload))
    }

    pub fn page(
        id: impl Into<ItemId>,
        title: impl Into<SharedString>,
        page: impl Into<PageId>,
    ) -> Self {
        Self::new(id, title, Target::OpenPage(page.into()))
    }

    pub fn highlights(mut self, highlights: Vec<Range<usize>>) -> Self {
        self.highlights = highlights;
        self
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn kind(mut self, kind: impl Into<SharedString>) -> Self {
        self.kind = Some(kind.into());
        self
    }

    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn tile(mut self, tile: Tile) -> Self {
        self.tile = tile;
        self
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn actions(mut self, actions: Vec<ItemAction>) -> Self {
        self.actions = actions;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn current(mut self, current: bool) -> Self {
        self.current = current;
        self
    }

    /// What Enter does, as the footer and the ⌘K panel name it.
    pub fn primary_title(&self) -> SharedString {
        if let Some(action) = self.actions.first() {
            return action.title.clone();
        }
        match self.target {
            Target::Run(_) => "Run".into(),
            Target::Apply(_) => "Apply".into(),
            Target::OpenPage(_) => "Open".into(),
        }
    }

    fn same_appearance(&self, other: &Self) -> bool {
        self.id == other.id
            && self.title == other.title
            && self.highlights == other.highlights
            && self.subtitle == other.subtitle
            && self.kind == other.kind
            && self.badge == other.badge
            && self.tile == other.tile
            && self.shortcut == other.shortcut
            && self.actions == other.actions
            && self.disabled == other.disabled
            && self.current == other.current
            && self.target.same_appearance(&other.target)
    }
}

#[derive(Clone)]
pub struct Section<A> {
    pub title: Option<SharedString>,
    pub items: Vec<Item<A>>,
}

impl<A> fmt::Debug for Section<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Section")
            .field("title", &self.title)
            .field("items", &self.items)
            .finish()
    }
}

impl<A> Section<A> {
    pub fn new(title: impl Into<SharedString>, items: Vec<Item<A>>) -> Self {
        Self {
            title: Some(title.into()),
            items,
        }
    }

    pub fn untitled(items: Vec<Item<A>>) -> Self {
        Self { title: None, items }
    }
}

/// Everything a page shows. Sections without items are skipped.
#[derive(Clone)]
pub struct PageContent<A> {
    pub sections: Vec<Section<A>>,
    pub status: Status,
}

impl<A> fmt::Debug for PageContent<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PageContent")
            .field("sections", &self.sections)
            .field("status", &self.status)
            .finish()
    }
}

impl<A> Default for PageContent<A> {
    fn default() -> Self {
        Self {
            sections: Vec::new(),
            status: Status::Ready,
        }
    }
}

impl<A> PageContent<A> {
    pub fn sections(sections: Vec<Section<A>>) -> Self {
        Self {
            sections,
            status: Status::Ready,
        }
    }

    pub fn items(items: Vec<Item<A>>) -> Self {
        Self::sections(vec![Section::untitled(items)])
    }

    pub fn loading(message: impl Into<SharedString>) -> Self {
        Self::status(Status::Loading(message.into()))
    }

    pub fn empty(message: impl Into<SharedString>) -> Self {
        Self::status(Status::Empty(message.into()))
    }

    pub fn error(message: impl Into<SharedString>) -> Self {
        Self::status(Status::Error(message.into()))
    }

    fn status(status: Status) -> Self {
        Self {
            sections: Vec::new(),
            status,
        }
    }

    fn same_appearance(&self, other: &Self) -> bool {
        self.status == other.status
            && self.sections.len() == other.sections.len()
            && self
                .sections
                .iter()
                .zip(&other.sections)
                .all(|(left, right)| {
                    left.title == right.title
                        && left.items.len() == right.items.len()
                        && left
                            .items
                            .iter()
                            .zip(&right.items)
                            .all(|(left, right)| left.same_appearance(right))
                })
    }
}

/// A control at the right of the search field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Accessory {
    /// A pill the user toggles, such as case sensitivity.
    Toggle {
        id: SharedString,
        label: SharedString,
        active: bool,
    },
    /// A muted hint, optionally with a keycap chord such as `tab`.
    Hint {
        label: SharedString,
        keycap: Option<SharedString>,
    },
}

/// How a page presents itself. Content arrives separately through
/// [`super::Palette::set_content`].
#[derive(Clone, Debug, Eq, PartialEq)]
#[must_use]
pub struct PageSpec {
    pub id: PageId,
    pub placeholder: SharedString,
    /// The back chip label on pushed pages.
    pub breadcrumb: Option<SharedString>,
    pub accessories: Vec<Accessory>,
    /// Tab asks the host to complete the query instead of moving the selection.
    pub completes_on_tab: bool,
    /// Shown when the page is ready but has no rows.
    pub empty_message: SharedString,
}

impl PageSpec {
    pub fn new(id: impl Into<PageId>, placeholder: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            placeholder: placeholder.into(),
            breadcrumb: None,
            accessories: Vec::new(),
            completes_on_tab: false,
            empty_message: "No results".into(),
        }
    }

    pub fn breadcrumb(mut self, breadcrumb: impl Into<SharedString>) -> Self {
        self.breadcrumb = Some(breadcrumb.into());
        self
    }

    pub fn accessories(mut self, accessories: Vec<Accessory>) -> Self {
        self.accessories = accessories;
        self
    }

    pub fn accessory(mut self, accessory: Accessory) -> Self {
        self.accessories.push(accessory);
        self
    }

    pub fn completes_on_tab(mut self) -> Self {
        self.completes_on_tab = true;
        self
    }

    pub fn empty_message(mut self, message: impl Into<SharedString>) -> Self {
        self.empty_message = message.into();
        self
    }
}

/// A flattened list row pointing into [`PageContent::sections`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Row {
    Section(usize),
    Item {
        section: usize,
        item: usize,
        keycaps: Vec<SharedString>,
    },
}

/// How [`PageState::set_content`] changed the page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ContentChange {
    /// Only payloads were swapped; nothing to redraw.
    Payloads,
    /// Rows changed and must be laid out again.
    Rows { selection_changed: bool },
}

/// One page of the stack: its spec, content, query and selection.
pub(super) struct PageState<A> {
    pub(super) spec: PageSpec,
    pub(super) content: PageContent<A>,
    pub(super) rows: Vec<Row>,
    pub(super) query: String,
    pub(super) ghost: SharedString,
    selected: Option<ItemId>,
    counts: (usize, usize),
}

impl<A> fmt::Debug for PageState<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PageState")
            .field("spec", &self.spec)
            .field("content", &self.content)
            .field("query", &self.query)
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl<A> PageState<A> {
    pub(super) fn new(spec: PageSpec, content: PageContent<A>) -> Self {
        let mut page = Self {
            spec,
            content,
            rows: Vec::new(),
            query: String::new(),
            ghost: SharedString::default(),
            selected: None,
            counts: (0, 0),
        };
        page.rebuild_rows();
        page.selected = page.first_selectable();
        page
    }

    pub(super) fn id(&self) -> &PageId {
        &self.spec.id
    }

    /// Replaces the content. When only payloads differ the rows are kept as
    /// they are; otherwise the selection keeps its item when it is still
    /// selectable and falls back to the first selectable item.
    pub(super) fn set_content(&mut self, content: PageContent<A>) -> ContentChange {
        if self.content.same_appearance(&content) {
            self.content = content;
            return ContentChange::Payloads;
        }
        self.content = content;
        self.rebuild_rows();
        let previous = self.selected.take();
        self.selected = previous
            .clone()
            .filter(|id| self.is_selectable(id))
            .or_else(|| self.first_selectable());
        ContentChange::Rows {
            selection_changed: previous != self.selected,
        }
    }

    pub(super) fn set_status(&mut self, status: Status) -> bool {
        if self.content.status == status {
            return false;
        }
        self.content.status = status;
        true
    }

    pub(super) fn set_current(&mut self, current: Option<&str>) -> bool {
        let mut changed = false;
        for item in self.items_mut() {
            let is_current = current.is_some_and(|id| item.id == id);
            changed |= item.current != is_current;
            item.current = is_current;
        }
        changed
    }

    pub(super) fn set_accessory_active(&mut self, id: &str, active: bool) -> bool {
        let mut changed = false;
        for accessory in &mut self.spec.accessories {
            if let Accessory::Toggle {
                id: candidate,
                active: current,
                ..
            } = accessory
                && candidate == id
                && *current != active
            {
                *current = active;
                changed = true;
            }
        }
        changed
    }

    /// Whether rows show; otherwise a status message takes their place.
    pub(super) fn shows_rows(&self) -> bool {
        self.content.status == Status::Ready && !self.rows.is_empty()
    }

    pub(super) fn counts(&self) -> (usize, usize) {
        self.counts
    }

    pub(super) fn item(&self, id: &str) -> Option<&Item<A>> {
        self.items().find(|item| item.id == id)
    }

    pub(super) fn row_item(&self, row: &Row) -> Option<&Item<A>> {
        let Row::Item { section, item, .. } = row else {
            return None;
        };
        self.content.sections.get(*section)?.items.get(*item)
    }

    pub(super) fn items(&self) -> impl Iterator<Item = &Item<A>> {
        self.content
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
    }

    fn items_mut(&mut self) -> impl Iterator<Item = &mut Item<A>> {
        self.content
            .sections
            .iter_mut()
            .flat_map(|section| section.items.iter_mut())
    }

    /// The selected item id, only while rows show.
    pub(super) fn selected(&self) -> Option<&ItemId> {
        if self.content.status != Status::Ready {
            return None;
        }
        self.selected.as_ref()
    }

    pub(super) fn selected_item(&self) -> Option<&Item<A>> {
        self.item(self.selected()?)
    }

    pub(super) fn selected_row(&self) -> Option<usize> {
        let selected = self.selected()?;
        self.rows
            .iter()
            .position(|row| self.row_item(row).is_some_and(|item| item.id == *selected))
    }

    pub(super) fn select(&mut self, id: &str) -> bool {
        if !self.is_selectable(id) {
            return false;
        }
        self.selected = Some(SharedString::from(id.to_owned()));
        true
    }

    /// Moves the selection by `delta` selectable items, wrapping around.
    /// Returns whether it moved.
    pub(super) fn move_selection(&mut self, delta: isize) -> bool {
        if self.content.status != Status::Ready {
            return false;
        }
        let selectable: Vec<&ItemId> = self
            .rows
            .iter()
            .filter_map(|row| self.row_item(row))
            .filter(|item| !item.disabled)
            .map(|item| &item.id)
            .collect();
        if selectable.is_empty() {
            return false;
        }
        let count = selectable.len();
        let next = match self
            .selected
            .as_ref()
            .and_then(|selected| selectable.iter().position(|id| *id == selected))
        {
            Some(current) if delta >= 0 => (current + delta.unsigned_abs()) % count,
            Some(current) => (current + count - delta.unsigned_abs() % count) % count,
            None if delta >= 0 => 0,
            None => count - 1,
        };
        let next = selectable[next].clone();
        if self.selected.as_ref() == Some(&next) {
            return false;
        }
        self.selected = Some(next);
        true
    }

    fn is_selectable(&self, id: &str) -> bool {
        self.item(id).is_some_and(|item| !item.disabled)
    }

    fn first_selectable(&self) -> Option<ItemId> {
        self.rows
            .iter()
            .filter_map(|row| self.row_item(row))
            .find(|item| !item.disabled)
            .map(|item| item.id.clone())
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        self.counts = (0, 0);
        for (section_index, section) in self.content.sections.iter().enumerate() {
            if section.items.is_empty() {
                continue;
            }
            if section.title.is_some() {
                self.counts.0 += 1;
                self.rows.push(Row::Section(section_index));
            }
            self.counts.1 += section.items.len();
            for (item_index, item) in section.items.iter().enumerate() {
                self.rows.push(Row::Item {
                    section: section_index,
                    item: item_index,
                    keycaps: item
                        .shortcut
                        .as_ref()
                        .map(|chord| keycaps::parse(chord))
                        .unwrap_or_default(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(content: PageContent<&'static str>) -> PageState<&'static str> {
        PageState::new(PageSpec::new("root", "Search"), content)
    }

    fn sections() -> PageContent<&'static str> {
        PageContent::sections(vec![
            Section::new(
                "Local",
                vec![
                    Item::run("main", "main", "switch main").disabled(true),
                    Item::run("feature", "feature", "switch feature"),
                ],
            ),
            Section::new("Remote", vec![Item::run("origin", "origin/main", "fetch")]),
            Section::new("Empty", Vec::new()),
        ])
    }

    #[test]
    fn selection_skips_disabled_items_and_survives_reorders() {
        let mut state = page(sections());
        assert_eq!(state.selected().map(AsRef::as_ref), Some("feature"));
        assert_eq!(state.counts(), (2, 3));
        assert!(state.move_selection(1));
        assert_eq!(state.selected().map(AsRef::as_ref), Some("origin"));
        assert!(state.move_selection(1));
        assert_eq!(state.selected().map(AsRef::as_ref), Some("feature"));
        assert!(state.move_selection(-1));
        assert_eq!(state.selected().map(AsRef::as_ref), Some("origin"));

        let change = state.set_content(PageContent::items(vec![
            Item::run("origin", "origin/main", "fetch"),
            Item::run("feature", "feature", "switch feature"),
        ]));
        assert_eq!(
            change,
            ContentChange::Rows {
                selection_changed: false
            }
        );
        assert_eq!(state.selected().map(AsRef::as_ref), Some("origin"));
        assert_eq!(state.selected_row(), Some(0));

        let change = state.set_content(PageContent::items(vec![
            Item::run("origin", "origin/main", "fetch").disabled(true),
            Item::run("feature", "feature", "switch feature"),
        ]));
        assert_eq!(
            change,
            ContentChange::Rows {
                selection_changed: true
            }
        );
        assert_eq!(state.selected().map(AsRef::as_ref), Some("feature"));
    }

    #[test]
    fn identical_appearance_swaps_payloads_without_touching_rows() {
        let mut state = page(sections());
        assert!(state.move_selection(1));
        let change = state.set_content(PageContent::sections(vec![
            Section::new(
                "Local",
                vec![
                    Item::run("main", "main", "new main").disabled(true),
                    Item::run("feature", "feature", "new feature"),
                ],
            ),
            Section::new("Remote", vec![Item::run("origin", "origin/main", "new")]),
            Section::new("Empty", Vec::new()),
        ]));
        assert_eq!(change, ContentChange::Payloads);
        assert_eq!(state.selected().map(AsRef::as_ref), Some("origin"));
        assert_eq!(
            state.selected_item().and_then(|item| item.target.payload()),
            Some(&"new")
        );

        let change = state.set_content(PageContent::sections(vec![
            Section::new(
                "Local",
                vec![
                    Item::run("main", "main", "new main").disabled(true),
                    Item::apply("feature", "feature", "new feature"),
                ],
            ),
            Section::new("Remote", vec![Item::run("origin", "origin/main", "new")]),
        ]));
        assert!(matches!(change, ContentChange::Rows { .. }));
    }

    #[test]
    fn status_hides_the_selection_until_ready() {
        let mut state = page(PageContent::loading("Loading…"));
        assert_eq!(state.selected(), None);
        assert!(!state.move_selection(1));
        state.set_content(sections());
        assert_eq!(state.selected().map(AsRef::as_ref), Some("feature"));
        assert!(state.set_status(Status::Error("Failed".into())));
        assert_eq!(state.selected(), None);
        assert!(state.set_status(Status::Ready));
        assert_eq!(state.selected().map(AsRef::as_ref), Some("feature"));
    }
}
