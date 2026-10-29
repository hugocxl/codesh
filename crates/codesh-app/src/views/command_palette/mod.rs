//! The command palette: root commands and search, plus one page per kind of
//! thing it can switch to or open. The widget holds no app data; every page
//! file answers its queries from the model.

pub(crate) mod add_project;
mod catalog;
mod files;
mod find;
mod git;
mod projects;
mod servers;
mod terminals;
mod themes;
mod worktrees;

use std::rc::Rc;

use codesh_app_core::{ProjectId, ServerId};
use codesh_ui::command_palette::{
    PageContent, PageId, PageSpec, Palette, PaletteConfig, PaletteEvent, Tile, TileHue,
};
use codesh_ui::icon::Icon;
use gpui::{Action, App, AppContext, Context, Entity, Focusable, SharedString, Window};

pub(crate) use catalog::RootCommand;
pub(crate) use find::FindState;
pub(crate) use terminals::ExistingSessions;

use super::overlays::Overlay;
use crate::model::AppModel;

pub(crate) type Handler = Rc<dyn Fn(&mut AppModel, &mut Window, &mut Context<AppModel>)>;

/// How many recently run root commands lead the empty root page.
pub(crate) const RECENT_LIMIT: usize = 8;

/// Runs `action` where the keyboard focus is, as its shortcut would.
pub(crate) fn dispatch(action: impl Action) -> Handler {
    Rc::new(move |_, window, cx| {
        window.dispatch_action(action.boxed_clone(), cx);
    })
}

/// Every page the palette can show, by its page id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PaletteEntry {
    Root,
    Files,
    Find,
    Branches(ProjectId),
    Changes(ProjectId),
    Worktrees(ProjectId),
    Terminals(ProjectId),
    Projects,
    Workspaces,
    NewTerminal,
    Themes,
    Servers,
    Server(ServerId),
    AddProject,
    AddProjectRemote(ServerId),
}

impl PaletteEntry {
    pub(crate) fn id(self) -> PageId {
        match self {
            Self::Root => "root".into(),
            Self::Files => "files".into(),
            Self::Find => "find".into(),
            Self::Branches(project) => format!("branches:{project}").into(),
            Self::Changes(project) => format!("changes:{project}").into(),
            Self::Worktrees(project) => format!("worktrees:{project}").into(),
            Self::Terminals(project) => format!("terminals:{project}").into(),
            Self::Projects => "projects".into(),
            Self::Workspaces => "workspaces".into(),
            Self::NewTerminal => "new-terminal".into(),
            Self::Themes => "themes".into(),
            Self::Servers => "servers".into(),
            Self::Server(server) => format!("server:{server}").into(),
            Self::AddProject => "add-project".into(),
            Self::AddProjectRemote(server) => format!("add-project-remote:{server}").into(),
        }
    }

    pub(crate) fn parse(id: &str) -> Option<Self> {
        let entry = match id {
            "root" => Self::Root,
            "files" => Self::Files,
            "find" => Self::Find,
            "projects" => Self::Projects,
            "workspaces" => Self::Workspaces,
            "new-terminal" => Self::NewTerminal,
            "themes" => Self::Themes,
            "servers" => Self::Servers,
            "add-project" => Self::AddProject,
            _ => {
                let (kind, parameter) = id.split_once(':')?;
                match kind {
                    "branches" => Self::Branches(parameter.parse().ok()?),
                    "changes" => Self::Changes(parameter.parse().ok()?),
                    "worktrees" => Self::Worktrees(parameter.parse().ok()?),
                    "terminals" => Self::Terminals(parameter.parse().ok()?),
                    "server" => Self::Server(parameter.parse().ok()?),
                    "add-project-remote" => Self::AddProjectRemote(parameter.parse().ok()?),
                    _ => return None,
                }
            }
        };
        Some(entry)
    }
}

/// What a row is, shown as its kind label and tile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Command,
    File,
    Branch,
    Change,
    Worktree,
    Project,
    Workspace,
    Terminal,
    Theme,
    Server,
    Folder,
}

