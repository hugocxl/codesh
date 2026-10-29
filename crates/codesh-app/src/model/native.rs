//! Codesh's native Files and Source Control panels, editor and diff panes,
//! and the Go to File and Find in Files pickers, with their server transport.

mod editors;
mod panels;
mod search;

use std::collections::{BTreeMap, HashMap, HashSet};

use codesh_app_core::{ProjectId, ServerId};
use codesh_protocol::{
    ExecRequest, ExecResult, FileChanges, FilesAction, FilesReply, FilesRequest, ServerPath,
};
use codesh_ui::panel::PanelResizeState;
use codesh_ui::tr;
use gpui::{Context, Entity, Subscription};

pub(crate) use editors::{DiffPane, EditorContent, EditorPane};
pub(crate) use panels::{FILES_PANEL, GIT_PANEL};
pub(crate) use search::FileList;

use super::AppModel;
use crate::views::files_panel::FilesPanel;
use crate::views::find_in_files::FindMatch;
use crate::views::git_panel::GitPanel;

/// Exec jobs share the side connection with other features, so these
/// numbers start far above theirs.
const JOB_BASE: u64 = 1 << 48;

pub(crate) struct Native {
    pub(crate) files: HashMap<ProjectId, (Entity<FilesPanel>, Subscription)>,
    pub(crate) git: HashMap<ProjectId, (Entity<GitPanel>, Subscription)>,
    pub(crate) resize: PanelResizeState,
    pub(crate) visible_files: Option<ProjectId>,
    /// The project whose Source Control panel is showing, refreshed each
    /// time it comes into view.
    visible_git: Option<ProjectId>,
    /// Projects whose Source Control panel is waiting for a change it asked
    /// for, shown busy until Git answers.
    git_busy: HashSet<ProjectId>,
    /// Projects whose Source Control panel has an answer to show.
    git_loaded: HashSet<ProjectId>,
    file_lists: HashMap<ProjectId, FileList>,
    find_results: Vec<FindMatch>,
    /// The Files panel entry whose context menu is open.
    menu_path: Option<String>,
    search: u64,
    next_job: u64,
    #[cfg(test)]
    pub(crate) stub: Option<std::rc::Rc<Stub>>,
}

impl Native {
    pub(crate) fn new(_cx: &mut Context<AppModel>) -> Self {
        Self {
            files: HashMap::new(),
            git: HashMap::new(),
            resize: PanelResizeState::default(),
            visible_files: None,
            visible_git: None,
            git_busy: HashSet::new(),
            git_loaded: HashSet::new(),
            file_lists: HashMap::new(),
            find_results: Vec::new(),
            menu_path: None,
            search: 0,
            next_job: 0,
            #[cfg(test)]
            stub: None,
        }
    }
}

/// Stands in for a server in tests.
#[cfg(test)]
pub(crate) struct Stub {
    pub(crate) files: Box<dyn Fn(FilesRequest) -> Result<FilesReply, String>>,
    pub(crate) exec: Box<dyn Fn(ExecRequest) -> Result<ExecResult, String>>,
}

/// How native panes reach a project's server: its side connection, so file
/// work never waits behind terminals.
#[derive(Clone)]
enum Transport {
    Client(codesh_client::Client),
    #[cfg(test)]
    Stub(std::rc::Rc<Stub>),
}

impl Transport {
    async fn files(self, request: FilesRequest) -> Result<FilesReply, String> {
        match self {
            Self::Client(client) => client
                .files_async(request)
                .await
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            Self::Stub(stub) => (stub.files)(request),
        }
    }

    async fn exec(self, request: ExecRequest) -> Result<ExecResult, String> {
        match self {
            Self::Client(client) => client
                .exec_async(request)
                .await
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            Self::Stub(stub) => (stub.exec)(request),
        }
    }
}

pub(crate) fn server_path(path: &str) -> ServerPath {
    ServerPath(path.as_bytes().into())
}

pub(crate) fn path_string(path: &ServerPath) -> String {
    String::from_utf8_lossy(&path.0).into_owned()
}

