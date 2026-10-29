use codesh_app_core::ProjectId;
use codesh_app_core::settings::SidePanel;
use codesh_protocol::{FilesAction, FilesReply, GitAction, GitReply, GitRequest};
use codesh_ui::panel::{PanelId, PanelMode, PanelPlacement, PanelPosition};
use gpui::{AppContext, Context, Entity, Focusable, Pixels, Point, Window};

use super::{path_string, server_path};
use crate::model::AppModel;
use crate::views::files_panel::{FilesPanel, FilesPanelEvent};
use crate::views::git_panel::{GitPanel, GitPanelEvent};
use crate::views::menu::{Command, FilesMenu, Item};

pub(crate) const FILES_PANEL: &str = "builtin:files";
pub(crate) const GIT_PANEL: &str = "builtin:git";

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

impl AppModel {
    /// Shows `panel` on the right, or hides it when it is already there.
    pub(crate) fn toggle_side_panel(
        &mut self,
        panel: SidePanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        let open = self.appearance.side_panel != panel;
        self.appearance.side_panel = if open { panel } else { SidePanel::None };
        self.native.resize.end();
        self.save_appearance(cx);
        self.sync_side_panel(cx);
        if open {
            self.focus_side_panel(window, cx);
        } else {
            self.focus_active(window, cx);
        }
        cx.notify();
    }