impl Kind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Command => "Command",
            Self::File => "File",
            Self::Branch => "Branch",
            Self::Change => "Change",
            Self::Worktree => "Worktree",
            Self::Project => "Project",
            Self::Workspace => "Workspace",
            Self::Terminal => "Terminal",
            Self::Theme => "Theme",
            Self::Server => "Server",
            Self::Folder => "Folder",
        }
    }

    /// Themes draw their own swatches instead.
    pub(crate) fn tile(self) -> Tile {
        match self {
            Self::Command => Tile::icon(Icon::Play, TileHue::Gray),
            Self::File => Tile::icon(Icon::File, TileHue::Blue),
            Self::Branch => Tile::icon(Icon::GitBranch, TileHue::Orange),
            Self::Change => Tile::icon(Icon::Pencil, TileHue::Orange),
            Self::Worktree => Tile::icon(Icon::GitBranch, TileHue::Purple),
            Self::Project | Self::Folder => Tile::icon(Icon::Folder, TileHue::Blue),
            Self::Workspace => Tile::icon(Icon::Rows, TileHue::Indigo),
            Self::Terminal => Tile::icon(Icon::Terminal, TileHue::Green),
            Self::Theme => Tile::swatches(Vec::new()),
            Self::Server => Tile::icon(Icon::Network, TileHue::Pink),
        }
    }
}

/// The root page's sections, in display order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Category {
    Terminal,
    Panes,
    Files,
    Git,
    Worktrees,
    Projects,
    View,
    Window,
    Remote,
    Settings,
    Custom,
}

impl Category {
    pub(crate) const ALL: [Self; 11] = [
        Self::Terminal,
        Self::Panes,
        Self::Files,
        Self::Git,
        Self::Worktrees,
        Self::Projects,
        Self::View,
        Self::Window,
        Self::Remote,
        Self::Settings,
        Self::Custom,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Panes => "Panes",
            Self::Files => "Files",
            Self::Git => "Git",
            Self::Worktrees => "Worktrees",
            Self::Projects => "Projects",
            Self::View => "View",
            Self::Window => "Window",
            Self::Remote => "Remote",
            Self::Settings => "Settings",
            Self::Custom => "Custom Commands",
        }
    }

    pub(crate) fn tile(self) -> Tile {
        match self {
            Self::Terminal => Tile::icon(Icon::TerminalStack, TileHue::Green),
            Self::Panes => Tile::icon(Icon::Columns, TileHue::Teal),
            Self::Files => Tile::icon(Icon::File, TileHue::Blue),
            Self::Git => Tile::icon(Icon::GitBranch, TileHue::Orange),
            Self::Worktrees => Tile::icon(Icon::GitBranch, TileHue::Purple),
            Self::Projects => Tile::icon(Icon::Folder, TileHue::Blue),
            Self::View => Tile::icon(Icon::PanelLeft, TileHue::Indigo),
            Self::Window => Tile::icon(Icon::AppWindow, TileHue::Gray),
            Self::Remote => Tile::icon(Icon::Network, TileHue::Pink),
            Self::Settings => Tile::icon(Icon::Settings, TileHue::Gray),
            Self::Custom => Tile::icon(Icon::Play, TileHue::Amber),
        }
    }

    fn order(self) -> usize {
        Self::ALL
            .iter()
            .position(|category| *category == self)
            .unwrap_or(usize::MAX)
    }
}

/// The palette's state that outlives one opening.
#[derive(Default)]
pub(crate) struct PaletteState {
    pub(crate) root_query: String,
    pub(crate) find: FindState,
}

