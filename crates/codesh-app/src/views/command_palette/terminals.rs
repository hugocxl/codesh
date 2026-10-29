//! Existing Terminals: a project's sessions that no tab shows, which the
//! server lists on request.

#![expect(clippy::unused_self, reason = "A stub until its page is implemented")]

use std::collections::HashMap;

use codesh_app_core::{ProjectId, ServerId};
use codesh_protocol::{ProjectSession, ProjectSessions};
use codesh_ui::command_palette::{Item, PageContent, PageSpec};
use gpui::{App, Context, Window};

use super::{Handler, PaletteEntry};
use crate::{boot::Work, model::AppModel};

#[derive(Default)]
pub(crate) struct ExistingSessions {
    projects: HashMap<ProjectId, Listing>,
}

#[derive(Default)]
struct Listing {
    entries: Vec<ProjectSession>,
    revision: Option<u64>,
    pending: bool,
    error: Option<String>,
}

impl AppModel {
    /// Opens `project`'s existing terminals, or closes them.
    pub(crate) fn open_session_picker(
        &mut self,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_palette(PaletteEntry::Terminals(project), window, cx);
    }

    #[cfg(test)]
    pub(crate) fn existing_terminal_count(&self) -> usize {
        self.available_sessions(self.state.current_project().id)
            .len()
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Existing Terminals' page lists them")
    )]
    fn available_sessions(&self, project: ProjectId) -> Vec<&ProjectSession> {
        let references = self
            .state
            .project_server(project)
            .map(|server| self.state.session_references(server))
            .unwrap_or_default();
        self.existing_sessions
            .projects
            .get(&project)
            .into_iter()
            .flat_map(|listing| &listing.entries)
            .filter(|session| !references.contains(&session.info.id) && !session.attached)
            .collect()
    }

    pub(crate) fn refresh_existing_sessions(&mut self, cx: &mut Context<Self>) {
        let project = self.state.current_project().id;
        self.request_sessions(project, cx);
        if let Some(PaletteEntry::Terminals(shown)) = self.palette_page(cx) {
            self.request_sessions(shown, cx);
        }
        self.show_existing_sessions(cx);
    }

    /// Forgets what `server` listed, which a new connection lists again.
    pub(crate) fn forget_server_sessions(&mut self, server: ServerId, cx: &mut Context<Self>) {
        let state = &self.state;
        self.existing_sessions
            .projects
            .retain(|project, _| state.project_server(*project) != Some(server));
        self.show_existing_sessions(cx);
    }

    /// Lists the open Existing Terminals page's project again.
    pub(crate) fn refresh_terminals_page(&mut self, cx: &mut Context<Self>) {
        if let Some(PaletteEntry::Terminals(project)) = self.palette_page(cx) {
            self.existing_sessions
                .projects
                .entry(project)
                .or_default()
                .revision = None;
        }
        self.refresh_existing_sessions(cx);
    }

    fn request_sessions(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let Some(current) = self.sessions_revision(project) else {
            return;
        };
        let listing = self.existing_sessions.projects.entry(project).or_default();
        if listing.pending || listing.revision.is_some_and(|revision| revision >= current) {
            return;
        }
        if self.send_session_request(project, Work::ProjectSessions { project }, cx) {
            let listing = self.existing_sessions.projects.entry(project).or_default();
            listing.pending = true;
            listing.error = None;
        }
    }

    pub(crate) fn receive_session_page(
        &mut self,
        project: ProjectId,
        result: Result<ProjectSessions, codesh_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        let current = self.sessions_revision(project).unwrap_or_default();
        let listing = self.existing_sessions.projects.entry(project).or_default();
        listing.pending = false;
        match result {
            Ok(page) => {
                listing.entries = page.sessions;
                listing.revision = Some(page.revision);
                listing.error = None;
            }
            Err(error) => {
                listing.entries.clear();
                listing.revision = Some(current);
                listing.error = Some(error.to_string());
            }
        }
        self.refresh_existing_sessions(cx);
        cx.notify();
    }

    /// Shows what is known on an open Existing Terminals page or root search.
    fn show_existing_sessions(&mut self, cx: &mut Context<Self>) {
        match self.palette_page(cx) {
            Some(PaletteEntry::Terminals(_)) => self.refresh_palette(cx),
            Some(PaletteEntry::Root) => self.refresh_root_page(cx),
            _ => {}
        }
    }

    pub(super) fn terminals_spec(&self, project: ProjectId, _cx: &App) -> PageSpec {
        PageSpec::new(
            PaletteEntry::Terminals(project).id(),
            "Filter terminals or owners…",
        )
        .breadcrumb("Existing Terminals")
    }

    pub(super) fn terminals_items(
        &mut self,
        _project: ProjectId,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
    }

    pub(super) fn terminals_opened(&mut self, _project: ProjectId, _cx: &mut Context<Self>) {}

    pub(super) fn terminals_action(
        &mut self,
        _project: ProjectId,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    /// Root search rows: the current project's best existing terminals, and
    /// whether there are more.
    pub(super) fn terminals_hits(
        &self,
        _query: &[char],
        _limit: usize,
        _cx: &App,
    ) -> (Vec<Item<Handler>>, bool) {
        (Vec::new(), false)
    }
}