    /// Opens Source Control and puts the caret in its commit message.
    pub(crate) fn focus_commit_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.appearance.side_panel == SidePanel::SourceControl {
            self.focus_side_panel(window, cx);
        } else {
            self.toggle_side_panel(SidePanel::SourceControl, window, cx);
        }
    }

    fn close_native_panel(&mut self, cx: &mut Context<Self>) {
        self.appearance.side_panel = SidePanel::None;
        self.native.resize.end();
        self.save_appearance(cx);
        self.sync_side_panel(cx);
        self.focus_requested = true;
        cx.notify();
    }

    fn focus_side_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let project = self.state.current_project().id;
        let focus = match self.appearance.side_panel {
            SidePanel::Files => self
                .native
                .files
                .get(&project)
                .map(|(panel, _)| panel.focus_handle(cx)),
            SidePanel::SourceControl => self
                .native
                .git
                .get(&project)
                .map(|(panel, _)| panel.focus_handle(cx)),
            SidePanel::None => None,
        };
        if let Some(focus) = focus {
            focus.focus(window);
            self.focus_requested = false;
        }
    }

    /// Whether the keyboard is in the open side panel.
    pub(crate) fn side_panel_focused(&self, window: &Window, cx: &gpui::App) -> bool {
        let project = self.state.current_project().id;
        match self.appearance.side_panel {
            SidePanel::Files => self
                .native
                .files
                .get(&project)
                .is_some_and(|(panel, _)| panel.focus_handle(cx).contains_focused(window, cx)),
            SidePanel::SourceControl => self
                .native
                .git
                .get(&project)
                .is_some_and(|(panel, _)| panel.focus_handle(cx).contains_focused(window, cx)),
            SidePanel::None => false,
        }
    }

    /// Pins the chosen side panel for the current project, creating its view
    /// the first time, and unpins the other.
    pub(crate) fn sync_side_panel(&mut self, cx: &mut Context<Self>) {
        let visible_files = (self.appearance.side_panel == SidePanel::Files)
            .then(|| self.state.current_project().id);
        let newly_visible = visible_files.is_some() && visible_files != self.native.visible_files;
        self.native.visible_files = visible_files;
        let visible_git = (self.appearance.side_panel == SidePanel::SourceControl)
            .then(|| self.state.current_project().id);
        let git_newly_visible = visible_git.is_some() && visible_git != self.native.visible_git;
        self.native.visible_git = visible_git;
        let wanted = match self.appearance.side_panel {
            SidePanel::Files => Some(FILES_PANEL),
            SidePanel::SourceControl => Some(GIT_PANEL),
            SidePanel::None => None,
        };
        for id in [FILES_PANEL, GIT_PANEL] {
            if Some(id) != wanted && self.panels.remove(&PanelId::new(id)).is_some() {
                self.native.resize.end();
            }
        }
        let Some(id) = wanted else {
            return;
        };
        let placement = PanelPlacement::new(id, PanelPosition::Right, PanelMode::Pinned);
        if self.panels.placement(&PanelId::new(id)) != Some(&placement) {
            self.place_panel(placement, cx);
        }
        let project = self.state.current_project().id;
        match self.appearance.side_panel {
            SidePanel::Files => {
                if let Some(panel) = self.files_panel(project, cx)
                    && newly_visible
                {
                    panel.update(cx, FilesPanel::ensure_root);
                }
            }
            SidePanel::SourceControl => {
                self.git_panel(project, cx);
                if git_newly_visible || !self.native.git_loaded.contains(&project) {
                    self.refresh_git_panel(project, cx);
                }
            }
            SidePanel::None => {}
        }
    }

    /// Asks for the visible Source Control panel's status again.
    pub(crate) fn refresh_visible_git_panel(&mut self, cx: &mut Context<Self>) {
        if let Some(project) = self.native.visible_git {
            self.refresh_git_panel(project, cx);
        }
    }

    /// The side panel was displaced by another panel taking its place.
    pub(crate) fn side_panel_displaced(&mut self, cx: &mut Context<Self>) {
        self.native.resize.end();
        self.appearance.side_panel = SidePanel::None;
        self.save_appearance(cx);
    }

    /// `project`'s Files panel, created and watched the first time.
    pub(crate) fn files_panel(
        &mut self,
        project: ProjectId,
        cx: &mut Context<Self>,
    ) -> Option<Entity<FilesPanel>> {
        if let Some((panel, _)) = self.native.files.get(&project) {
            return Some(panel.clone());
        }
        let name = self.state.project(project)?.name.clone();
        let (theme, metrics) = (self.theme.clone(), self.metrics);
        let panel = cx.new(|cx| {
            let mut panel = FilesPanel::new(project, &theme, metrics, cx);
            panel.set_project_name(name, cx);
            panel
        });
        let subscription = cx.subscribe(&panel, move |model, _, event, cx| {
            model.files_panel_event(project, event, cx);
        });
        self.native
            .files
            .insert(project, (panel.clone(), subscription));
        self.watch_files(project, cx);
        Some(panel)
    }

    /// `project`'s Source Control panel, created the first time.
    pub(crate) fn git_panel(
        &mut self,
        project: ProjectId,
        cx: &mut Context<Self>,
    ) -> Option<Entity<GitPanel>> {
        if let Some((panel, _)) = self.native.git.get(&project) {
            return Some(panel.clone());
        }
        let name = self.state.project(project)?.name.clone();
        let (theme, metrics) = (self.theme.clone(), self.metrics);
        let panel = cx.new(|cx| {
            let mut panel = GitPanel::new(project, &theme, metrics, cx);
            panel.name = name;
            panel
        });
        let subscription = cx.subscribe(&panel, move |model, _, event, cx| {
            model.git_panel_event(project, event.clone(), cx);
        });
        self.native
            .git
            .insert(project, (panel.clone(), subscription));
        Some(panel)
    }

    fn files_panel_event(
        &mut self,
        project: ProjectId,
        event: &FilesPanelEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            FilesPanelEvent::Close => self.close_native_panel(cx),
            FilesPanelEvent::List { path } => self.list_directory(project, path.clone(), cx),
            FilesPanelEvent::Open { path } => self.open_project_file(project, path, None, cx),
            FilesPanelEvent::CreateFile { path } => {
                let action = FilesAction::Write {
                    path: server_path(path),
                    content: String::new(),
                };
                let path = path.clone();
                self.files_mutation(project, action, vec![path.clone()], cx, move |model, cx| {
                    model.open_project_file(project, &path, None, cx);
                });
            }
            FilesPanelEvent::CreateFolder { path } => {
                let action = FilesAction::Mkdir(server_path(path));
                self.files_mutation(project, action, vec![path.clone()], cx, |_, _| {});
            }
            FilesPanelEvent::Rename { from, to } => {
                let action = FilesAction::Rename {
                    path: server_path(from),
                    name: server_path(basename(to)),
                };
                self.files_mutation(
                    project,
                    action,
                    vec![from.clone(), to.clone()],
                    cx,
                    |_, _| {},
                );
            }
            FilesPanelEvent::Delete { path } => self.confirm_delete_file(project, path.clone(), cx),
            FilesPanelEvent::ContextMenu { path, position } => {
                self.files_menu(project, path.clone(), *position, cx);
            }
        }
    }

    fn list_directory(&mut self, project: ProjectId, path: String, cx: &mut Context<Self>) {
        let pending = self.native_connection_pending(project, cx);
        if pending {
            // Keep the tree request pending until native_side_ready reissues it.
            return;
        }
        let action = FilesAction::List(server_path(&path));
        self.files_request(project, action, cx, move |model, result, cx| {
            let Some((panel, _)) = model.native.files.get(&project) else {
                return;
            };
            panel.update(cx, |panel, cx| match result {
                Ok(FilesReply::Entries(entries)) => panel.set_entries(&path, entries, cx),
                Ok(_) => panel.set_error(Some("Unexpected reply".to_string()), cx),
                Err(error) => panel.set_error(Some(error), cx),
            });
        });
    }

    /// Runs a change to `project`'s files, then lists the folders it touched.
    fn files_mutation(
        &mut self,
        project: ProjectId,
        action: FilesAction,
        touched: Vec<String>,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) {
        self.files_request(project, action, cx, move |model, result, cx| {
            let panel = model
                .native
                .files
                .get(&project)
                .map(|(panel, _)| panel.clone());
            match result {
                Ok(_) => {
                    if let Some(panel) = panel {
                        panel.update(cx, |panel, cx| panel.refresh(&touched, false, cx));
                    }
                    if let Some(list) = model.native.file_lists.get_mut(&project) {
                        list.stale = true;
                    }
                    then(model, cx);
                }
                Err(error) => match panel {
                    Some(panel) => panel.update(cx, |panel, cx| {
                        panel.set_error(Some(error), cx);
                    }),
                    None => model.fail(error, cx),
                },
            }
        });
    }

    fn confirm_delete_file(&mut self, project: ProjectId, path: String, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        let window = self.window;
        let name = basename(&path).to_owned();
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let confirmed = crate::views::confirm::prompt_delete_file(window, &name, cx)
                .await
                .unwrap_or(false);
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                if confirmed && model.state.project(project).is_some() {
                    let action = FilesAction::Delete(vec![server_path(&path)]);
                    model.files_mutation(project, action, vec![path], cx, |_, _| {});
                }
            });
        }));
    }

    fn files_menu(
        &mut self,
        project: ProjectId,
        path: String,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let local = self
            .state
            .project(project)
            .is_some_and(|project| project.server_id.is_local());
        let directory = self
            .native
            .files
            .get(&project)
            .is_some_and(|(panel, _)| panel.read(cx).is_directory(&path));
        let command = |action| Command::FilesEntry(project, action);
        let mut items = Vec::new();
        if !directory {
            items.push(Item::action("Open", command(FilesMenu::Open)));
        }
        items.extend([
            Item::action("New File…", command(FilesMenu::NewFile)).separated(),
            Item::action("New Folder…", command(FilesMenu::NewFolder)),
            Item::action("Rename…", command(FilesMenu::Rename)).separated(),
            Item::action("Delete", command(FilesMenu::Delete)),
            Item::action("Copy Path", command(FilesMenu::CopyPath)).separated(),
            Item::action("Copy Relative Path", command(FilesMenu::CopyRelativePath)),
        ]);
        if local {
            items.push(Item::action("Reveal in Finder", command(FilesMenu::Reveal)));
        }
        self.native.menu_path = Some(path);
        let model = cx.entity().downgrade();
        let window = self.window;
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = model.update(cx, |model, cx| model.open_menu(items, position, window, cx));
            });
        });
    }

    /// Runs a row of a Files panel's context menu.
    pub(crate) fn perform_files_menu(
        &mut self,
        project: ProjectId,
        action: FilesMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.native.menu_path.take() else {
            return;
        };
        let path = path.as_str();
        let Some(panel) = self
            .native
            .files
            .get(&project)
            .map(|(panel, _)| panel.clone())
        else {
            return;
        };
        let absolute = self
            .state
            .project(project)
            .map(|project| project.directory.join(path));
        match action {
            FilesMenu::Open => panel.update(cx, |panel, cx| panel.open(path, cx)),
            FilesMenu::NewFile | FilesMenu::NewFolder => panel.update(cx, |panel, cx| {
                let directory = panel.selected_directory();
                if action == FilesMenu::NewFile {
                    panel.new_file(&directory, window, cx);
                } else {
                    panel.new_folder(&directory, window, cx);
                }
            }),
            FilesMenu::Rename => panel.update(cx, |panel, cx| panel.rename(path, window, cx)),
            FilesMenu::Delete => self.confirm_delete_file(project, path.to_owned(), cx),
            FilesMenu::CopyPath => {
                if let Some(absolute) = absolute {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                        absolute.to_string_lossy().into_owned(),
                    ));
                }
            }
            FilesMenu::CopyRelativePath => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(path.to_owned()));
            }
            FilesMenu::Reveal => {
                if let Some(absolute) = absolute {
                    cx.reveal_path(&absolute);
                }
            }
        }
    }

    fn git_panel_event(
        &mut self,
        project: ProjectId,
        event: GitPanelEvent,
        cx: &mut Context<Self>,
    ) {
        let paths = |paths: Vec<String>| paths.iter().map(|path| server_path(path)).collect();
        match event {
            GitPanelEvent::Close => self.close_native_panel(cx),
            GitPanelEvent::Refresh => self.refresh_git_panel(project, cx),
            GitPanelEvent::Stage(files) => {
                self.git_panel_change(project, GitAction::Stage(paths(files)), cx);
            }
            GitPanelEvent::Unstage(files) => {
                self.git_panel_change(project, GitAction::Unstage(paths(files)), cx);
            }
            GitPanelEvent::Discard(files) => {
                let message = match files.as_slice() {
                    [file] => format!(
                        "Discard changes to “{file}”? Untracked files will be permanently deleted; staged changes are preserved."
                    ),
                    _ => format!(
                        "Discard changes to {} files? Untracked files will be permanently deleted; staged changes are preserved.",
                        files.len()
                    ),
                };
                self.confirm_git_action(
                    project,
                    GitAction::Discard(paths(files)),
                    message.clone(),
                    cx,
                );
            }
            GitPanelEvent::Commit { message, amend } => {
                let action = if amend {
                    GitAction::Amend { message }
                } else {
                    GitAction::Commit {
                        message,
                        stage_all: false,
                    }
                };
                self.git_panel_change(project, action, cx);
            }
            GitPanelEvent::OpenDiff { path, staged } => self.open_diff(project, &path, staged, cx),
        }
    }

    fn git_panel_change(&mut self, project: ProjectId, action: GitAction, cx: &mut Context<Self>) {
        if !self.git_request(project, action, cx) {
            return;
        }
        self.native.git_busy.insert(project);
        if let Some((panel, _)) = self.native.git.get(&project) {
            panel.update(cx, |panel, cx| {
                panel.set_error(None, cx);
                panel.set_busy(true, cx);
            });
        }
    }

    /// Asks for `project`'s working tree status when its panel exists.
    pub(super) fn refresh_git_panel(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        if self.native.git.contains_key(&project) {
            self.git_request(project, GitAction::Status { local: true }, cx);
        }
    }

    /// Git answered: show it in the panels and panes that asked, and refresh
    /// them after a change.
    pub(in crate::model) fn native_git_reply(
        &mut self,
        request: &GitRequest,
        result: &Result<GitReply, codesh_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        let project = request.project;
        match &request.action {
            GitAction::Status { local: true } => {
                self.native.git_loaded.insert(project);
                let Some((panel, _)) = self.native.git.get(&project) else {
                    return;
                };
                panel.update(cx, |panel, cx| match result {
                    Ok(GitReply::Status(status)) => panel.set_status((**status).clone(), cx),
                    Ok(_) => {}
                    Err(error) => panel.set_error(Some(error.to_string()), cx),
                });
            }
            GitAction::Diff(diff) => {
                let path = diff.path.as_ref().map(path_string).unwrap_or_default();
                self.receive_diff(project, &path, diff.staged, result, cx);
            }
            GitAction::Stage(_)
            | GitAction::Unstage(_)
            | GitAction::Discard(_)
            | GitAction::Commit { .. }
            | GitAction::Amend { .. } => {
                let asked = self.native.git_busy.remove(&project);
                if let Some((panel, _)) = self.native.git.get(&project) {
                    let committed = matches!(
                        request.action,
                        GitAction::Commit { .. } | GitAction::Amend { .. }
                    );
                    panel.update(cx, |panel, cx| {
                        if asked {
                            panel.set_busy(false, cx);
                        }
                        match result {
                            Ok(_) if committed && asked => panel.committed(cx),
                            Err(error) if asked => panel.set_error(Some(error.to_string()), cx),
                            _ => {}
                        }
                    });
                }
                self.refresh_git_panel(project, cx);
                self.refresh_diffs(project, cx);
            }
            _ => {}
        }
    }
}
