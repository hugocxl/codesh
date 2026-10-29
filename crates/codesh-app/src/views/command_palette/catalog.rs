//! The root page: every command, by category, and one search over commands
//! and the top hits of the pages that list things.

use std::ops::Range;
use std::rc::Rc;

use codesh_core::capability::Feature;
use codesh_core::shortcuts::ShortcutId;
use codesh_protocol::GitAction;
use codesh_ui::command_palette::{Item, PageContent, PageSpec, Section, Target};
use codesh_ui::fuzzy;
use gpui::{Action, Context, SharedString, Window};

use super::{Category, Handler, Kind, PaletteEntry, RECENT_LIMIT, dispatch};
use crate::model::AppModel;
use crate::views::workspace::{
    CheckForUpdates, ClosePane, CloseTab, DecreaseFontSize, DetachTerminal, EndAllSessionsAndQuit,
    EqualizeSplits, Find, FindNext, FindPrevious, FocusCommitMessage, FocusPaneDown, FocusPaneLeft,
    FocusPaneRight, FocusPaneUp, IncreaseFontSize, InstallCommandLineTool, Minimize, NewBrowserTab,
    NewHomeTab, NewTab, NewWorktree, NextProject, NextPrompt, NextTab, OpenSettings,
    PreviousProject, PreviousPrompt, PreviousTab, Quit, ScrollToBottom, SelectCommandOutput,
    SplitDown, SplitRight, SwitchWorktree, ToggleAgentSessions, ToggleAgentsPanel, ToggleFiles,
    ToggleFullScreen, ToggleSidebar, ToggleSourceControl, ToggleVoiceRecording, ToggleZoomPane,
    Zoom,
};

const COMMAND_LIMIT: usize = 20;
const HIT_LIMIT: usize = 5;
const KEYWORD_PENALTY: i64 = 50;

/// One command on the root page.
#[derive(Clone)]
#[must_use]
pub(crate) struct RootCommand {
    pub(crate) id: SharedString,
    pub(crate) title: SharedString,
    /// Words the command is also found by, without highlights.
    pub(crate) keywords: String,
    pub(crate) category: Category,
    /// A key chord such as `cmd-shift-p`.
    pub(crate) shortcut: Option<SharedString>,
    pub(crate) disabled: bool,
    pub(crate) target: Target<Handler>,
}

impl RootCommand {
    pub(crate) fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        category: Category,
        target: Target<Handler>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            keywords: String::new(),
            category,
            shortcut: None,
            disabled: false,
            target,
        }
    }

    pub(crate) fn run(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        category: Category,
        handler: Handler,
    ) -> Self {
        Self::new(id, title, category, Target::Run(handler))
    }

    pub(crate) fn page(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        category: Category,
        page: PaletteEntry,
    ) -> Self {
        Self::new(id, title, category, Target::OpenPage(page.id()))
    }

    pub(crate) fn keywords(mut self, keywords: impl Into<String>) -> Self {
        self.keywords = keywords.into();
        self
    }

    pub(crate) fn shortcut(mut self, shortcut: Option<SharedString>) -> Self {
        self.shortcut = shortcut;
        self
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// The row, whose `Run` handler first records the command as recent.
    fn item(&self, highlights: Vec<Range<usize>>) -> Item<Handler> {
        let target = match &self.target {
            Target::Run(handler) => {
                let id = self.id.clone();
                let handler = handler.clone();
                let recorded: Handler = Rc::new(move |model, window, cx| {
                    model.note_palette_recent(&id, cx);
                    handler(model, window, cx);
                });
                Target::Run(recorded)
            }
            target => target.clone(),
        };
        let item = Item::new(self.id.clone(), self.title.clone(), target)
            .highlights(highlights)
            .kind(Kind::Command.label())
            .tile(self.category.tile())
            .disabled(self.disabled);
        match &self.shortcut {
            Some(shortcut) => item.shortcut(shortcut.clone()),
            None => item,
        }
    }
}

/// Every root command, in registration order within each category.
#[derive(Clone, Default)]
pub(crate) struct Catalog {
    commands: Vec<RootCommand>,
}

