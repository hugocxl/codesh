//! Switch Branch and Changes over a project's repository. The Source Control
//! panel keeps its own anchored popovers.

#![expect(
    clippy::unused_self,
    reason = "Page specs follow the shared page contract"
)]

use std::{ops::Range, rc::Rc};

use codesh_app_core::ProjectId;
use codesh_protocol::{GitAction, GitBranch, ServerPath};
use codesh_ui::{
    command_palette::{Item, ItemAction, PageContent, PageSpec, Status},
    fuzzy,
};
use gpui::{App, Context, Window};

use super::{Handler, Kind, PaletteEntry};
use crate::model::{AppModel, git::Repository};

impl AppModel {
    pub(super) fn branches_spec(&self, project: ProjectId, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Branches(project).id(), "Search branches…")
            .breadcrumb("Switch Branch")
    }

    pub(super) fn branches_items(
        &mut self,
        project: ProjectId,
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        let repository = self.git.projects.get(&project);
        let busy = !self.session_listing_ready() || repository.is_some_and(Repository::busy);
        let create: Handler = Rc::new(move |model, window, cx| {
            if !model.palette_git_ready(project) || model.not_a_repository(project) {
                return;
            }
            model.open_git_form(project, false, cx);
            model.focus_git_form(window, cx);
        });
        let mut items = vec![
            Item::run("new-branch", "New Branch…", create)
                .kind(Kind::Command.label())
                .tile(Kind::Branch.tile())
                .disabled(busy || self.not_a_repository(project)),
        ];
        if let Some(repository) = repository {
            items.extend(
                fuzzy::rank(
                    &repository.branches,
                    |branch| &branch.name,
                    query,
                    usize::MAX,
                )
                .into_iter()
                .map(|ranked| branch_item(project, ranked.item, ranked.ranges, busy)),
            );
        }
        let mut content = PageContent::items(items);
        let status = git_status(
            self.session_listing_ready(),
            repository,
            &GitAction::Branches,
            content.sections[0].items.len() > 1,
        );
        content.status = match status {
            Status::Empty(_) => Status::Ready,
            status => status,
        };
        content
    }

    pub(super) fn branches_opened(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.queue_git_refresh(project, vec![GitAction::Summary, GitAction::Branches], cx);
    }

    pub(super) fn branches_action(
        &mut self,
        project: ProjectId,
        item: &str,
        action: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.palette_git_ready(project) {
            return;
        }
        let Some(branch) = self.git.projects.get(&project).and_then(|repository| {
            repository
                .branches
                .iter()
                .find(|branch| branch.name == item)
        }) else {
            return;
        };
        match action {
            "switch" if !branch.current => {
                let action = GitAction::SwitchBranch(branch.name.clone());
                if self.palette_entity().is_some() {
                    self.dismiss_overlay(cx);
                }
                self.git_request(project, action, cx);
            }
            "delete" if !branch.checked_out => {
                let name = branch.name.clone();
                self.confirm_git_action(
                    project,
                    GitAction::DeleteBranch(name.clone()),
                    format!("Permanently delete branch “{name}”? Unmerged commits may become unreachable."),
                    cx,
                );
            }
            _ => {}
        }
    }

    /// Root search rows: the current project's best branches, and whether
    /// there are more.
    pub(super) fn branches_hits(
        &self,
        query: &[char],
        limit: usize,
        _cx: &App,
    ) -> (Vec<Item<Handler>>, bool) {
        let project = self.state.current_project().id;
        let Some(repository) = self.git.projects.get(&project) else {
            return (Vec::new(), false);
        };
        let query: String = query.iter().collect();
        let ranked = fuzzy::rank(
            &repository.branches,
            |branch| &branch.name,
            &query,
            limit.saturating_add(1),
        );
        let more = ranked.len() > limit;
        let busy = !self.session_listing_ready() || repository.busy();
        let items = ranked
            .into_iter()
            .take(limit)
            .map(|ranked| branch_item(project, ranked.item, ranked.ranges, busy))
            .collect();
        (items, more)
    }

    pub(super) fn changes_spec(&self, project: ProjectId, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Changes(project).id(), "Search changes…").breadcrumb("Changes")
    }

    pub(super) fn changes_items(
        &mut self,
        project: ProjectId,
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        let repository = self.git.projects.get(&project);
        let busy = !self.session_listing_ready() || repository.is_some_and(Repository::busy);
        let files: Vec<_> = repository
            .into_iter()
            .flat_map(|repository| &repository.files)
            .map(|file| (file, String::from_utf8_lossy(&file.path.0).into_owned()))
            .collect();
        let items = fuzzy::rank(files, |(_, title)| title, query, usize::MAX)
            .into_iter()
            .map(|ranked| {
                let (file, title) = ranked.item;
                let id = path_key(&file.path);
                let open_id = id.clone();
                let open: Handler = Rc::new(move |model, window, cx| {
                    model.changes_action(project, &open_id, "open", window, cx);
                });
                let mut actions = vec![ItemAction::new("open", "Open Diff")];
                if file.unstaged() || file.untracked() {
                    actions.push(ItemAction::new("stage", "Stage"));
                }
                if file.staged() {
                    actions.push(ItemAction::new("unstage", "Unstage"));
                }
                if !file.conflicted() && (file.unstaged() || file.untracked()) {
                    actions.push(ItemAction::new("discard", "Discard").destructive());
                }
                let subtitle = if file.conflicted() {
                    "Conflict"
                } else if file.untracked() {
                    "Untracked"
                } else if file.staged() && file.unstaged() {
                    "Staged and unstaged"
                } else if file.staged() {
                    "Staged"
                } else {
                    "Unstaged"
                };
                let mut item = Item::run(id, title, open)
                    .highlights(ranked.ranges)
                    .kind(Kind::Change.label())
                    .tile(Kind::Change.tile())
                    .subtitle(subtitle)
                    .disabled(busy)
                    .actions(actions);
                if let Some((added, removed)) = file.added.zip(file.removed) {
                    item = item.badge(format!("+{added} −{removed}"));
                }
                item
            })
            .collect::<Vec<_>>();
        let status = git_status(
            self.session_listing_ready(),
            repository,
            &GitAction::Changes,
            !items.is_empty(),
        );
        let mut content = PageContent::items(items);
        content.status = status;
        content
    }

    pub(super) fn changes_opened(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.queue_git_refresh(project, vec![GitAction::Summary, GitAction::Changes], cx);
    }

    pub(super) fn changes_action(
        &mut self,
        project: ProjectId,
        item: &str,
        action: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.palette_git_ready(project) {
            return;
        }
        let Some(file) = self.git.projects.get(&project).and_then(|repository| {
            repository
                .files
                .iter()
                .find(|file| path_key(&file.path) == item)
        }) else {
            return;
        };
        let paths = vec![file.path.clone()];
        let operation = match action {
            "stage" if file.unstaged() || file.untracked() => GitAction::Stage(paths),
            "unstage" if file.staged() => GitAction::Unstage(paths),
            "discard" if !file.conflicted() && (file.unstaged() || file.untracked()) => {
                GitAction::Discard(paths)
            }
            "open" => {
                let path = String::from_utf8_lossy(&file.path.0).into_owned();
                let staged = file.staged() && !file.unstaged();
                if self.palette_entity().is_some() {
                    self.dismiss_overlay(cx);
                }
                self.open_diff(project, &path, staged, cx);
                return;
            }
            _ => return,
        };
        if let GitAction::Discard(paths) = &operation {
            self.confirm_git_action(
                project,
                operation.clone(),
                format!("Discard changes to “{}”? Untracked files will be permanently deleted; staged changes are preserved.", String::from_utf8_lossy(&paths[0].0)),
                cx,
            );
            return;
        }
        self.git_request(project, operation, cx);
    }

    fn palette_git_ready(&self, project: ProjectId) -> bool {
        self.session_listing_ready()
            && self.state.project(project).is_some()
            && self
                .git
                .projects
                .get(&project)
                .is_some_and(|repository| !repository.busy())
    }

    /// Shows new repository state on an open Switch Branch or Changes page.
    pub(crate) fn refresh_git_pages(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.palette_page(cx),
            Some(PaletteEntry::Branches(_) | PaletteEntry::Changes(_))
        ) {
            self.refresh_palette(cx);
        }
    }

    /// The project and the read an open git page needs refreshed with its
    /// repository.
    pub(crate) fn palette_git_action(&self, cx: &App) -> Option<(ProjectId, GitAction)> {
        match self.palette_page(cx)? {
            PaletteEntry::Branches(project) => Some((project, GitAction::Branches)),
            PaletteEntry::Changes(project) => Some((project, GitAction::Changes)),
            _ => None,
        }
    }
}