impl AppModel {
    pub(crate) fn toggle_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(PaletteEntry::Root, window, cx);
    }

    /// Opens the palette on `entry`, over the root page so Escape returns
    /// there, or closes it when it is already showing `entry`.
    pub(crate) fn open_palette(
        &mut self,
        entry: PaletteEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        if self.palette_page(cx) == Some(entry) {
            self.dismiss_overlay(cx);
            return;
        }
        self.dismiss_overlay(cx);
        let config = PaletteConfig::new("command-palette", catalog::root_spec());
        let (theme, metrics) = (self.theme.clone(), self.metrics);
        let palette = cx.new(|cx| Palette::new(config, theme, metrics, cx));
        self.overlay_subscription = Some(cx.subscribe_in(
            &palette,
            window,
            |model, _, event, window, cx| model.palette_event(event, window, cx),
        ));
        if entry != PaletteEntry::Root {
            let spec = self.page_spec(entry, cx);
            palette.update(cx, |palette, cx| palette.push_page(spec, cx));
        }
        palette.focus_handle(cx).focus(window);
        self.overlay = Some(Overlay::Palette {
            palette,
            add_project: None,
        });
        self.focus_requested = false;
        self.page_opened(entry, cx);
        if entry != PaletteEntry::Root {
            self.root_opened(cx);
        }
        self.refresh_palette(cx);
        cx.notify();
    }

    pub(crate) fn palette_entity(&self) -> Option<Entity<Palette<Handler>>> {
        match &self.overlay {
            Some(Overlay::Palette { palette, .. }) => Some(palette.clone()),
            _ => None,
        }
    }

    /// The page the open palette shows.
    pub(crate) fn palette_page(&self, cx: &App) -> Option<PaletteEntry> {
        let Some(Overlay::Palette { palette, .. }) = &self.overlay else {
            return None;
        };
        PaletteEntry::parse(palette.read(cx).page())
    }

    /// Pushes `entry` on the open palette, optionally with a query typed in.
    pub(crate) fn push_palette_page(
        &mut self,
        entry: PaletteEntry,
        query: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let Some(palette) = self.palette_entity() else {
            return;
        };
        let spec = self.page_spec(entry, cx);
        let query = query.map(str::to_owned);
        palette.update(cx, |palette, cx| {
            palette.push_page(spec, cx);
            if let Some(query) = query {
                palette.set_query(query, cx);
            }
        });
        self.page_opened(entry, cx);
        self.refresh_palette(cx);
    }

    /// Shows the current page's content for its query.
    pub(crate) fn refresh_palette(&mut self, cx: &mut Context<Self>) {
        let Some(palette) = self.palette_entity() else {
            return;
        };
        let (page, query) = {
            let palette = palette.read(cx);
            (palette.page().clone(), palette.query().to_owned())
        };
        let Some(entry) = PaletteEntry::parse(&page) else {
            return;
        };
        let content = self.page_items(entry, &query, cx);
        palette.update(cx, |palette, cx| palette.set_content(&page, content, cx));
    }

    /// Puts `id` first among the root page's suggestions.
    pub(crate) fn note_palette_recent(&mut self, id: &str, cx: &mut Context<Self>) {
        let recent = &mut self.appearance.palette_recent;
        if recent.first().is_some_and(|first| first == id) {
            return;
        }
        recent.retain(|existing| existing != id);
        recent.insert(0, id.to_owned());
        recent.truncate(RECENT_LIMIT);
        self.save_appearance(cx);
    }

    fn page_spec(&self, entry: PaletteEntry, cx: &App) -> PageSpec {
        match entry {
            PaletteEntry::Root => catalog::root_spec(),
            PaletteEntry::Files => self.files_spec(cx),
            PaletteEntry::Find => self.find_spec(cx),
            PaletteEntry::Branches(project) => self.branches_spec(project, cx),
            PaletteEntry::Changes(project) => self.changes_spec(project, cx),
            PaletteEntry::Worktrees(parent) => self.worktrees_spec(parent, cx),
            PaletteEntry::Terminals(project) => self.terminals_spec(project, cx),
            PaletteEntry::Projects => self.projects_spec(cx),
            PaletteEntry::Workspaces => self.workspaces_spec(cx),
            PaletteEntry::NewTerminal => self.new_terminal_spec(cx),
            PaletteEntry::Themes => self.themes_spec(cx),
            PaletteEntry::Servers => self.servers_spec(cx),
            PaletteEntry::Server(server) => self.server_spec(server, cx),
            PaletteEntry::AddProject => self.add_project_spec(ServerId::local(), cx),
            PaletteEntry::AddProjectRemote(server) => self.add_project_spec(server, cx),
        }
    }

    fn page_items(
        &mut self,
        entry: PaletteEntry,
        query: &str,
        cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        match entry {
            PaletteEntry::Root => self.root_items(query, cx),
            PaletteEntry::Files => self.files_items(query, cx),
            PaletteEntry::Find => self.find_items(query, cx),
            PaletteEntry::Branches(project) => self.branches_items(project, query, cx),
            PaletteEntry::Changes(project) => self.changes_items(project, query, cx),
            PaletteEntry::Worktrees(parent) => self.worktrees_items(parent, query, cx),
            PaletteEntry::Terminals(project) => self.terminals_items(project, query, cx),
            PaletteEntry::Projects => self.projects_items(query, cx),
            PaletteEntry::Workspaces => self.workspaces_items(query, cx),
            PaletteEntry::NewTerminal => self.new_terminal_items(query, cx),
            PaletteEntry::Themes => self.themes_items(query, cx),
            PaletteEntry::Servers => self.servers_items(query, cx),
            PaletteEntry::Server(server) => self.server_items(server, query, cx),
            PaletteEntry::AddProject => self.add_project_items(ServerId::local(), query, cx),
            PaletteEntry::AddProjectRemote(server) => self.add_project_items(server, query, cx),
        }
    }

    fn page_opened(&mut self, entry: PaletteEntry, cx: &mut Context<Self>) {
        match entry {
            PaletteEntry::Root => self.root_opened(cx),
            PaletteEntry::Files => self.files_opened(cx),
            PaletteEntry::Find => self.find_opened(cx),
            PaletteEntry::Branches(project) => self.branches_opened(project, cx),
            PaletteEntry::Changes(project) => self.changes_opened(project, cx),
            PaletteEntry::Worktrees(parent) => self.worktrees_opened(parent, cx),
            PaletteEntry::Terminals(project) => self.terminals_opened(project, cx),
            PaletteEntry::Projects => self.projects_opened(cx),
            PaletteEntry::Workspaces => self.workspaces_opened(cx),
            PaletteEntry::NewTerminal => self.new_terminal_opened(cx),
            PaletteEntry::Themes => self.themes_opened(cx),
            PaletteEntry::Servers => self.servers_opened(cx),
            PaletteEntry::Server(server) => self.server_opened(server, cx),
            PaletteEntry::AddProject => self.add_project_opened(ServerId::local(), cx),
            PaletteEntry::AddProjectRemote(server) => self.add_project_opened(server, cx),
        }
    }

    fn page_query_changed(&mut self, entry: PaletteEntry, query: &str, cx: &mut Context<Self>) {
        match entry {
            PaletteEntry::Root => query.clone_into(&mut self.command_palette.root_query),
            PaletteEntry::Find => self.find_query_changed(query, cx),
            PaletteEntry::AddProject => {
                self.add_project_query_changed(ServerId::local(), query, cx);
            }
            PaletteEntry::AddProjectRemote(server) => {
                self.add_project_query_changed(server, query, cx);
            }
            _ => {}
        }
    }

    fn page_toggled(
        &mut self,
        entry: PaletteEntry,
        id: &str,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        if entry == PaletteEntry::Find {
            self.find_toggled(id, active, cx);
        }
    }

    fn page_completion(
        &mut self,
        entry: PaletteEntry,
        query: &str,
        item: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        match entry {
            PaletteEntry::AddProject => {
                self.add_project_completion(ServerId::local(), query, item, cx);
            }
            PaletteEntry::AddProjectRemote(server) => {
                self.add_project_completion(server, query, item, cx);
            }
            _ => {}
        }
    }

    fn page_selection(&mut self, entry: PaletteEntry, item: Option<&str>, cx: &mut Context<Self>) {
        match entry {
            PaletteEntry::AddProject => self.add_project_selection(ServerId::local(), item, cx),
            PaletteEntry::AddProjectRemote(server) => {
                self.add_project_selection(server, item, cx);
            }
            _ => {}
        }
    }

    fn page_action(
        &mut self,
        entry: PaletteEntry,
        item: &str,
        action: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match entry {
            PaletteEntry::Root => self.root_action(item, action, window, cx),
            PaletteEntry::Files => self.files_action(item, action, window, cx),
            PaletteEntry::Find => self.find_action(item, action, window, cx),
            PaletteEntry::Branches(project) => {
                self.branches_action(project, item, action, window, cx);
            }
            PaletteEntry::Changes(project) => {
                self.changes_action(project, item, action, window, cx);
            }
            PaletteEntry::Worktrees(parent) => {
                self.worktrees_action(parent, item, action, window, cx);
            }
            PaletteEntry::Terminals(project) => {
                self.terminals_action(project, item, action, window, cx);
            }
            PaletteEntry::Projects => self.projects_action(item, action, window, cx),
            PaletteEntry::Workspaces => self.workspaces_action(item, action, window, cx),
            PaletteEntry::NewTerminal => self.new_terminal_action(item, action, window, cx),
            PaletteEntry::Themes => self.themes_action(item, action, window, cx),
            PaletteEntry::Servers => self.servers_action(item, action, window, cx),
            PaletteEntry::Server(server) => self.server_action(server, item, action, window, cx),
            PaletteEntry::AddProject => {
                self.add_project_action(ServerId::local(), item, action, window, cx);
            }
            PaletteEntry::AddProjectRemote(server) => {
                self.add_project_action(server, item, action, window, cx);
            }
        }
    }

    fn palette_event(
        &mut self,
        event: &PaletteEvent<Handler>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entry = |page: &SharedString| PaletteEntry::parse(page);
        match event {
            PaletteEvent::QueryChanged { page, query } => {
                let Some(entry) = entry(page) else {
                    return;
                };
                self.page_query_changed(entry, query, cx);
                self.refresh_palette(cx);
            }
            PaletteEvent::SelectionChanged { page, item } => {
                if let Some(entry) = entry(page) {
                    self.page_selection(entry, item.as_ref().map(AsRef::as_ref), cx);
                }
            }
            PaletteEvent::AccessoryToggled { page, id, active } => {
                let Some(entry) = entry(page) else {
                    return;
                };
                self.page_toggled(entry, id, *active, cx);
                self.refresh_palette(cx);
            }
            PaletteEvent::CompletionRequested { page, query, item } => {
                if let Some(entry) = entry(page) {
                    self.page_completion(entry, query, item.as_ref().map(AsRef::as_ref), cx);
                }
            }
            PaletteEvent::OpenPage { page, parent } => {
                let Some(target) = entry(page) else {
                    return;
                };
                if self
                    .palette_entity()
                    .is_some_and(|palette| palette.read(cx).depth() == 1)
                {
                    self.note_palette_recent(parent, cx);
                }
                self.push_palette_page(target, None, cx);
            }
            PaletteEvent::PagePopped { page } => {
                if matches!(
                    entry(page),
                    Some(PaletteEntry::AddProject | PaletteEntry::AddProjectRemote(_))
                ) && let Some(Overlay::Palette { add_project, .. }) = &mut self.overlay
                {
                    *add_project = None;
                }
                self.refresh_palette(cx);
            }
            PaletteEvent::Run { payload, .. } => {
                self.dismiss_overlay(cx);
                self.focus_active(window, cx);
                self.focus_requested = false;
                payload(self, window, cx);
            }
            PaletteEvent::Apply { payload, .. } => {
                payload(self, window, cx);
                self.refresh_palette(cx);
            }
            PaletteEvent::RunAction {
                page, item, action, ..
            } => {
                let Some(entry) = entry(page) else {
                    return;
                };
                self.page_action(entry, item, action, window, cx);
                self.refresh_palette(cx);
            }
            PaletteEvent::Dismissed => {
                self.dismiss_overlay(cx);
                self.focus_active(window, cx);
            }
        }
    }
}