impl Catalog {
    /// Adds `command`, replacing one with the same id.
    pub(crate) fn register(&mut self, command: RootCommand) {
        match self
            .commands
            .iter_mut()
            .find(|existing| existing.id == command.id)
        {
            Some(existing) => *existing = command,
            None => self.commands.push(command),
        }
    }

    pub(crate) fn command(&self, id: &str) -> Option<&RootCommand> {
        self.commands.iter().find(|command| command.id == id)
    }
}

/// The chord bound to `id`, as the palette's keycaps read it.
fn chord(model: &AppModel, id: ShortcutId) -> Option<SharedString> {
    model
        .settings
        .keymap
        .chord(id)
        .map(|chord| SharedString::from(chord.as_str().to_owned()))
}

/// A root command that runs a shortcut's action where the focus is.
pub(crate) fn action(
    model: &AppModel,
    id: ShortcutId,
    title: &'static str,
    action: impl Action,
    category: Category,
) -> RootCommand {
    titled(model, id, title, action, category)
}

/// A root command for a shortcut's action under a computed title.
pub(crate) fn titled(
    model: &AppModel,
    id: ShortcutId,
    title: impl Into<SharedString>,
    action: impl Action,
    category: Category,
) -> RootCommand {
    RootCommand::run(id.name(), title, category, dispatch(action)).shortcut(chord(model, id))
}

/// A root command that opens a page, under the id and chord of the shortcut
/// that opens the same page directly.
fn page(
    model: &AppModel,
    id: ShortcutId,
    title: &'static str,
    entry: PaletteEntry,
    category: Category,
) -> RootCommand {
    RootCommand::page(id.name(), title, category, entry).shortcut(chord(model, id))
}

#[allow(clippy::too_many_lines, reason = "One list of terminal commands")]
fn terminal_commands(catalog: &mut Catalog, model: &AppModel) {
    let no_terminal = model
        .active_pane()
        .and_then(|pane| model.terminal(&pane))
        .is_none();
    let current = model.state.current_project().id;
    let category = Category::Terminal;
    catalog.register(action(
        model,
        ShortcutId::NewTab,
        "New Terminal",
        NewTab,
        category,
    ));
    catalog.register(action(
        model,
        ShortcutId::NewHomeTab,
        "New Home Terminal",
        NewHomeTab,
        category,
    ));
    catalog.register(page(
        model,
        ShortcutId::NewTabInProject,
        "New Terminal in Project…",
        PaletteEntry::NewTerminal,
        category,
    ));
    catalog.register(page(
        model,
        ShortcutId::ExistingTerminals,
        "Existing Terminals…",
        PaletteEntry::Terminals(current),
        category,
    ));
    catalog.register(
        action(model, ShortcutId::CloseTab, "Close Tab", CloseTab, category)
            .disabled(model.active_tab().is_none()),
    );
    let single_tab = model.navigation_targets().len() < 2;
    catalog.register(
        action(model, ShortcutId::NextTab, "Next Tab", NextTab, category).disabled(single_tab),
    );
    catalog.register(
        action(
            model,
            ShortcutId::PreviousTab,
            "Previous Tab",
            PreviousTab,
            category,
        )
        .disabled(single_tab),
    );
    for command in [
        action(model, ShortcutId::Find, "Find…", Find, category),
        action(model, ShortcutId::FindNext, "Find Next", FindNext, category),
        action(
            model,
            ShortcutId::FindPrevious,
            "Find Previous",
            FindPrevious,
            category,
        ),
        action(
            model,
            ShortcutId::PreviousPrompt,
            "Previous Prompt",
            PreviousPrompt,
            category,
        ),
        action(
            model,
            ShortcutId::NextPrompt,
            "Next Prompt",
            NextPrompt,
            category,
        ),
        action(
            model,
            ShortcutId::SelectCommandOutput,
            "Select Command Output",
            SelectCommandOutput,
            category,
        ),
        action(
            model,
            ShortcutId::ScrollToBottom,
            "Scroll to Bottom",
            ScrollToBottom,
            category,
        ),
        action(
            model,
            ShortcutId::IncreaseFontSize,
            "Increase Font Size",
            IncreaseFontSize,
            category,
        ),
        action(
            model,
            ShortcutId::DecreaseFontSize,
            "Decrease Font Size",
            DecreaseFontSize,
            category,
        ),
        action(
            model,
            ShortcutId::DetachTerminal,
            "Detach Terminal",
            DetachTerminal,
            category,
        ),
    ] {
        catalog.register(command.disabled(no_terminal));
    }
}