fn branch_item(
    project: ProjectId,
    branch: &GitBranch,
    highlights: Vec<Range<usize>>,
    busy: bool,
) -> Item<Handler> {
    let name = branch.name.clone();
    let switch: Handler = Rc::new(move |model, window, cx| {
        model.branches_action(project, &name, "switch", window, cx);
    });
    let mut actions = vec![ItemAction::new("switch", "Switch Branch")];
    if !branch.checked_out {
        actions.push(ItemAction::new("delete", "Delete").destructive());
    }
    let mut item = Item::run(branch.name.clone(), branch.name.clone(), switch)
        .highlights(highlights)
        .kind(Kind::Branch.label())
        .tile(Kind::Branch.tile())
        .current(branch.current)
        .disabled(busy || branch.current)
        .actions(actions);
    if branch.checked_out {
        item = item.subtitle("Checked out");
    }
    item
}

fn git_status(
    connected: bool,
    repository: Option<&Repository>,
    action: &GitAction,
    has_items: bool,
) -> Status {
    if !connected {
        return Status::Error("Reconnect to use Git".into());
    }
    if let Some(error) = repository.and_then(|repository| repository.load_error(action)) {
        return Status::Error(error.clone().into());
    }
    if !has_items && repository.is_none_or(|repository| !repository.has_loaded(action)) {
        return Status::Loading("Loading…".into());
    }
    if !has_items {
        return Status::Empty("No matches".into());
    }
    Status::Ready
}

fn path_key(path: &ServerPath) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    path.0
        .iter()
        .flat_map(|byte| {
            [
                char::from(HEX[usize::from(byte >> 4)]),
                char::from(HEX[usize::from(byte & 15)]),
            ]
        })
        .collect()
}