/// Readable server errors shared by all native panels and panes.
pub(crate) fn friendly_error(error: &str, folder: bool) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("worktree is missing")
        || lower.contains("not a git repository")
        || lower.contains("not a repository")
    {
        return tr!("Not a Git repository").to_string();
    }
    if lower.contains("badpath")
        || lower.contains("no such file or directory")
        || lower.contains("not found")
    {
        return if folder {
            tr!("Folder not found")
        } else {
            tr!("File not found")
        }
        .to_string();
    }
    error
        .split_once(": ")
        .filter(|(code, _)| !code.contains(char::is_whitespace))
        .map_or(error, |(_, message)| message)
        .to_owned()
}

pub(crate) fn connecting(error: &str) -> bool {
    error.contains("Still connecting to ") || error.contains("isn't connected.")
}

impl AppModel {
    /// Queue listings while the side connection opens, without manufacturing
    /// an error (or depending on localized connection error wording).
    fn native_connection_pending(&mut self, project: ProjectId, cx: &mut Context<Self>) -> bool {
        #[cfg(test)]
        if self.native.stub.is_some() {
            return false;
        }
        self.state
            .project_server(project)
            .is_some_and(|server| !self.ready(server) || self.side_client(server, cx).is_none())
    }

    fn transport(
        &mut self,
        project: ProjectId,
        cx: &mut Context<Self>,
    ) -> Result<Transport, String> {
        #[cfg(test)]
        if let Some(stub) = &self.native.stub {
            return Ok(Transport::Stub(stub.clone()));
        }
        let server = self
            .state
            .project_server(project)
            .ok_or_else(|| tr!("The project is no longer available.").to_string())?;
        let name = self.server_label(server);
        if !self.ready(server) {
            return Err(tr!("%@ isn't connected.", &name).to_string());
        }
        self.side_client(server, cx)
            .map(Transport::Client)
            .ok_or_else(|| tr!("Still connecting to %@. Try again in a moment.", &name).to_string())
    }