fn pane_commands(catalog: &mut Catalog, model: &AppModel) {
    let missing = model.state.current_project().status() == codesh_app_core::ProjectStatus::Missing;
    let no_pane = missing || model.active_pane().is_none();
    let unsplit = missing
        || model
            .active_tab()
            .and_then(|tab| model.tab(tab))
            .is_none_or(|tab| tab.panes.len() < 2);
    let category = Category::Panes;
    for command in [
        action(
            model,
            ShortcutId::SplitRight,
            "Split Right",
            SplitRight,
            category,
        ),
        action(
            model,
            ShortcutId::SplitDown,
            "Split Down",
            SplitDown,
            category,
        ),
        action(
            model,
            ShortcutId::ClosePane,
            "Close Pane",
            ClosePane,
            category,
        ),
        action(
            model,
            ShortcutId::ToggleZoomPane,
            "Toggle Pane Zoom",
            ToggleZoomPane,
            category,
        ),
    ] {
        catalog.register(command.disabled(no_pane));
    }
    for command in [
        action(
            model,
            ShortcutId::EqualizeSplits,
            "Equalize Splits",
            EqualizeSplits,
            category,
        ),
        action(
            model,
            ShortcutId::FocusPaneLeft,
            "Focus Pane Left",
            FocusPaneLeft,
            category,
        ),
        action(
            model,
            ShortcutId::FocusPaneRight,
            "Focus Pane Right",
            FocusPaneRight,
            category,
        ),
        action(
            model,
            ShortcutId::FocusPaneUp,
            "Focus Pane Up",
            FocusPaneUp,
            category,
        ),
        action(
            model,
            ShortcutId::FocusPaneDown,
            "Focus Pane Down",
            FocusPaneDown,
            category,
        ),
    ] {
        catalog.register(command.disabled(unsplit));
    }
    catalog.register(
        action(
            model,
            ShortcutId::NewBrowserTab,
            "New Browser Pane",
            NewBrowserTab,
            category,
        )
        .disabled(missing || !model.browser_enabled()),
    );
}

fn file_commands(catalog: &mut Catalog, model: &AppModel) {
    let category = Category::Files;
    catalog.register(
        page(
            model,
            ShortcutId::GoToFile,
            "Go to File…",
            PaletteEntry::Files,
            category,
        )
        .keywords("quick open file"),
    );
    catalog.register(
        page(
            model,
            ShortcutId::FindInFiles,
            "Find in Files…",
            PaletteEntry::Find,
            category,
        )
        .keywords("search grep"),
    );
    catalog.register(
        action(
            model,
            ShortcutId::ToggleFiles,
            "Toggle Files",
            ToggleFiles,
            category,
        )
        .keywords("explorer tree folder"),
    );
}

fn git_commands(catalog: &mut Catalog, model: &AppModel) {
    let category = Category::Git;
    let project = model.state.current_project().id;
    let not_a_repository = model.not_a_repository(project);
    catalog.register(
        RootCommand::page(
            "switch_branch",
            "Switch Branch…",
            category,
            PaletteEntry::Branches(project),
        )
        .keywords("git checkout")
        .disabled(not_a_repository),
    );
    catalog.register(
        RootCommand::page(
            "git_changes",
            "Changes…",
            category,
            PaletteEntry::Changes(project),
        )
        .keywords("git status stage diff")
        .disabled(not_a_repository),
    );
    let new_branch: Handler = Rc::new(move |model, window, cx| {
        model.open_git_form(project, false, cx);
        model.focus_git_form(window, cx);
    });
    catalog.register(
        RootCommand::run("new_branch", "New Branch…", category, new_branch)
            .keywords("git create")
            .disabled(not_a_repository),
    );
    catalog.register(
        RootCommand::run("commit", "Commit…", category, dispatch(FocusCommitMessage))
            .keywords("git message"),
    );
    catalog.register(
        action(
            model,
            ShortcutId::ToggleSourceControl,
            "Toggle Source Control",
            ToggleSourceControl,
            category,
        )
        .keywords("git changes stage"),
    );
}

