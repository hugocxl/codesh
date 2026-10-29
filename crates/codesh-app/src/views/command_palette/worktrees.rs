//! Switch Worktree: a project and its worktrees, by recent use when the
//! sidebar orders them so.

#![expect(clippy::unused_self, reason = "A stub until its page is implemented")]

use codesh_app_core::{Project, ProjectId, ProjectStatus};
use codesh_ui::command_palette::{Item, PageContent, PageSpec};
use gpui::{App, Context, Window, point, px};

use super::{Handler, PaletteEntry};
use crate::model::AppModel;

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

    #[expect(dead_code, reason = "Switch Worktree's page orders its rows with it")]
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

    #[expect(dead_code, reason = "Switch Worktree's page runs its rows with it")]
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
        if action.is_none() && !available {
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

    pub(super) fn worktrees_spec(&self, parent: ProjectId, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Worktrees(parent).id(), "Search worktrees…")
            .breadcrumb("Switch Worktree")
    }

    pub(super) fn worktrees_items(
        &mut self,
        _parent: ProjectId,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
    }

    pub(super) fn worktrees_opened(&mut self, _parent: ProjectId, _cx: &mut Context<Self>) {}

    pub(super) fn worktrees_action(
        &mut self,
        _parent: ProjectId,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    /// Keeps an open Switch Worktree page in step with its projects and
    /// their activity.
    pub(crate) fn refresh_worktrees_page(&mut self, cx: &mut Context<Self>) {
        if matches!(self.palette_page(cx), Some(PaletteEntry::Worktrees(_))) {
            self.refresh_palette(cx);
        }
    }

    /// Root search rows: the current project's best worktrees, and whether
    /// there are more.
    pub(super) fn worktrees_hits(
        &self,
        _query: &[char],
        _limit: usize,
        _cx: &App,
    ) -> (Vec<Item<Handler>>, bool) {
        (Vec::new(), false)
    }
}