    /// Runs a files request for `project` off the UI thread and hands its
    /// answer to `done`.
    pub(crate) fn files_request(
        &mut self,
        project: ProjectId,
        action: FilesAction,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<FilesReply, String>, &mut Context<Self>) + 'static,
    ) {
        let transport = self.transport(project, cx);
        let request = FilesRequest { project, action };
        cx.spawn(async move |model, cx| {
            let result = match transport {
                Ok(transport) => transport.files(request).await,
                Err(error) => Err(error),
            };
            let _ = model.update(cx, |model, cx| done(model, result, cx));
        })
        .detach();
    }

    /// Runs `argv` in `project`'s folder on its server.
    fn exec_request(
        &mut self,
        project: ProjectId,
        argv: Vec<String>,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Result<ExecResult, String>, &mut Context<Self>) + 'static,
    ) {
        self.native.next_job += 1;
        let request = ExecRequest {
            job: JOB_BASE + self.native.next_job,
            project,
            argv,
            shell: None,
            cwd: None,
            env: BTreeMap::new(),
            stdin: Vec::new(),
            timeout_ms: 30_000,
        };
        let transport = self.transport(project, cx);
        cx.spawn(async move |model, cx| {
            let result = match transport {
                Ok(transport) => transport.exec(request).await,
                Err(error) => Err(error),
            };
            let _ = model.update(cx, |model, cx| done(model, result, cx));
        })
        .detach();
    }

    /// `server`'s side connection opened: work that could not reach it runs now.
    pub(super) fn native_side_ready(&mut self, server: ServerId, cx: &mut Context<Self>) {
        crate::diagnostics::event("native.side_ready", format_args!("server={server}"));
        let on_server = |model: &Self, project: &ProjectId| {
            model.state.project_server(*project) == Some(server)
        };
        let panels: Vec<_> = self
            .native
            .files
            .iter()
            .filter(|(project, _)| on_server(self, project))
            .map(|(_, (panel, _))| panel.clone())
            .collect();
        for panel in panels {
            panel.update(cx, |panel, cx| {
                panel.reconnect(cx);
            });
        }
        let editors: Vec<_> = self
            .editors
            .iter()
            .filter(|(_, editor)| editor.needs_load() && on_server(self, &editor.project))
            .map(|(id, _)| *id)
            .collect();
        for id in editors {
            self.load_editor(id, cx);
        }
    }

    /// A watched project's files changed on its server.
    pub(super) fn native_files_changed(
        &mut self,
        project: ProjectId,
        changes: &FileChanges,
        cx: &mut Context<Self>,
    ) {
        let paths: Vec<String> = changes.paths.iter().map(path_string).collect();
        if let Some((panel, _)) = self.native.files.get(&project) {
            panel.update(cx, |panel, cx| panel.refresh(&paths, changes.rescan, cx));
        }
        if changes.rescan {
            self.native.file_lists.remove(&project);
        } else if let Some(list) = self.native.file_lists.get_mut(&project) {
            list.stale = true;
        }
        let editors: Vec<_> = self
            .editors
            .iter()
            .filter(|(_, editor)| {
                editor.project == project && (changes.rescan || paths.contains(&editor.path))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in editors {
            self.check_disk(id, cx);
        }
    }

    /// A project's repository changed on its server.
    pub(super) fn native_git_changed(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.refresh_git_panel(project, cx);
        self.refresh_diffs(project, cx);
    }

    /// Restyles the native panels and panes.
    pub(super) fn native_theme(&mut self, cx: &mut Context<Self>) {
        let theme = self.theme.clone();
        for (panel, _) in self.native.files.values() {
            panel.update(cx, |panel, cx| panel.set_theme(&theme, cx));
        }
        for (panel, _) in self.native.git.values() {
            panel.update(cx, |panel, cx| panel.set_theme(&theme, cx));
        }
        for editor in self.editors.values() {
            if let EditorContent::Ready { view, .. } = &editor.content {
                view.update(cx, |view, cx| {
                    view.set_theme(&theme, cx);
                    view.set_terminal(&self.terminal, cx);
                });
            }
        }
        for diff in self.diffs.values() {
            if let Some(view) = &diff.view {
                view.update(cx, |view, cx| {
                    view.set_theme(&theme, cx);
                    view.set_terminal(&self.terminal, cx);
                });
            }
        }
        match &self.overlay {
            Some(crate::views::overlays::Overlay::QuickOpen(picker)) => {
                picker.update(cx, |picker, cx| picker.set_theme(&theme, cx));
            }
            Some(crate::views::overlays::Overlay::FindInFiles(picker)) => {
                picker.update(cx, |picker, cx| picker.set_theme(&theme, cx));
            }
            _ => {}
        }
    }

    /// Keeps native panes and panels in step with the projects and panes
    /// that exist: new visible editors and diffs load, closed ones go.
    pub(super) fn sync_native(&mut self, cx: &mut Context<Self>) {
        let projects: HashSet<_> = self.state.projects().iter().map(|p| p.id).collect();
        self.native
            .git
            .retain(|project, _| projects.contains(project));
        self.native
            .git_loaded
            .retain(|project| projects.contains(project));
        self.native
            .git_busy
            .retain(|project| projects.contains(project));
        self.native
            .file_lists
            .retain(|project, _| projects.contains(project));
        let gone: Vec<_> = self
            .native
            .files
            .keys()
            .filter(|project| !projects.contains(project))
            .copied()
            .collect();
        for project in gone {
            self.native.files.remove(&project);
        }
        self.sync_native_panes(cx);
        let watched: HashSet<_> = self
            .native
            .files
            .keys()
            .copied()
            .chain(self.editors.values().map(|editor| editor.project))
            .collect();
        let stale: Vec<_> = self
            .state
            .projects()
            .iter()
            .map(|project| project.id)
            .filter(|project| !watched.contains(project) && self.side_clients.watches(*project))
            .collect();
        for project in stale {
            self.unwatch_files(project, cx);
        }
        for project in watched {
            self.watch_files(project, cx);
        }
        self.sync_side_panel(cx);
    }
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn native_server_errors_hide_protocol_codes_and_classify_missing_targets() {
        assert_eq!(
            friendly_error("BadPath: No such file or directory (os error 2)", false),
            "File not found"
        );
        assert_eq!(
            friendly_error("BadPath: No such file or directory (os error 2)", true),
            "Folder not found"
        );
        assert_eq!(
            friendly_error("BadRequest: Git worktree is missing", false),
            "Not a Git repository"
        );
        assert_eq!(
            friendly_error("fatal: not a git repository (or any parent)", true),
            "Not a Git repository"
        );
        assert_eq!(
            friendly_error("PermissionDenied: Permission denied", false),
            "Permission denied"
        );
    }
}
