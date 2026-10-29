//! Switch Branch and Changes over a project's repository. The Source Control
//! panel keeps its own anchored popovers.

#![expect(clippy::unused_self, reason = "A stub until its page is implemented")]

use codesh_app_core::ProjectId;
use codesh_protocol::GitAction;
use codesh_ui::command_palette::{Item, PageContent, PageSpec};
use gpui::{App, Context, Window};

use super::{Handler, PaletteEntry};
use crate::model::AppModel;

impl AppModel {
    pub(super) fn branches_spec(&self, project: ProjectId, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Branches(project).id(), "Search branches…")
            .breadcrumb("Switch Branch")
    }

    pub(super) fn branches_items(
        &mut self,
        _project: ProjectId,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
    }

    pub(super) fn branches_opened(&mut self, _project: ProjectId, _cx: &mut Context<Self>) {}

    pub(super) fn branches_action(
        &mut self,
        _project: ProjectId,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    /// Root search rows: the current project's best branches, and whether
    /// there are more.
    pub(super) fn branches_hits(
        &self,
        _query: &[char],
        _limit: usize,
        _cx: &App,
    ) -> (Vec<Item<Handler>>, bool) {
        (Vec::new(), false)
    }

    pub(super) fn changes_spec(&self, project: ProjectId, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Changes(project).id(), "Search changes…").breadcrumb("Changes")
    }

    pub(super) fn changes_items(
        &mut self,
        _project: ProjectId,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
    }

    pub(super) fn changes_opened(&mut self, _project: ProjectId, _cx: &mut Context<Self>) {}

    pub(super) fn changes_action(
        &mut self,
        _project: ProjectId,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
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
