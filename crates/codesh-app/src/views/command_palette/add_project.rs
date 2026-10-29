//! Open Project: a folder browser over this computer's folders, or another
//! computer's through its server. `/` or `~/` browses paths, Tab completes
//! the highlighted folder, Enter opens and ⌘↩ adds.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

use codesh_app_core::ServerId;
use codesh_ui::command_palette::{
    Item, ItemAction, PageContent, PageSpec, Section, Status, Tile, TileHue,
};
use codesh_ui::icon::Icon;
use gpui::{App, Context, SharedString, Window};

use super::{Handler, Kind, PaletteEntry};
use crate::model::{AppModel, PendingRemote, RemoteIntent};
use crate::picker::path_service::{self, DirectoryItem, TypedPathState};
use crate::picker::remote::RemoteFolders;
use crate::picker::search::{SearchService, Snapshot};
use crate::picker::session::{InputMode, LoadState, Session};
use crate::views::overlays::Overlay;

const RELOAD_DELAY: Duration = Duration::from_millis(100);
const LOADING_MESSAGE_DELAY: Duration = Duration::from_millis(500);
const SEARCH_RESULT_LIMIT: usize = 50;
const DIRECTORY_CACHE_LIMIT: usize = 64;
const DIRECTORY_CACHE_ITEMS: usize = 50_000;

/// The chord the palette binds to its secondary confirm, which adds a folder.
const ADD_CHORD: &str = if cfg!(target_os = "macos") {
    "cmd-enter"
} else {
    "ctrl-shift-enter"
};

/// The folder browser behind an open Open Project page. Dropping it stops
/// its searches.
pub(crate) struct AddProjectPage {
    /// Tells this page apart from one opened later.
    pub(crate) id: usize,
    pub(crate) server: ServerId,
    pub(crate) session: Session,
    pub(crate) search: SearchService,
    /// Another computer's folders, instead of this one's.
    pub(crate) remote: Option<RemoteFolders>,
    /// Why the folder couldn't be listed, when the server said.
    pub(crate) listing_error: Option<String>,
    pub(crate) generation: usize,
    pub(crate) cancelled: Arc<AtomicBool>,
    pub(crate) directory_cache: HashMap<String, Vec<DirectoryItem>>,
    pub(crate) directory_cache_order: Vec<String>,
    /// Folders that are already projects, standardized.
    pub(crate) added: HashSet<String>,
    /// A check in flight or why adding failed, shown until the input changes.
    pub(crate) notice: Option<Status>,
}

/// A row of the page, by its item id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RowId {
    Search(usize),
    Path(usize),
    Typed,
    Finder,
    Location,
}

impl RowId {
    fn parse(id: &str) -> Option<Self> {
        match id {
            "typed" => Some(Self::Typed),
            "finder" => Some(Self::Finder),
            "location" => Some(Self::Location),
            _ => {
                let (kind, index) = id.split_once('-')?;
                let index = index.parse().ok()?;
                match kind {
                    "search" => Some(Self::Search(index)),
                    "path" => Some(Self::Path(index)),
                    _ => None,
                }
            }
        }
    }
}