fn worktree_commands(catalog: &mut Catalog, model: &AppModel) {
    let category = Category::Worktrees;
    catalog.register(
        action(
            model,
            ShortcutId::NewWorktree,
            "New Worktree…",
            NewWorktree,
            category,
        )
        .disabled(model.new_worktree_target().is_none()),
    );
    let switch = match model.worktree_switcher_project() {
        Some(parent) => page(
            model,
            ShortcutId::SwitchWorktree,
            "Switch Worktree…",
            PaletteEntry::Worktrees(parent),
            category,
        ),
        None => action(
            model,
            ShortcutId::SwitchWorktree,
            "Switch Worktree…",
            SwitchWorktree,
            category,
        )
        .disabled(true),
    };
    catalog.register(switch);
}

fn project_commands(catalog: &mut Catalog, model: &AppModel) {
    let category = Category::Projects;
    let single_project = model.navigation_projects().len() < 2;
    catalog.register(
        action(
            model,
            ShortcutId::NextProject,
            "Next Project",
            NextProject,
            category,
        )
        .disabled(single_project),
    );
    catalog.register(
        action(
            model,
            ShortcutId::PreviousProject,
            "Previous Project",
            PreviousProject,
            category,
        )
        .disabled(single_project),
    );
    catalog.register(RootCommand::page(
        "switch_project",
        "Switch Project…",
        category,
        PaletteEntry::Projects,
    ));
    if !model.state.workspaces().is_empty() || model.remote_servers_offered() {
        catalog.register(RootCommand::page(
            "switch_workspace",
            "Switch Workspace…",
            category,
            PaletteEntry::Workspaces,
        ));
    }
    catalog.register(
        page(
            model,
            ShortcutId::AddProject,
            "Open Project…",
            PaletteEntry::AddProject,
            category,
        )
        .keywords("add project folder"),
    );
    for server in &model.settings.servers {
        catalog.register(
            RootCommand::page(
                format!("add_project_on:{}", server.id),
                format!("Add Project on {}…", server.name),
                category,
                PaletteEntry::AddProjectRemote(server.id),
            )
            .keywords(server.destination().to_owned()),
        );
    }
}

fn view_commands(catalog: &mut Catalog, model: &AppModel) {
    let category = Category::View;
    for command in [
        action(
            model,
            ShortcutId::ToggleSidebar,
            "Toggle Sidebar",
            ToggleSidebar,
            category,
        ),
        action(
            model,
            ShortcutId::ToggleAgentsPanel,
            "Toggle Agents Panel",
            ToggleAgentsPanel,
            category,
        ),
        action(
            model,
            ShortcutId::ToggleAgentSessions,
            "Toggle Agent Sessions",
            ToggleAgentSessions,
            category,
        )
        .keywords("harness history resume providers"),
        page(
            model,
            ShortcutId::ToggleThemePicker,
            "Change Theme…",
            PaletteEntry::Themes,
            category,
        )
        .keywords("appearance colors"),
    ] {
        catalog.register(command);
    }
    if model.capabilities.has_backend(Feature::Voice) {
        catalog.register(action(
            model,
            ShortcutId::ToggleVoiceRecording,
            "Toggle Voice Recording",
            ToggleVoiceRecording,
            category,
        ));
    }
}

fn window_commands(catalog: &mut Catalog, model: &AppModel) {
    let category = Category::Window;
    for command in [
        action(
            model,
            ShortcutId::ToggleFullScreen,
            "Toggle Full Screen",
            ToggleFullScreen,
            category,
        ),
        action(
            model,
            ShortcutId::Minimize,
            "Minimize Window",
            Minimize,
            category,
        ),
        action(model, ShortcutId::Zoom, "Zoom Window", Zoom, category).keywords("Maximize Restore"),
        titled(
            model,
            ShortcutId::Quit,
            format!("Quit {}", crate::app_name()),
            Quit,
            category,
        )
        .keywords("Quit exit"),
        action(
            model,
            ShortcutId::EndAllSessionsAndQuit,
            "End All Sessions and Quit",
            EndAllSessionsAndQuit,
            category,
        ),
    ] {
        catalog.register(command);
    }
    if model.capabilities.has_backend(Feature::Updates) {
        catalog.register(RootCommand::run(
            "check_for_updates",
            "Check for Updates…",
            category,
            dispatch(CheckForUpdates),
        ));
    }
    if model.capabilities.has_backend(Feature::CommandLineTool) {
        catalog.register(action(
            model,
            ShortcutId::InstallCommandLineTool,
            "Install Command Line Tool…",
            InstallCommandLineTool,
            category,
        ));
    }
}

