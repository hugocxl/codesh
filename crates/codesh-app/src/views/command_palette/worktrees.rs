//! Switch Worktree: a project and its worktrees, by recent use when the
//! sidebar orders them so.

use std::rc::Rc;

use codesh_app_core::{Project, ProjectId, ProjectStatus};
use codesh_protocol::ProgressState;
use codesh_ui::command_palette::{Item, ItemAction, PageContent, PageSpec, Section};
use codesh_ui::fuzzy;
use gpui::{App, Context, Window, point, px};

use super::{Handler, Kind, PaletteEntry};
use crate::model::AppModel;
use crate::views::tab_activity::{self, Status};

const NEW_WORKTREE: &str = "new-worktree";
const OPEN: &str = "open";
const RENAME: &str = "rename";
const REMOVE: &str = "remove";

impl AppModel {
    pub(crate) fn worktree_switcher_project(&self) -> Option<ProjectId> {
        let current = self.state.current_project();
        let project = self
            .state
            .project(current.parent_id.unwrap_or(current.id))?;
        (!project.home
            && project.status() == ProjectStatus::Available
            && self.worktrees_visible(project.id))
        .then_some(project.id)
    }

    /// Opens Switch Worktree over the current project's worktrees, or closes it.
    pub(crate) fn toggle_worktree_switcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.palette_page(cx), Some(PaletteEntry::Worktrees(_))) {
            self.dismiss_overlay(cx);
            return;
        }
        let Some(project) = self.worktree_switcher_project() else {
            return;
        };
        self.open_palette(PaletteEntry::Worktrees(project), window, cx);
    }

    pub(crate) fn worktree_switcher_order<'a>(
        &'a self,
        project: &'a Project,
    ) -> Vec<(&'a Project, bool)> {
        let mut rows: Vec<_> = std::iter::once((project, true))
            .chain(
                self.worktree_children(project.id)
                    .into_iter()
                    .map(|child| (child, false)),
            )
            .collect();
        if self.appearance.worktree_recent_order {
            let current = self.state.current_project().id;
            let recent = &self.appearance.worktree_recent;
            rows.sort_by_key(|(row, _)| {
                if row.id == current {
                    0
                } else {
                    recent
                        .iter()
                        .position(|id| *id == row.id)
                        .map_or(usize::MAX, |index| index + 1)
                }
            });
        }
        rows
    }

    pub(super) fn choose_worktree(
        &mut self,
        parent: ProjectId,
        row: &str,
        action: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = row
            .parse::<ProjectId>()
            .ok()
            .and_then(|id| self.state.project(id))
            .filter(|project| project.id == parent || project.parent_id == Some(parent))
        else {
            return;
        };
        let id = project.id;
        let available = project.status() == ProjectStatus::Available;
        if self.worktree_removing(id)
            || (action.is_none() && !available)
            || (action.is_some() && project.parent_id != Some(parent))
            || (action == Some(RENAME) && !available)
            || action.is_some_and(|action| !matches!(action, RENAME | REMOVE))
        {
            return;
        }
        self.dismiss_overlay(cx);
        match action {
            None => {
                self.select_project(id, cx);
                self.focus_active(window, cx);
            }
            Some(RENAME) => {
                let size = window.viewport_size();
                let position = point(size.width / 2.0 - px(120.0), size.height / 3.0);
                self.open_project_editor(id, position, window, cx);
            }
            Some(REMOVE) if available => {
                self.git_request(id, codesh_protocol::GitAction::InspectRemoval, cx);
            }
            Some(REMOVE) => self.confirm_remove_project(id, cx),
            Some(_) => {}
        }
    }

    #[expect(
        clippy::unused_self,
        reason = "Every page function takes the model, used or not"
    )]
    pub(super) fn worktrees_spec(&self, parent: ProjectId, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Worktrees(parent).id(), "Search worktrees…")
            .breadcrumb("Switch Worktree")
    }

    pub(super) fn worktrees_items(
        &mut self,
        parent: ProjectId,
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        let Some(project) = self.state.project(parent) else {
            return PageContent::empty("No matching worktrees");
        };
        let query = fuzzy::normalize(query);
        let mut items: Vec<_> = self
            .worktree_switcher_order(project)
            .into_iter()
            .filter(|(project, _)| self.worktree_matches(project, &query))
            .map(|(project, primary)| self.worktree_item(parent, project, primary, &query))
            .collect();
        items.extend(self.worktree_creations(parent).enumerate().filter_map(
            |(index, creation)| {
                let found = fuzzy::fuzzy(&creation.name, &query)?;
                let handler: Handler = Rc::new(|_, _, _| {});
                Some(
                    Item::run(format!("creating-{index}"), creation.name.clone(), handler)
                        .highlights(found.ranges)
                        .kind(Kind::Worktree.label())
                        .tile(Kind::Worktree.tile())
                        .subtitle(tab_activity::CREATING)
                        .disabled(true),
                )
            },
        ));
        let create: Handler = Rc::new(move |model, window, cx| {
            if model.new_worktree_target() != Some(parent) {
                return;
            }
            model.open_git_form(parent, true, cx);
            model.focus_git_form(window, cx);
        });
        PageContent::sections(vec![
            Section::untitled(items),
            Section::new(
                "More",
                vec![
                    Item::run(NEW_WORKTREE, "New Worktree…", create)
                        .kind(Kind::Worktree.label())
                        .tile(Kind::Worktree.tile())
                        .actions(vec![ItemAction::new(OPEN, "New Worktree…")])
                        .disabled(self.new_worktree_target() != Some(parent)),
                ],
            ),
        ])
    }

    fn worktree_matches(&self, project: &Project, query: &[char]) -> bool {
        fuzzy::fuzzy(&project.name, query).is_some()
            || fuzzy::fuzzy(&self.project_path_label(project), query).is_some()
    }

    fn worktree_item(
        &self,
        parent: ProjectId,
        project: &Project,
        primary: bool,
        query: &[char],
    ) -> Item<Handler> {
        let id = project.id.to_string();
        let row = id.clone();
        let handler: Handler = Rc::new(move |model, window, cx| {
            model.choose_worktree(parent, &row, None, window, cx);
        });
        let missing = project.status() != ProjectStatus::Available;
        let removing = self.worktree_removing(project.id);
        let mut actions = vec![ItemAction::new(OPEN, "Switch Worktree")];
        if !primary && !removing {
            if !missing {
                actions.push(ItemAction::new(RENAME, "Rename…"));
            }
            actions.push(ItemAction::new(REMOVE, "Remove…").destructive());
        }
        let status = match tab_activity::worktree_status(project.id, self) {
            Status::Busy(key) => Some(key),
            Status::Blocked => Some("Blocked"),
            Status::Progress(progress) => Some(match progress.state {
                ProgressState::Error => "Work reported an error.",
                ProgressState::Paused => "Work is paused.",
                ProgressState::Running
                | ProgressState::Indeterminate
                | ProgressState::Unrecognized(_) => "Working",
            }),
            Status::Unread(_) | Status::Completed => Some("Finished working"),
            Status::None => None,
        };
        let path = self.project_path_label(project);
        let subtitle = match status {
            Some(status) => format!("{path} · {status}"),
            None => path,
        };
        let item = Item::run(id, project.name.clone(), handler)
            .highlights(
                fuzzy::fuzzy(&project.name, query).map_or_else(Vec::new, |found| found.ranges),
            )
            .subtitle(subtitle)
            .kind(Kind::Worktree.label())
            .tile(Kind::Worktree.tile())
            .current(project.id == self.state.current_project().id)
            .disabled(removing || (missing && primary))
            .actions(actions);
        if primary { item.badge("PRIMARY") } else { item }
    }

    pub(super) fn worktrees_opened(&mut self, parent: ProjectId, cx: &mut Context<Self>) {
        let Some(palette) = self.palette_entity() else {
            return;
        };
        if palette.read(cx).selected_item().is_some() {
            return;
        }
        self.refresh_palette(cx);
        let current = self.state.current_project().id.to_string();
        palette.update(cx, |palette, cx| {
            let _ = palette.select(&PaletteEntry::Worktrees(parent).id(), &current, cx);
        });
    }

    pub(super) fn worktrees_action(
        &mut self,
        parent: ProjectId,
        item: &str,
        action: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if action == OPEN && item == NEW_WORKTREE {
            if self.new_worktree_target() == Some(parent) {
                self.dismiss_overlay(cx);
                self.open_git_form(parent, true, cx);
                self.focus_git_form(window, cx);
            }
            return;
        }
        match action {
            OPEN => self.choose_worktree(parent, item, None, window, cx),
            RENAME | REMOVE => self.choose_worktree(parent, item, Some(action), window, cx),
            _ => {}
        }
    }

    /// Keeps an open Switch Worktree page in step with its projects and
    /// their activity.
    pub(crate) fn refresh_worktrees_page(&mut self, cx: &mut Context<Self>) {
        let Some(PaletteEntry::Worktrees(parent)) = self.palette_page(cx) else {
            return;
        };
        if self.worktree_switcher_project() != Some(parent) {
            self.dismiss_overlay(cx);
            return;
        }
        self.refresh_palette(cx);
    }

    /// Root search rows: the current project's best worktrees, and whether
    /// there are more.
    pub(super) fn worktrees_hits(
        &self,
        query: &[char],
        limit: usize,
        _cx: &App,
    ) -> (Vec<Item<Handler>>, bool) {
        let Some(parent) = self.worktree_switcher_project() else {
            return (Vec::new(), false);
        };
        let Some(project) = self.state.project(parent) else {
            return (Vec::new(), false);
        };
        let candidates = self
            .worktree_switcher_order(project)
            .into_iter()
            .filter(|(project, _)| self.worktree_matches(project, query))
            .map(|(project, primary)| {
                (
                    project,
                    primary,
                    format!("{} {}", project.name, self.project_path_label(project)),
                )
            });
        let query_text: String = query.iter().collect();
        let mut ranked = fuzzy::rank(
            candidates,
            |(_, _, text)| text.as_str(),
            &query_text,
            limit.saturating_add(1),
        );
        let more = ranked.len() > limit;
        ranked.truncate(limit);
        let items = ranked
            .into_iter()
            .map(|ranked| {
                let (project, primary, _) = ranked.item;
                self.worktree_item(parent, project, primary, query)
            })
            .collect();
        (items, more)
    }
}