impl AddProjectPage {
    fn new(
        server: ServerId,
        session: Session,
        search: SearchService,
        remote: Option<RemoteFolders>,
    ) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let added = session
            .project_paths
            .iter()
            .map(|path| path_service::standardize(path))
            .collect();
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            server,
            session,
            search,
            remote,
            listing_error: None,
            generation: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
            directory_cache: HashMap::new(),
            directory_cache_order: Vec::new(),
            added,
            notice: None,
        }
    }

    /// This computer's folders, from `search_root`, warming the folder index
    /// in the background.
    pub(crate) fn local(
        search: SearchService,
        search_root: &str,
        project_paths: Vec<String>,
        cx: &mut App,
    ) -> Self {
        let page = Self::new(
            ServerId::local(),
            Session::new(search_root, project_paths),
            search,
            None,
        );
        let warm_root = page.session.search_root_path.clone();
        let warm_search = page.search.clone();
        let cancelled = page.cancelled.clone();
        cx.background_executor()
            .spawn(async move { warm_search.prepare(&warm_root, &cancelled) })
            .detach();
        page
    }

    /// Another computer's folders, from its Home.
    pub(crate) fn remote(
        server: ServerId,
        folders: RemoteFolders,
        project_paths: Vec<String>,
    ) -> Self {
        let session = Session::remote(&folders.home, project_paths);
        Self::new(server, session, SearchService::new(), Some(folders))
    }

    fn entry(&self) -> PaletteEntry {
        if self.server.is_local() {
            PaletteEntry::AddProject
        } else {
            PaletteEntry::AddProjectRemote(self.server)
        }
    }

    /// The item id of the session's highlighted row.
    fn highlighted_id(&self) -> Option<String> {
        let index = self.session.highlighted_index?;
        Some(match self.session.input_mode() {
            InputMode::FolderSearch => format!("search-{index}"),
            InputMode::Path => format!("path-{index}"),
        })
    }

    /// Highlights the folder row `item`; other rows highlight no folder.
    fn follow_selection(&mut self, item: Option<&str>) {
        match item.and_then(RowId::parse) {
            Some(RowId::Search(index) | RowId::Path(index)) => self.session.select_row(index),
            Some(_) => self.session.highlighted_index = None,
            None => {}
        }
    }

    /// The folder `row` stands for.
    fn row_path(&self, row: RowId) -> Option<String> {
        match row {
            RowId::Search(index) => self
                .session
                .search_results
                .get(index)
                .map(|result| result.path.clone()),
            RowId::Path(index) => {
                let item = self
                    .session
                    .rows
                    .get(index)
                    .filter(|item| !item.is_parent())?;
                Some(child_path(&self.session.path_state().directory_path, item))
            }
            RowId::Typed => self.session.confirmation_path(),
            RowId::Finder => Some(match self.session.input_mode() {
                InputMode::FolderSearch => self.session.search_root_path.clone(),
                InputMode::Path => self.session.path_state().directory_path,
            }),
            RowId::Location => Some(self.session.search_root_path.clone()),
        }
    }

    /// What adding the typed path does.
    fn confirm_title(&self) -> &'static str {
        if self
            .session
            .confirmation_path()
            .is_some_and(|path| self.missing_remote_folder(&path).is_some())
        {
            return "Create & Add Project";
        }
        self.session.top_right_action_title()
    }

    /// Why `path` on another computer can't be added, when the listing of
    /// its parent already shows it isn't there. Otherwise the server checks.
    fn missing_remote_folder(&self, path: &str) -> Option<String> {
        self.remote.as_ref()?;
        let parent = path_service::parent_path(path);
        let name = path_service::last_component(path);
        let listed = self.directory_cache.get(&parent)?;
        if name.is_empty() || listed.iter().any(|item| item.name() == name) {
            return None;
        }
        let parent = self.session.path_service.abbreviated_display_path(&parent);
        Some(format!("There is no folder named {name} in {parent}"))
    }

    fn cache_items(&mut self, directory: &str, items: Vec<DirectoryItem>) {
        if self
            .directory_cache
            .insert(directory.to_owned(), items)
            .is_none()
        {
            self.directory_cache_order.push(directory.to_owned());
        }
        while self.directory_cache_order.len() > DIRECTORY_CACHE_LIMIT
            || self.directory_cache.values().map(Vec::len).sum::<usize>() > DIRECTORY_CACHE_ITEMS
        {
            let evicted = self.directory_cache_order.remove(0);
            self.directory_cache.remove(&evicted);
        }
    }

    /// Everything the page shows, from the session alone.
    fn content(&self) -> PageContent<Handler> {
        let mut sections = Vec::new();
        if !self.session.load_state.is_loading() {
            match self.session.input_mode() {
                InputMode::FolderSearch => {
                    sections.push(Section::new("Folders", self.search_items()));
                }
                InputMode::Path => {
                    let directory = self.session.path_state().directory_path;
                    let title = self
                        .session
                        .path_service
                        .abbreviated_display_path(&directory);
                    sections.push(Section::new(title, self.path_items(&directory)));
                    sections.push(Section::new("Path", vec![self.typed_item()]));
                }
            }
            if self.remote.is_none() {
                sections.push(Section::new("More", self.more_items()));
            }
        }
        let status = match (&self.notice, self.session.load_state) {
            (Some(notice), _) => notice.clone(),
            (
                None,
                LoadState::Loading {
                    shows_message: true,
                },
            ) => Status::Loading("Loading folders…".into()),
            (None, LoadState::Loading { .. }) => Status::Loading(SharedString::default()),
            (None, _) => Status::Ready,
        };
        PageContent { sections, status }
    }

    fn search_items(&self) -> Vec<Item<Handler>> {
        let query = self.session.search_query();
        let results = &self.session.search_results;
        if results.is_empty() {
            return vec![self.unavailable_item()];
        }
        results
            .iter()
            .enumerate()
            .map(|(index, result)| {
                let id: SharedString = format!("search-{index}").into();
                let added = self.added.contains(&result.path);
                self.folder_item(&id, &result.name, added, "add")
                    .highlights(word_ranges(&result.name, query))
                    .subtitle(result.display_path.clone())
                    .actions(self.folder_actions(added, ItemAction::new("browse", "Browse Folder")))
            })
            .collect()
    }

    fn path_items(&self, directory: &str) -> Vec<Item<Handler>> {
        let mut items: Vec<Item<Handler>> = self
            .session
            .rows
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.is_parent())
            .map(|(index, item)| {
                let id: SharedString = format!("path-{index}").into();
                let added = self.added.contains(&child_path(directory, item));
                let mut actions = vec![
                    ItemAction::new("open", "Browse Folder"),
                    ItemAction::new("add", add_title(added)).shortcut(ADD_CHORD),
                ];
                if self.remote.is_none() {
                    actions.push(ItemAction::new("finder", finder_title()));
                }
                actions.push(ItemAction::new("copy", "Copy Path"));
                actions.push(ItemAction::new("parent", "Go to Parent Folder"));
                self.folder_item(&id, item.name(), added, "open")
                    .actions(actions)
            })
            .collect();
        if self.session.shows_unavailable_state() {
            items.push(self.unavailable_item());
        }
        items
    }

    fn folder_item(
        &self,
        id: &SharedString,
        name: &str,
        added: bool,
        enter: &'static str,
    ) -> Item<Handler> {
        let item = Item::apply(id.clone(), name.to_owned(), self.handler(id, enter))
            .kind(Kind::Folder.label())
            .tile(Kind::Folder.tile());
        if added { item.badge("Added") } else { item }
    }

    /// Enter adds a found folder; `secondary` is its ⌘↩.
    fn folder_actions(&self, added: bool, secondary: ItemAction) -> Vec<ItemAction> {
        let mut actions = vec![
            ItemAction::new("add", add_title(added)),
            secondary.shortcut(ADD_CHORD),
        ];
        if self.remote.is_none() {
            actions.push(ItemAction::new("finder", finder_title()));
        }
        actions.push(ItemAction::new("copy", "Copy Path"));
        actions
    }

    /// The typed path itself, which stays reachable when no folder matches.
    fn typed_item(&self) -> Item<Handler> {
        let path = self.session.path_state().standardized_confirm_path;
        let title = self.session.path_service.abbreviated_display_path(&path);
        let confirm = self.confirm_title();
        let id = SharedString::from("typed");
        Item::apply(id.clone(), title, self.handler(&id, "add"))
            .kind(confirm)
            .tile(Tile::icon(Icon::Plus, TileHue::Green))
            .disabled(self.session.confirmation_path().is_none())
            .actions(vec![
                ItemAction::new("add", confirm),
                ItemAction::new("add", confirm).shortcut(ADD_CHORD),
                ItemAction::new("copy", "Copy Path"),
                ItemAction::new("parent", "Go to Parent Folder"),
            ])
    }

    /// Why no folder shows, as a row that can't be chosen.
    fn unavailable_item(&self) -> Item<Handler> {
        let title = if self.session.load_state.read_failed() {
            match (&self.remote, &self.listing_error) {
                (Some(_), Some(error)) => error.clone(),
                _ => "Could not read this folder".to_owned(),
            }
        } else if self.session.input_mode() == InputMode::Path {
            if self.session.path_state().leaf_filter.is_empty() {
                "Folder is empty".to_owned()
            } else {
                "No matching folders".to_owned()
            }
        } else if self.session.search_query().is_empty() {
            "Type to search folders, or enter a path".to_owned()
        } else {
            "No matching folders".to_owned()
        };
        Item::apply(
            "unavailable",
            title,
            Rc::new(|_: &mut AppModel, _: &mut Window, _: &mut Context<AppModel>| {}) as Handler,
        )
        .disabled(true)
    }

    fn more_items(&self) -> Vec<Item<Handler>> {
        let finder = SharedString::from("finder");
        let location = SharedString::from("location");
        let root = self
            .session
            .path_service
            .abbreviated_display_path(&self.session.search_root_path);
        vec![
            Item::apply(
                finder.clone(),
                finder_title(),
                self.handler(&finder, "finder"),
            )
            .tile(Tile::icon(Icon::Folder, TileHue::Gray))
            .actions(vec![ItemAction::new("finder", finder_title())]),
            Item::apply(
                location.clone(),
                "Change Search Location…",
                self.handler(&location, "location"),
            )
            .subtitle(root)
            .tile(Tile::icon(Icon::Search, TileHue::Gray))
            .actions(vec![ItemAction::new("location", "Change Search Location…")]),
        ]
    }

    /// Enter on `id` runs its `action`, resolved against the live session.
    fn handler(&self, id: &SharedString, action: &'static str) -> Handler {
        let (server, id) = (self.server, id.clone());
        Rc::new(move |model, window, cx| {
            model.add_project_action(server, &id, action, window, cx);
        })
    }
}

