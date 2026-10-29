//! Switch Project, Switch Workspace, and New Terminal in Project.

#![expect(
    clippy::unused_self,
    reason = "Every page function takes the model, used or not"
)]

use std::rc::Rc;

use codesh_app_core::{Project, ProjectStatus, ServerId};
use codesh_ui::command_palette::{Item, ItemAction, PageContent, PageSpec, Section};
use codesh_ui::fuzzy;
use gpui::{App, Context, Window};

use super::{Handler, Kind, PaletteEntry};
use crate::model::AppModel;

impl AppModel {
    /// Opens New Terminal in Project, or closes it when it is open.
    pub(crate) fn toggle_new_tab_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(PaletteEntry::NewTerminal, window, cx);
    }

    pub(super) fn projects_spec(&self, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Projects.id(), "Search projects…").breadcrumb("Switch Project")
    }

    pub(super) fn projects_items(
        &mut self,
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::items(filter_items(self.project_switch_items(), query, usize::MAX))
    }

    pub(super) fn projects_opened(&mut self, _cx: &mut Context<Self>) {}

    pub(super) fn projects_action(
        &mut self,
        item: &str,
        action: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if action == "open" {
            let items = self.project_switch_items();
            run_item(self, items, item, window, cx);
        }
    }

    /// Root search rows: the best projects, and whether there are more.
    pub(super) fn projects_hits(
        &self,
        query: &[char],
        limit: usize,
        _cx: &App,
    ) -> (Vec<Item<Handler>>, bool) {
        let query: String = query.iter().collect();
        let mut items = filter_items(self.project_switch_items(), &query, limit.saturating_add(1));
        let more = items.len() > limit;
        items.truncate(limit);
        (items, more)
    }

    fn project_switch_items(&self) -> Vec<Item<Handler>> {
        let mut parents: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| project.parent_id.is_none() && self.project_listed(project))
            .collect();
        self.sort_projects(&mut parents);
        let by_server = self.state.remote_servers_selected()
            || self.appearance.sidebar_project_order
                != codesh_app_core::settings::ProjectOrder::Manual;
        parents.sort_by_cached_key(|project| {
            let server = self
                .settings
                .servers
                .iter()
                .position(|entry| entry.id == project.server_id)
                .map_or(0, |index| index + 1);
            (
                !project.server_id.is_local(),
                if by_server { server } else { 0 },
            )
        });
        parents
            .into_iter()
            .flat_map(|parent| {
                std::iter::once(parent).chain(self.state.projects().iter().filter(move |project| {
                    project.parent_id == Some(parent.id) && self.project_listed(project)
                }))
            })
            .map(|project| {
                let id = project.id;
                let title = project
                    .parent_id
                    .and_then(|parent| self.state.project(parent))
                    .map_or_else(
                        || self.project_title(project),
                        |parent| format!("{} / {}", parent.name, project.name),
                    );
                let handler: Handler = Rc::new(move |model, _, cx| model.select_project(id, cx));
                project_item(project, title, handler)
                    .current(id == self.state.current_project().id)
                    .actions(vec![ItemAction::new("open", "Switch Project")])
            })
            .collect()
    }

    pub(super) fn workspaces_spec(&self, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Workspaces.id(), "Search workspaces…")
            .breadcrumb("Switch Workspace")
    }

    pub(super) fn workspaces_items(
        &mut self,
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::items(filter_items(
            self.workspace_switch_items(),
            query,
            usize::MAX,
        ))
    }

    pub(super) fn workspaces_opened(&mut self, _cx: &mut Context<Self>) {}

    pub(super) fn workspaces_action(
        &mut self,
        item: &str,
        action: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if action == "open" {
            let items = self.workspace_switch_items();
            run_item(self, items, item, window, cx);
        }
    }

    fn workspace_switch_items(&self) -> Vec<Item<Handler>> {
        let remote_selected = self.state.remote_servers_selected();
        let active = self.state.active_workspace().map(|workspace| workspace.id);
        let all = std::iter::once((None, "All Projects".to_owned()));
        let named = self
            .state
            .workspaces()
            .iter()
            .map(|workspace| (Some(workspace.id), workspace.name.clone()));
        let mut items = Vec::new();
        for (id, title) in all.chain(named) {
            let handler: Handler = Rc::new(move |model, _, cx| model.select_workspace(id, cx));
            items.push(
                workspace_item(
                    id.map_or_else(|| "all".to_owned(), |id| id.to_string()),
                    title,
                    handler,
                )
                .current(!remote_selected && active == id),
            );
            if id.is_none() && self.remote_servers_offered() {
                let handler: Handler = Rc::new(|model, _, cx| model.select_remote_servers(cx));
                items.push(
                    workspace_item(
                        "remote_servers".to_owned(),
                        "Remote Servers".to_owned(),
                        handler,
                    )
                    .current(remote_selected),
                );
            }
        }
        items
    }

    pub(super) fn new_terminal_spec(&self, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::NewTerminal.id(), "Search projects…")
            .breadcrumb("New Terminal in Project")
    }

    pub(super) fn new_terminal_items(
        &mut self,
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::sections(
            self.new_terminal_sections()
                .into_iter()
                .map(|section| Section {
                    title: section.title,
                    items: filter_items(section.items, query, usize::MAX),
                })
                .collect(),
        )
    }

    pub(super) fn new_terminal_opened(&mut self, _cx: &mut Context<Self>) {}

    pub(super) fn new_terminal_action(
        &mut self,
        item: &str,
        action: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if action == "open" {
            let items = self
                .new_terminal_sections()
                .into_iter()
                .flat_map(|section| section.items)
                .collect();
            run_item(self, items, item, window, cx);
        }
    }

    fn new_terminal_sections(&self) -> Vec<Section<Handler>> {
        let local = terminal_projects(self, ServerId::local())
            .into_iter()
            .map(|project| terminal_item(project, self.project_title(project)))
            .collect();
        let mut remote = Vec::new();
        for entry in &self.settings.servers {
            let server = entry.id;
            let home: Handler = Rc::new(move |model, _, cx| model.new_remote_home_tab(server, cx));
            remote.push(
                Item::run(format!("server-{server}"), entry.name.clone(), home)
                    .subtitle(entry.destination().to_owned())
                    .kind(Kind::Terminal.label())
                    .tile(Kind::Terminal.tile())
                    .actions(vec![ItemAction::new("open", "New Terminal")]),
            );
            remote.extend(
                terminal_projects(self, server)
                    .into_iter()
                    .filter(|project| !project.home)
                    .map(|project| {
                        terminal_item(project, format!("{} · {}", entry.name, project.name))
                    }),
            );
        }
        vec![Section::new("Local", local), Section::new("Remote", remote)]
    }

    /// Shows new projects or workspaces on an open project page.
    pub(crate) fn refresh_projects_page(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.palette_page(cx),
            Some(PaletteEntry::Projects | PaletteEntry::Workspaces | PaletteEntry::NewTerminal)
        ) {
            self.refresh_palette(cx);
        }
    }
}