impl AppModel {
    /// Every command the root page offers now.
    pub(crate) fn root_catalog(&self) -> Catalog {
        let mut catalog = Catalog::default();
        terminal_commands(&mut catalog, self);
        pane_commands(&mut catalog, self);
        file_commands(&mut catalog, self);
        git_commands(&mut catalog, self);
        worktree_commands(&mut catalog, self);
        project_commands(&mut catalog, self);
        view_commands(&mut catalog, self);
        window_commands(&mut catalog, self);
        catalog.register(RootCommand::page(
            "remote_servers",
            "Remote Servers…",
            Category::Remote,
            PaletteEntry::Servers,
        ));
        catalog.register(action(
            self,
            ShortcutId::OpenSettings,
            "Open Settings",
            OpenSettings,
            Category::Settings,
        ));
        for command in self.custom_commands() {
            catalog.register(command);
        }
        catalog
    }

    pub(super) fn root_items(
        &mut self,
        query: &str,
        cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        let catalog = self.root_catalog();
        let query = query.trim();
        if query.is_empty() {
            return root_sections(&catalog, &self.appearance.palette_recent);
        }
        let chars = fuzzy::normalize(query);
        let mut sections = Vec::new();
        let commands = ranked_commands(&catalog, &chars);
        if !commands.is_empty() {
            sections.push(Section::new("Commands", commands));
        }
        let current = self.state.current_project().id;
        if chars.len() >= 2 {
            let (items, more) = self.files_hits(&chars, HIT_LIMIT, cx);
            push_hits(
                &mut sections,
                "Files",
                items,
                more,
                PaletteEntry::Files,
                query,
            );
        }
        let (items, more) = self.branches_hits(&chars, HIT_LIMIT, cx);
        let branches = PaletteEntry::Branches(current);
        push_hits(&mut sections, "Branches", items, more, branches, query);
        if let Some(parent) = self.worktree_switcher_project() {
            let (items, more) = self.worktrees_hits(&chars, HIT_LIMIT, cx);
            let worktrees = PaletteEntry::Worktrees(parent);
            push_hits(&mut sections, "Worktrees", items, more, worktrees, query);
        }
        let (items, more) = self.projects_hits(&chars, HIT_LIMIT, cx);
        push_hits(
            &mut sections,
            "Projects",
            items,
            more,
            PaletteEntry::Projects,
            query,
        );
        let (items, more) = self.terminals_hits(&chars, HIT_LIMIT, cx);
        let terminals = PaletteEntry::Terminals(current);
        push_hits(&mut sections, "Terminals", items, more, terminals, query);
        if sections.is_empty() {
            return PageContent::empty(format!("No results for “{query}”"));
        }
        PageContent::sections(sections)
    }

    /// Asks for what the root search's providers list, when it is not known.
    pub(super) fn root_opened(&mut self, cx: &mut Context<Self>) {
        let current = self.state.current_project();
        let (project, home) = (current.id, current.home);
        if !home {
            self.load_file_list(project, cx);
        }
        if self
            .git
            .projects
            .get(&project)
            .is_some_and(|repository| repository.summary.is_some())
        {
            self.queue_git_refresh(project, vec![GitAction::Branches], cx);
        }
        self.refresh_existing_sessions(cx);
    }

    pub(crate) fn refresh_root_page(&mut self, cx: &mut Context<Self>) {
        if self.palette_page(cx) == Some(PaletteEntry::Root) {
            self.refresh_palette(cx);
        }
    }
}

pub(super) fn root_spec() -> PageSpec {
    PageSpec::new(
        PaletteEntry::Root.id(),
        "Search for commands, files, branches…",
    )
}

