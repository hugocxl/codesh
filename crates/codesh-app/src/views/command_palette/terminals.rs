//! Existing Terminals: a project's sessions that no tab shows, which the
//! server lists on request.

use std::{collections::HashMap, rc::Rc};

use codesh_app_core::{ProjectId, ServerId};
use codesh_protocol::{ProjectSession, ProjectSessions, SessionId};
use codesh_ui::command_palette::{Item, ItemAction, PageContent, PageSpec};
use codesh_ui::fuzzy;
use gpui::{App, Context, Window};

use super::{Handler, Kind, PaletteEntry};
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

    #[expect(
        clippy::unused_self,
        reason = "The page dispatcher uses a model method"
    )]
    pub(super) fn terminals_spec(&self, project: ProjectId, _cx: &App) -> PageSpec {
        PageSpec::new(
            PaletteEntry::Terminals(project).id(),
            "Filter terminals or owners…",
        )
        .breadcrumb("Existing Terminals")
    }

    pub(super) fn terminals_items(
        &mut self,
        project: ProjectId,
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        let listing = self.existing_sessions.projects.get(&project);
        if self.sessions_revision(project).is_none() {
            return PageContent::error("Reconnect to see existing terminals");
        }
        if let Some(error) = listing.and_then(|listing| listing.error.as_ref()) {
            return PageContent::error(error.clone());
        }
        if listing.is_none_or(|listing| listing.revision.is_none() && listing.pending) {
            return PageContent::loading("Loading terminals…");
        }
        let items = terminal_items(project, self.available_sessions(project), query, usize::MAX);
        if items.is_empty() {
            return PageContent::empty(if query.is_empty() {
                "No other terminals in this project"
            } else {
                "No matching terminals"
            });
        }
        PageContent::items(items)
    }

    pub(super) fn terminals_opened(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.request_sessions(project, cx);
        self.refresh_existing_sessions(cx);
    }

    pub(super) fn terminals_action(
        &mut self,
        project: ProjectId,
        item: &str,
        action: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if action != "open" {
            return;
        }
        let Some(id) = item.parse().ok().and_then(SessionId::new) else {
            return;
        };
        self.choose_existing_session(project, id, cx);
    }

    fn choose_existing_session(
        &mut self,
        project: ProjectId,
        id: SessionId,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self
            .available_sessions(project)
            .into_iter()
            .find(|session| session.info.id == id)
            .cloned()
        else {
            return;
        };
        self.open_existing_session(project, &session, cx);
    }

    /// Root search rows: the current project's best existing terminals, and
    /// whether there are more.
    pub(super) fn terminals_hits(
        &self,
        query: &[char],
        limit: usize,
        _cx: &App,
    ) -> (Vec<Item<Handler>>, bool) {
        let project = self.state.current_project().id;
        if self.sessions_revision(project).is_none() {
            return (Vec::new(), false);
        }
        let query: String = query.iter().collect();
        let mut items = terminal_items(
            project,
            self.available_sessions(project),
            &query,
            limit.saturating_add(1),
        );
        let more = items.len() > limit;
        items.truncate(limit);
        (items, more)
    }
}

fn terminal_items(
    project: ProjectId,
    sessions: Vec<&ProjectSession>,
    query: &str,
    limit: usize,
) -> Vec<Item<Handler>> {
    let candidates = sessions.into_iter().map(|session| {
        let title = format!("Terminal {}", session.info.id.get());
        let directory = String::from_utf8_lossy(&session.info.directory.0).into_owned();
        let owner = session
            .owner
            .map_or_else(|| "No owner".to_owned(), |owner| format!("Owner: {owner}"));
        let search = format!("{title} {directory} {owner}");
        (session.info.id, title, directory, owner, search)
    });
    let ranked = if query.is_empty() {
        candidates
            .take(limit)
            .map(|item| fuzzy::Ranked {
                item,
                score: 0,
                ranges: Vec::new(),
            })
            .collect()
    } else {
        fuzzy::rank(candidates, |candidate| &candidate.4, query, limit)
    };
    let query = fuzzy::normalize(query);
    ranked
        .into_iter()
        .map(|entry| {
            let (id, title, directory, owner, _) = entry.item;
            let highlights =
                fuzzy::fuzzy(&title, &query).map_or_else(Vec::new, |found| found.ranges);
            let open: Handler = Rc::new(move |model, _, cx| {
                model.choose_existing_session(project, id, cx);
            });
            Item::run(id.get().to_string(), title, open)
                .highlights(highlights)
                .subtitle(directory)
                .badge(owner)
                .kind(Kind::Terminal.label())
                .tile(Kind::Terminal.tile())
                .actions(vec![ItemAction::new("open", "Open Terminal")])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use codesh_protocol::{
        ClientId, ClientKind, ServerPath, SessionClient, SessionInfo, SessionStatus,
    };

    use super::*;

    fn session(project: ProjectId, id: u64, directory: &[u8]) -> ProjectSession {
        ProjectSession {
            info: SessionInfo {
                id: SessionId::new(id).expect("session"),
                project,
                directory: ServerPath(directory.to_vec()),
            },
            status: SessionStatus::Live,
            attached: false,
            owner: None,
        }
    }

    #[test]
    fn terminal_search_matches_directory_and_owner_without_highlighting_the_title() {
        let project = ProjectId::new();
        let mut owned = session(project, 12, "/tmp/éclair".as_bytes());
        owned.owner = Some(SessionClient {
            id: ClientId::new(),
            kind: ClientKind::Desktop,
        });
        let other = session(project, 2, b"/tmp/other");
        for query in ["ÉCLAIR", "desktop"] {
            let rows = terminal_items(project, vec![&other, &owned], query, 5);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].id, "12");
            assert!(rows[0].highlights.is_empty());
            assert_eq!(
                rows[0].subtitle.as_ref().map(AsRef::as_ref),
                Some("/tmp/éclair")
            );
            assert_eq!(
                rows[0].badge.as_ref().map(AsRef::as_ref),
                Some(format!("Owner: {}", owned.owner.expect("owner")).as_str())
            );
        }
    }

    #[test]
    fn terminal_search_limits_keep_the_page_ids_and_unfiltered_server_order() {
        let project = ProjectId::new();
        let sessions = [
            session(project, 20, b"/tmp/twenty"),
            session(project, 3, b"/tmp/three"),
            session(project, 1, b"/tmp/one"),
        ];
        let all = terminal_items(project, sessions.iter().collect(), "", usize::MAX);
        assert_eq!(
            all.iter().map(|item| item.id.as_ref()).collect::<Vec<_>>(),
            ["20", "3", "1"]
        );
        let page = terminal_items(project, sessions.iter().collect(), "term", usize::MAX);
        let hits = terminal_items(project, sessions.iter().collect(), "term", 2);
        assert_eq!(hits.len(), 2);
        for (hit, row) in hits.iter().zip(&page) {
            assert_eq!(hit.id, row.id);
            assert_eq!(hit.highlights, row.highlights);
            assert_eq!(hit.actions[0].id, "open");
            assert!(matches!(
                hit.target,
                codesh_ui::command_palette::Target::Run(_)
            ));
        }
    }
}