fn project_item(project: &Project, title: String, handler: Handler) -> Item<Handler> {
    Item::run(project.id.to_string(), title, handler)
        .subtitle(project.directory.to_string_lossy().into_owned())
        .kind(Kind::Project.label())
        .tile(Kind::Project.tile())
        .disabled(project.status() == ProjectStatus::Missing)
}

fn workspace_item(id: String, title: String, handler: Handler) -> Item<Handler> {
    Item::run(id, title, handler)
        .kind(Kind::Workspace.label())
        .tile(Kind::Workspace.tile())
        .actions(vec![ItemAction::new("open", "Switch Workspace")])
}

fn terminal_projects(model: &AppModel, server: ServerId) -> Vec<&Project> {
    let mut projects: Vec<_> = model
        .state
        .projects()
        .iter()
        .filter(|project| {
            project.server_id == server
                && project.parent_id.is_none()
                && model.project_shown(project)
        })
        .collect();
    model.sort_projects(&mut projects);
    projects
}

fn terminal_item(project: &Project, title: String) -> Item<Handler> {
    let id = project.id;
    let handler: Handler = Rc::new(move |model, _, cx| model.new_tab_in(id, cx));
    project_item(project, title, handler).actions(vec![ItemAction::new("open", "New Terminal")])
}

fn filter_items(items: Vec<Item<Handler>>, query: &str, limit: usize) -> Vec<Item<Handler>> {
    if query.is_empty() {
        return items.into_iter().take(limit).collect();
    }
    let normalized = fuzzy::normalize(query);
    let candidates = items.into_iter().map(|item| {
        let title_matches = fuzzy::fuzzy(&item.title, &normalized).is_some();
        (item, title_matches)
    });
    fuzzy::rank(
        candidates,
        |(item, title_matches)| {
            if *title_matches {
                item.title.as_ref()
            } else {
                item.subtitle
                    .as_ref()
                    .map_or("", |subtitle| subtitle.as_ref())
            }
        },
        query,
        limit,
    )
    .into_iter()
    .map(|ranked| {
        let (item, title_matches) = ranked.item;
        if title_matches {
            item.highlights(ranked.ranges)
        } else {
            item
        }
    })
    .collect()
}

fn run_item(
    model: &mut AppModel,
    items: Vec<Item<Handler>>,
    id: &str,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) {
    let Some(item) = items
        .into_iter()
        .find(|item| item.id == id && !item.disabled)
    else {
        return;
    };
    let Some(handler) = item.target.payload() else {
        return;
    };
    model.dismiss_overlay(cx);
    handler(model, window, cx);
}