/// The empty query: recent commands, then every category.
fn root_sections(catalog: &Catalog, recent: &[String]) -> PageContent<Handler> {
    let suggestions: Vec<&RootCommand> = recent
        .iter()
        .filter_map(|id| catalog.command(id))
        .filter(|command| !command.disabled)
        .take(RECENT_LIMIT)
        .collect();
    let mut sections = Vec::new();
    if !suggestions.is_empty() {
        sections.push(Section::new(
            "Suggestions",
            suggestions
                .iter()
                .map(|command| command.item(Vec::new()))
                .collect(),
        ));
    }
    for category in Category::ALL {
        let items: Vec<_> = catalog
            .commands
            .iter()
            .filter(|command| {
                command.category == category
                    && !suggestions
                        .iter()
                        .any(|suggested| suggested.id == command.id)
            })
            .map(|command| command.item(Vec::new()))
            .collect();
        if !items.is_empty() {
            sections.push(Section::new(category.title(), items));
        }
    }
    PageContent::sections(sections)
}

/// Commands whose title matches best, then those only their keywords match.
/// Ties go to the earlier category, then to the shorter title.
fn ranked_commands(catalog: &Catalog, query: &[char]) -> Vec<Item<Handler>> {
    let mut ranked: Vec<_> = catalog
        .commands
        .iter()
        .filter_map(|command| {
            if let Some(found) = fuzzy::fuzzy(&command.title, query) {
                return Some((found.score, command, found.ranges));
            }
            fuzzy::fuzzy(&command.keywords, query)
                .filter(|_| !command.keywords.is_empty())
                .map(|found| (found.score - KEYWORD_PENALTY, command, Vec::new()))
        })
        .collect();
    ranked.sort_by(|(left_score, left, _), (right_score, right, _)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.category.order().cmp(&right.category.order()))
            .then_with(|| left.title.len().cmp(&right.title.len()))
    });
    ranked
        .into_iter()
        .take(COMMAND_LIMIT)
        .map(|(_, command, ranges)| command.item(ranges))
        .collect()
}

impl AppModel {
    /// Runs a ⌘K action on a root search hit through the page that found it.
    pub(super) fn root_action(
        &mut self,
        item: &str,
        action: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((tag, raw_id)) = split_hit_id(item) else {
            return;
        };
        let current = self.state.current_project().id;
        let entry = match tag {
            "files" => PaletteEntry::Files,
            "branches" => PaletteEntry::Branches(current),
            "worktrees" => match self.worktree_switcher_project() {
                Some(parent) => PaletteEntry::Worktrees(parent),
                None => return,
            },
            "projects" => PaletteEntry::Projects,
            "terminals" => PaletteEntry::Terminals(current),
            _ => return,
        };
        self.page_action(entry, raw_id, action, window, cx);
    }
}

/// A provider's section, with a row that opens its page on the same query
/// when it found more than it shows.
fn push_hits(
    sections: &mut Vec<Section<Handler>>,
    title: &'static str,
    mut items: Vec<Item<Handler>>,
    more: bool,
    entry: PaletteEntry,
    query: &str,
) {
    if items.is_empty() {
        return;
    }
    for item in &mut items {
        item.id = hit_id(entry, &item.id).into();
    }
    if more {
        let query = query.to_owned();
        let open: Handler = Rc::new(move |model, _, cx| {
            model.push_palette_page(entry, Some(&query), cx);
        });
        items.push(Item::apply(
            format!("see-all:{}", entry.id()),
            format!("See all in {title}"),
            open,
        ));
    }
    sections.push(Section::new(title, items));
}

/// A root search row's id, namespaced by the provider page that found it.
fn hit_id(entry: PaletteEntry, id: &str) -> String {
    let tag = match entry {
        PaletteEntry::Files => "files",
        PaletteEntry::Branches(_) => "branches",
        PaletteEntry::Worktrees(_) => "worktrees",
        PaletteEntry::Projects => "projects",
        PaletteEntry::Terminals(_) => "terminals",
        _ => return id.to_owned(),
    };
    format!("{tag}/{id}")
}

fn split_hit_id(id: &str) -> Option<(&str, &str)> {
    let (tag, raw) = id.split_once('/')?;
    matches!(
        tag,
        "files" | "branches" | "worktrees" | "projects" | "terminals"
    )
    .then_some((tag, raw))
}