impl Drop for AddProjectPage {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

fn child_path(directory: &str, item: &DirectoryItem) -> String {
    path_service::standardize(&format!("{directory}/{}", item.name()))
}

fn add_title(added: bool) -> &'static str {
    if added { "Open Project" } else { "Add Project" }
}

fn finder_title() -> &'static str {
    if cfg!(target_os = "macos") {
        "Choose in Finder…"
    } else {
        "Choose Folder…"
    }
}

/// Where each word of `query` first appears in `text`, ignoring ASCII case,
/// since folder search reports no match positions.
fn word_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = query
        .split_whitespace()
        .filter_map(|word| {
            let length = word.len();
            text.char_indices()
                .map(|(start, _)| start..start + length)
                .find(|range| {
                    text.is_char_boundary(range.end)
                        && text
                            .as_bytes()
                            .get(range.clone())
                            .is_some_and(|bytes| bytes.eq_ignore_ascii_case(word.as_bytes()))
                })
        })
        .collect();
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

impl AppModel {
    pub(crate) fn open_project_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(PaletteEntry::AddProject, window, cx);
    }

    /// Opens the same page on another computer's folders, from its Home.
    /// A server that isn't ready yet connects first, asking for its password
    /// if needed, and the page opens once it is.
    pub(crate) fn open_remote_project_picker(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if self.server_name(server).is_none() {
            return;
        }
        if self.remote_folders(server).is_none() {
            self.dismiss_overlay(cx);
            self.pending_remote = Some(PendingRemote::new(server, RemoteIntent::AddProject));
            if self.server_ready(server) && self.side_clients.client(server).is_none() {
                self.connect_side_client(server, cx);
            } else {
                self.connect_remote_server(server, cx);
            }
            cx.notify();
            return;
        }
        self.pending_remote = None;
        let window = self.window;
        let model = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = model.update(cx, |model, cx| {
                    model.open_palette(PaletteEntry::AddProjectRemote(server), window, cx);
                });
            });
        });
    }

    /// `server`'s folders through its side connection, once it is ready.
    fn remote_folders(&self, server: ServerId) -> Option<RemoteFolders> {
        let name = self.server_name(server)?.to_owned();
        let home = self.state.server_home(server)?;
        let (project, home) = (home.id, home.directory.to_string_lossy().into_owned());
        if !self.server_ready(server) || !self.confirmed(server) {
            return None;
        }
        let client = self.side_clients.client(server)?;
        Some(RemoteFolders::through(client, name, project, &home))
    }

    /// The open Open Project page's browser.
    pub(crate) fn add_project_page(&mut self) -> Option<&mut AddProjectPage> {
        match &mut self.overlay {
            Some(Overlay::Palette { add_project, .. }) => add_project.as_deref_mut(),
            _ => None,
        }
    }

    /// The open page's browser while it browses `server`.
    fn server_page(&mut self, server: ServerId) -> Option<&mut AddProjectPage> {
        self.add_project_page().filter(|page| page.server == server)
    }

    /// The open page's browser while it is still page `id` at `generation`.
    fn current_add_project(&mut self, id: usize, generation: usize) -> Option<&mut AddProjectPage> {
        self.add_project_page()
            .filter(|page| page.id == id && page.generation == generation)
    }

    /// Which Open Project page is open, if one is.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Tests tell pages apart with it")
    )]
    pub(crate) fn add_project_page_id(&self) -> Option<usize> {
        match &self.overlay {
            Some(Overlay::Palette {
                add_project: Some(page),
                ..
            }) => Some(page.id),
            _ => None,
        }
    }

    pub(super) fn add_project_spec(&self, server: ServerId, _cx: &App) -> PageSpec {
        if server.is_local() {
            return PageSpec::new(
                PaletteEntry::AddProject.id(),
                "Search folders or enter a path…",
            )
            .breadcrumb("Open Project")
            .completes_on_tab();
        }
        let name = self.server_name(server).unwrap_or("the server");
        PageSpec::new(
            PaletteEntry::AddProjectRemote(server).id(),
            format!("Enter a path on {name}…"),
        )
        .breadcrumb("Open Project")
        .completes_on_tab()
    }

    pub(super) fn add_project_items(
        &mut self,
        server: ServerId,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        match self.server_page(server) {
            Some(page) => page.content(),
            None => PageContent::loading(""),
        }
    }

    /// Starts the browser the page shows, unless it is already there. A
    /// server that isn't ready connects first and opens the page later.
    pub(super) fn add_project_opened(&mut self, server: ServerId, cx: &mut Context<Self>) {
        let Some(Overlay::Palette { add_project, .. }) = &self.overlay else {
            return;
        };
        if add_project
            .as_ref()
            .is_some_and(|page| page.server == server)
        {
            return;
        }
        let paths = self.project_paths(server);
        let page = if server.is_local() {
            let root = self
                .settings
                .projects
                .search_root
                .as_ref()
                .unwrap_or(&self.state.home().directory)
                .to_string_lossy()
                .into_owned();
            AddProjectPage::local(self.picker_search.clone(), &root, paths, cx)
        } else {
            let Some(folders) = self.remote_folders(server) else {
                self.open_remote_project_picker(server, cx);
                return;
            };
            AddProjectPage::remote(server, folders, paths)
        };
        if let Some(Overlay::Palette { add_project, .. }) = &mut self.overlay {
            *add_project = Some(Box::new(page));
        }
        self.apply_add_project_input(cx);
    }

    pub(super) fn add_project_action(
        &mut self,
        server: ServerId,
        item: &str,
        action: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = self.server_page(server) else {
            return;
        };
        let Some(row) = RowId::parse(item) else {
            return;
        };
        page.follow_selection(Some(item));
        let Some(path) = page.row_path(row) else {
            return;
        };
        let local = page.remote.is_none();
        match action {
            "add" => self.confirm_add_project(server, &path, row == RowId::Typed, cx),
            "open" => {
                let Some(folder) = page
                    .session
                    .highlighted_item()
                    .filter(|_| matches!(row, RowId::Path(_)))
                    .cloned()
                else {
                    return;
                };
                page.session.activate(&folder);
                self.apply_add_project_input(cx);
            }
            "browse" => {
                let display = page.session.path_service.abbreviated_display_path(&path);
                page.session.set_input(format!("{display}/"));
                self.apply_add_project_input(cx);
            }
            "parent" => {
                page.session.go_back();
                self.apply_add_project_input(cx);
            }
            "copy" => cx.write_to_clipboard(gpui::ClipboardItem::new_string(path)),
            "finder" if local => self.choose_project_folder(path, false, cx),
            "location" if local => self.choose_project_folder(path, true, cx),
            _ => {}
        }
    }

    pub(super) fn add_project_query_changed(
        &mut self,
        server: ServerId,
        query: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = self.server_page(server) else {
            return;
        };
        if page.session.input == query {
            return;
        }
        page.session.set_input(query);
        self.reload_add_project(cx);
    }

    pub(super) fn add_project_completion(
        &mut self,
        server: ServerId,
        _query: &str,
        item: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = self.server_page(server) else {
            return;
        };
        page.follow_selection(item);
        page.session.complete_highlighted();
        self.apply_add_project_input(cx);
    }

    pub(super) fn add_project_selection(
        &mut self,
        server: ServerId,
        item: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let Some(palette) = self.palette_entity() else {
            return;
        };
        if palette.read(cx).selected_item().map(AsRef::as_ref) != item {
            return;
        }
        let Some(page) = self.server_page(server) else {
            return;
        };
        page.follow_selection(item);
        let (id, ghost) = (page.entry().id(), page.session.ghost_text());
        palette.update(cx, |palette, cx| palette.set_ghost(&id, ghost, cx));
    }

    pub(crate) fn refresh_add_project_page(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.palette_page(cx),
            Some(PaletteEntry::AddProject | PaletteEntry::AddProjectRemote(_))
        ) {
            self.refresh_palette(cx);
        }
    }

    /// Shows the session's input in the search field and lists it.
    fn apply_add_project_input(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.add_project_page() else {
            return;
        };
        let (id, input) = (page.entry().id(), page.session.input.clone());
        if let Some(palette) = self.palette_entity() {
            palette.update(cx, |palette, cx| {
                if palette.is_page(&id) && palette.query() != input {
                    palette.set_query(input, cx);
                }
            });
        }
        self.reload_add_project(cx);
    }

    /// Shows the session on the page, with its highlighted row selected and
    /// the completion of that row as ghost text.
    fn sync_add_project(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.add_project_page() else {
            return;
        };
        if page
            .session
            .highlighted_item()
            .is_some_and(DirectoryItem::is_parent)
        {
            page.session.highlighted_index = None;
        }
        let id = page.entry().id();
        let highlighted = page.highlighted_id();
        let ghost = page.session.ghost_text();
        self.refresh_add_project_page(cx);
        let Some(palette) = self.palette_entity() else {
            return;
        };
        palette.update(cx, |palette, cx| {
            if let Some(highlighted) = highlighted {
                palette.select(&id, &highlighted, cx);
            }
            palette.set_ghost(&id, ghost, cx);
        });
    }

    /// Searches or lists what the session's input names, after a short
    /// pause, dropping results once the input changes.
    fn reload_add_project(&mut self, cx: &mut Context<Self>) {
        let Some(page) = self.add_project_page() else {
            return;
        };
        page.listing_error = None;
        page.notice = None;
        page.generation = page.generation.wrapping_add(1);
        let (id, generation) = (page.id, page.generation);
        let mode = page.session.input_mode();
        let query = page.session.search_query().to_owned();
        let path_state = page.session.path_state();

        if mode == InputMode::FolderSearch && (query.is_empty() || page.remote.is_some()) {
            page.session.apply_search_snapshot(Snapshot::default());
            self.sync_add_project(cx);
            return;
        }
        if mode == InputMode::Path
            && let Some(items) = page
                .directory_cache
                .get(&path_state.directory_path)
                .cloned()
        {
            page.session
                .apply_directory_snapshot(path_state.directory_items(items), false);
            self.sync_add_project(cx);
            return;
        }
        let root = page.session.search_root_path.clone();
        let paths = page.session.project_paths.clone();
        let search = page.search.clone();
        let cancelled = page.cancelled.clone();
        let remote = page.remote.clone();

        self.sync_add_project(cx);
        cx.spawn(async move |model, cx| {
            cx.background_executor().timer(LOADING_MESSAGE_DELAY).await;
            let _ = model.update(cx, |model, cx| {
                if let Some(page) = model.current_add_project(id, generation) {
                    page.session.show_loading_message();
                    model.sync_add_project(cx);
                }
            });
        })
        .detach();

        match mode {
            InputMode::FolderSearch => {
                cx.spawn(async move |model, cx| {
                    cx.background_executor().timer(RELOAD_DELAY).await;
                    if !model
                        .read_with(cx, |model, _| current_page(model, id, generation))
                        .unwrap_or(false)
                    {
                        return;
                    }
                    let snapshot = cx
                        .background_executor()
                        .spawn(async move {
                            search.search(&query, &root, &paths, SEARCH_RESULT_LIMIT, &cancelled)
                        })
                        .await;
                    let _ = model.update(cx, |model, cx| {
                        if let Some(page) = model.current_add_project(id, generation) {
                            page.session.apply_search_snapshot(snapshot);
                            model.sync_add_project(cx);
                        }
                    });
                })
                .detach();
            }
            InputMode::Path => {
                Self::list_add_project_directory(
                    id,
                    generation,
                    path_state.directory_path,
                    remote,
                    cx,
                );
            }
        }
    }

    /// Lists a folder here, or through another computer's server.
    fn list_add_project_directory(
        id: usize,
        generation: usize,
        directory: String,
        remote: Option<RemoteFolders>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |model, cx| {
            cx.background_executor().timer(RELOAD_DELAY).await;
            if !model
                .read_with(cx, |model, _| current_page(model, id, generation))
                .unwrap_or(false)
            {
                return;
            }
            let listed = directory.clone();
            let contents = cx
                .background_executor()
                .spawn(async move {
                    match remote {
                        Some(remote) => remote.list(&listed),
                        None => path_service::directory_contents(&listed)
                            .map_err(|error| error.to_string()),
                    }
                })
                .await;
            let _ = model.update(cx, |model, cx| {
                let Some(page) = model.current_add_project(id, generation) else {
                    return;
                };
                let path_state = page.session.path_state();
                page.listing_error = contents.as_ref().err().cloned();
                match contents {
                    Ok(items) => {
                        page.cache_items(&directory, items.clone());
                        page.session
                            .apply_directory_snapshot(path_state.directory_items(items), false);
                    }
                    Err(_) => page
                        .session
                        .apply_directory_snapshot(path_state.directory_read_failure_items(), true),
                }
                model.sync_add_project(cx);
            });
        })
        .detach();
    }

    /// Adds `path`, or opens it when it is already a project. `typed` adds
    /// what was typed, which may create a missing folder after asking.
    fn confirm_add_project(
        &mut self,
        server: ServerId,
        path: &str,
        typed: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = self.server_page(server) else {
            return;
        };
        let create_if_missing = typed
            && page.session.input_mode() == InputMode::Path
            && (page.remote.is_some()
                || page.session.typed_path_state() == TypedPathState::Missing);
        if server.is_local() {
            self.confirm_project_path(path, create_if_missing, cx);
        } else {
            self.confirm_remote_project_path(server, path, create_if_missing, cx);
        }
    }

    /// Shows `notice` in place of the page's rows until the input changes.
    fn add_project_notice(&mut self, notice: Option<Status>, cx: &mut Context<Self>) {
        let Some(page) = self.add_project_page() else {
            return;
        };
        page.notice = notice;
        self.refresh_add_project_page(cx);
    }

    /// The folders of the projects the sidebar lists on `server`, besides
    /// another computer's Home, which is where its picker starts.
    fn project_paths(&self, server: ServerId) -> Vec<String> {
        self.state
            .projects()
            .iter()
            .filter(|project| {
                project.server_id == server
                    && self.project_listed(project)
                    && !project.is_remote_home()
            })
            .map(|project| project.directory.to_string_lossy().into_owned())
            .collect()
    }

    fn confirm_remote_project_path(
        &mut self,
        server: ServerId,
        path: &str,
        allow_create: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        let Some(page) = self.add_project_page() else {
            return;
        };
        let Some(folders) = page.remote.clone() else {
            return;
        };
        let (id, generation) = (page.id, page.generation);
        let path = path.to_owned();
        let checked_path = path.clone();
        let checking = folders.clone();
        let job = crate::picker::remote::next_job();
        let window = self.window;
        self.add_project_notice(Some(Status::Loading("Checking folder…".into())), cx);
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let mut result = cx
                .background_executor()
                .spawn(async move { checking.check(&checked_path, job).map(Some) })
                .await;
            let current = |cx: &gpui::AsyncApp| {
                model
                    .read_with(cx, |model, _| current_page(model, id, generation))
                    .unwrap_or(false)
            };
            if current(cx) && allow_create && result == Ok(Some(TypedPathState::Missing)) {
                let message = format!(
                    "Codesh will create \"{}\" on {} and add it as a project.",
                    &path, &folders.name
                )
                .to_string();
                result = match create_prompt(window, message, cx).await {
                    Ok(true) if current(cx) => {
                        let creating_path = path.clone();
                        cx.background_executor()
                            .spawn(async move {
                                folders
                                    .create(&creating_path, job)
                                    .map(|()| Some(TypedPathState::Directory))
                            })
                            .await
                    }
                    Ok(_) => Ok(None),
                    Err(error) => Err(error),
                };
            }
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                if !current_page(model, id, generation) {
                    return;
                }
                match result {
                    Ok(Some(TypedPathState::Directory)) => {
                        model.open_remote_project_path(server, &path, cx);
                    }
                    Ok(Some(TypedPathState::Missing)) => model.project_picker_error(
                        "Choose an existing folder, or use Create & Add Project.".to_string(),
                        cx,
                    ),
                    Ok(Some(TypedPathState::NotDirectory)) => model.project_picker_error(
                        "This path is a file. Choose a folder for the project.".to_string(),
                        cx,
                    ),
                    Ok(None) => model.add_project_notice(None, cx),
                    Err(error) => model.project_picker_error(error, cx),
                }
                cx.notify();
            });
        }));
    }

    /// Adds a folder on another computer; its server checks that it exists.
    fn open_remote_project_path(&mut self, server: ServerId, path: &str, cx: &mut Context<Self>) {
        let path = path_service::standardize(path);
        let existing = self
            .state
            .projects()
            .iter()
            .find(|project| {
                project.server_id == server
                    && path_service::standardize(&project.directory.to_string_lossy()) == path
            })
            .map(|project| project.id);
        if self
            .state
            .server_home(server)
            .is_some_and(|home| Some(home.id) == existing)
        {
            let message = match self.server_name(server) {
                Some(name) => format!("Choose a folder inside the home folder on {name}."),
                None => "Choose a folder inside the home folder on the server.".to_string(),
            };
            self.project_picker_error(message.clone(), cx);
        } else if let Some(id) = existing {
            self.join_active_workspace(id, cx);
            self.select_project(id, cx);
            self.dismiss_overlay(cx);
        } else if self
            .add_project_on(server, PathBuf::from(path), cx)
            .is_some()
        {
            self.dismiss_overlay(cx);
        } else if let Some(error) = self.error.clone() {
            self.project_picker_error(error, cx);
        }
    }

    fn confirm_project_path(
        &mut self,
        path: &str,
        create_if_missing: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        let path = PathBuf::from(path_service::standardize(path));
        if !path.exists() && create_if_missing {
            let window = self.window;
            let message = format!(
                "Codesh will create \"{}\" and add it as a project.",
                path.display()
            )
            .to_string();
            self.close_prompt = Some(cx.spawn(async move |model, cx| {
                let response = create_prompt(window, message, cx).await;
                if response.as_ref().is_ok_and(|confirmed| *confirmed) {
                    let created_path = path.clone();
                    let result = cx
                        .background_executor()
                        .spawn(async move { std::fs::create_dir_all(created_path) })
                        .await;
                    let _ = model.update(cx, |model, cx| {
                        model.finish_confirmation(cx);
                        match result {
                            Ok(()) => model.open_project_path(path, cx),
                            Err(error) => model.project_picker_error(
                                format!("Could not create folder: {error}"),
                                cx,
                            ),
                        }
                    });
                } else {
                    let _ = model.update(cx, |model, cx| {
                        model.finish_confirmation(cx);
                        if let Err(error) = response {
                            model.project_picker_error(error, cx);
                        }
                        cx.notify();
                    });
                }
            }));
        } else {
            self.open_project_path(path, cx);
        }
    }

    pub(crate) fn open_folders_from(
        folders: async_channel::Receiver<Vec<String>>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Ok(urls) = folders.recv().await {
                if this
                    .update(cx, |model, cx| {
                        if urls.is_empty() {
                            model.raise_window(cx);
                        }
                        model
                            .pending_folders
                            .extend(urls.iter().filter_map(|url| folder_from_url(url)));
                        if model.can_open_local_folders() {
                            model.resume_folders(ServerId::local(), cx);
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn resume_folders(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if server.is_local() {
            for folder in std::mem::take(&mut self.pending_folders) {
                self.open_folder(folder, cx);
            }
        }
    }

    fn open_folder(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        let canonical = folder.canonicalize().ok();
        let existing = self
            .state
            .projects()
            .iter()
            .find(|project| {
                project.server_id.is_local()
                    && canonical.is_some()
                    && project.directory.canonicalize().ok() == canonical
            })
            .map(|project| project.id);
        match existing {
            Some(id) => self.show_project(id, cx),
            None => self.open_project_path(folder, cx),
        }
    }

    fn open_project_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.is_dir() {
            self.project_picker_error(
                "Choose an existing folder, or use Create & Add Project.".to_string(),
                cx,
            );
            return;
        }
        let existing = self
            .state
            .projects()
            .iter()
            .find(|project| {
                project.server_id.is_local()
                    && path_service::standardize(&project.directory.to_string_lossy())
                        == path.to_string_lossy()
            })
            .map(|project| project.id);
        if let Some(id) = existing {
            self.show_project(id, cx);
        } else if self.add_project(path, cx).is_some() {
            self.dismiss_overlay(cx);
        } else if let Some(error) = self.error.clone() {
            self.project_picker_error(error, cx);
        }
    }

    fn show_project(&mut self, id: codesh_app_core::ProjectId, cx: &mut Context<Self>) {
        self.refresh_project_statuses(cx);
        self.join_active_workspace(id, cx);
        self.select_project(id, cx);
        self.dismiss_overlay(cx);
    }

    /// Shows `error` on the open Open Project page, or as the app's error.
    fn project_picker_error(&mut self, error: String, cx: &mut Context<Self>) {
        if self.add_project_page().is_none() {
            self.fail(error, cx);
            return;
        }
        self.add_project_notice(Some(Status::Error(error.into())), cx);
    }

    fn choose_project_folder(
        &mut self,
        directory: String,
        search_location: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        self.dismiss_overlay(cx);
        let window = self.window;
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = folder_prompt(window, directory.into(), search_location, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                match response {
                    Ok(Some(path)) if search_location => model.save_project_search_root(path, cx),
                    Ok(Some(path)) => model.open_project_path(path, cx),
                    Ok(None) => {}
                    Err(error) => model.fail(error, cx),
                }
                cx.notify();
            });
        }));
    }
}

/// Whether the Open Project page `id` is still open at `generation`.
fn current_page(model: &AppModel, id: usize, generation: usize) -> bool {
    matches!(
        &model.overlay,
        Some(Overlay::Palette { add_project: Some(page), .. })
            if page.id == id && page.generation == generation
    )
}

fn folder_from_url(url: &str) -> Option<PathBuf> {
    let url = reqwest::Url::parse(url).ok()?;
    if url.scheme() != "file" || url.query().is_some() || url.fragment().is_some() {
        return None;
    }
    let path = url.to_file_path().ok()?;
    path.is_dir().then(|| path.components().collect())
}

#[cfg(not(test))]
async fn create_prompt(
    window: gpui::AnyWindowHandle,
    message: String,
    cx: &mut gpui::AsyncApp,
) -> Result<bool, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, cx| {
            codesh_ui::dialog::confirm(
                window,
                cx,
                "Create Project Folder?",
                &message,
                "Create & Add",
                None,
                move |response| {
                    let _ = sender.try_send(response);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(matches!(
        receiver.recv().await,
        Ok(codesh_ui::dialog::ConfirmationResponse::Confirmed { .. })
    ))
}

#[cfg(test)]
async fn create_prompt(
    window: gpui::AnyWindowHandle,
    message: String,
    cx: &mut gpui::AsyncApp,
) -> Result<bool, String> {
    let response = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                "Create Project Folder?",
                Some(&message),
                &["Create & Add", "Cancel"],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(response.await == Ok(0))
}

#[cfg(not(test))]
async fn folder_prompt(
    window: gpui::AnyWindowHandle,
    directory: PathBuf,
    search_location: bool,
    cx: &mut gpui::AsyncApp,
) -> Result<Option<PathBuf>, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, _, cx| {
            codesh_ui::dialog::choose_folder(
                cx,
                if search_location {
                    "Select where Codesh searches for project folders"
                } else {
                    "Select a project folder"
                },
                &directory,
                move |path| {
                    let _ = sender.try_send(path);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    match receiver.recv().await {
        Ok(answer) => answer.map_err(|error| error.to_string()),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
async fn folder_prompt(
    _: gpui::AnyWindowHandle,
    _: PathBuf,
    _: bool,
    cx: &mut gpui::AsyncApp,
) -> Result<Option<PathBuf>, String> {
    let response = cx.update(|cx| {
        cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        })
    });
    response
        .map_err(|error| error.to_string())?
        .await
        .map_err(|error| error.to_string())?
        .map(|paths| paths.and_then(|paths| paths.into_iter().next()))
        .map_err(|error| error.to_string())
}
