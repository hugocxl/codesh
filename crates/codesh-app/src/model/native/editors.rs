use std::collections::HashSet;

use codesh_app_core::{PaneContent, PaneId, ProjectId};
use codesh_protocol::{FilesAction, FilesReply, GitAction, GitDiffRequest, GitReply};
use codesh_ui::tr;
use gpui::{AppContext, Context, Entity, Subscription};

use super::server_path;
use crate::model::AppModel;
use crate::views::confirm::Unsaved;
use crate::views::diff_pane::DiffView;
use crate::views::editor::{EditorEvent, EditorView};

/// Diffs longer than this many lines are cut short by the server.
const DIFF_LINE_LIMIT: u32 = 100_000;

pub(crate) enum EditorContent {
    /// The file's text is on its way, or the server could not be reached yet.
    Loading,
    Ready {
        view: Entity<EditorView>,
        _subscription: Subscription,
    },
    /// Why the file cannot be edited here, such as being binary or too large.
    Message(String),
}

/// An editor pane's file and what is known of it on disk.
pub(crate) struct EditorPane {
    pub(crate) project: ProjectId,
    pub(crate) path: String,
    pub(crate) content: EditorContent,
    pub(crate) dirty: bool,
    /// What the file held on disk when last read or written.
    saved: Option<String>,
    /// The file's new text on disk while the editor has unsaved changes.
    pub(crate) disk: Option<String>,
    requested: bool,
    /// Where the caret goes once the text arrives.
    jump: Option<(u32, u32)>,
}

impl EditorPane {
    pub(super) fn needs_load(&self) -> bool {
        matches!(self.content, EditorContent::Loading) && !self.requested
    }

    pub(crate) fn view(&self) -> Option<&Entity<EditorView>> {
        match &self.content {
            EditorContent::Ready { view, .. } => Some(view),
            _ => None,
        }
    }
}

/// A diff pane's file and its latest diff.
pub(crate) struct DiffPane {
    pub(crate) project: ProjectId,
    pub(crate) path: String,
    pub(crate) staged: bool,
    pub(crate) view: Option<Entity<DiffView>>,
    pub(crate) error: Option<String>,
    /// Not asked for yet, as the server was not connected.
    unsent: bool,
}

/// The editor's message for a file the server would not read as text.
fn unreadable(error: &str) -> Option<String> {
    if error.contains("not valid UTF-8") {
        Some(tr!("This file is binary, so it can't be shown here.").to_string())
    } else if error.contains("5 MiB") {
        Some(tr!("This file is larger than 5 MB, so it can't be opened here.").to_string())
    } else {
        None
    }
}

impl AppModel {
    /// Opens `path` of `project` in an editor pane, with the caret at a
    /// one-based line and column when given.
    pub(crate) fn open_project_file(
        &mut self,
        project: ProjectId,
        path: &str,
        position: Option<(u32, u32)>,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() || self.close_request.is_some() {
            return;
        }
        let previous = self.state.clone();
        let pane = match self.state.open_editor_pane(project, path) {
            Ok(pane) => pane,
            Err(error) => {
                self.state = previous;
                self.fail(
                    tr!("Could not open file: %@", error.to_string()).to_string(),
                    cx,
                );
                return;
            }
        };
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        self.changed(cx);
        self.focus_requested = true;
        if let Some((line, column)) = position {
            self.jump_editor(pane, line, column, cx);
        }
    }

    fn jump_editor(&mut self, pane: PaneId, line: u32, column: u32, cx: &mut Context<Self>) {
        let Some(editor) = self.editors.get_mut(&pane) else {
            return;
        };
        match editor.view() {
            Some(view) => view.update(cx, |view, cx| view.go_to_line(line, column, cx)),
            None => editor.jump = Some((line, column)),
        }
    }

    /// Opens `path`'s diff in a pane beside the focused one.
    pub(crate) fn open_diff(
        &mut self,
        project: ProjectId,
        path: &str,
        staged: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() || self.close_request.is_some() {
            return;
        }
        let previous = self.state.clone();
        if let Err(error) = self.state.open_diff_pane(project, path, staged) {
            self.state = previous;
            self.fail(
                tr!("Could not open file: %@", error.to_string()).to_string(),
                cx,
            );
            return;
        }
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        self.changed(cx);
        self.focus_requested = true;
    }

    /// Editors and diffs whose panes closed go; visible new ones load.
    pub(super) fn sync_native_panes(&mut self, cx: &mut Context<Self>) {
        let panes: Vec<_> = self
            .state
            .projects()
            .iter()
            .flat_map(|project| {
                project
                    .tabs
                    .iter()
                    .flat_map(|tab| &tab.panes)
                    .map(move |pane| (pane.id, project.id, &pane.content))
            })
            .filter(|(_, _, content)| {
                matches!(
                    content,
                    PaneContent::Editor { .. } | PaneContent::Diff { .. }
                )
            })
            .map(|(id, project, content)| (id, project, content.clone()))
            .collect();
        let existing: HashSet<_> = panes.iter().map(|(id, ..)| *id).collect();
        self.editors.retain(|id, _| existing.contains(id));
        self.diffs.retain(|id, _| existing.contains(id));
        let retry: Vec<_> = self
            .editors
            .iter()
            .filter(|(_, editor)| editor.needs_load())
            .map(|(id, _)| *id)
            .collect();
        for id in retry {
            self.load_editor(id, cx);
        }
        let mut unsent: Vec<_> = self
            .diffs
            .values()
            .filter(|diff| diff.unsent)
            .map(|diff| (diff.project, diff.path.clone(), diff.staged))
            .collect();
        unsent.dedup();
        for (project, path, staged) in unsent {
            self.request_diff(project, &path, staged, cx);
        }
        let visible: HashSet<_> = self.visible_panes().into_iter().collect();
        for (id, project, content) in panes {
            if !visible.contains(&id) {
                continue;
            }
            match content {
                PaneContent::Editor { path } if !self.editors.contains_key(&id) => {
                    self.editors.insert(
                        id,
                        EditorPane {
                            project,
                            path,
                            content: EditorContent::Loading,
                            dirty: false,
                            saved: None,
                            disk: None,
                            requested: false,
                            jump: None,
                        },
                    );
                    self.load_editor(id, cx);
                }
                PaneContent::Diff { path, staged } if !self.diffs.contains_key(&id) => {
                    self.diffs.insert(
                        id,
                        DiffPane {
                            project,
                            path: path.clone(),
                            staged,
                            view: None,
                            error: None,
                            unsent: false,
                        },
                    );
                    self.request_diff(project, &path, staged, cx);
                }
                _ => {}
            }
        }
    }

    /// Reads an editor pane's file from its server.
    pub(super) fn load_editor(&mut self, id: PaneId, cx: &mut Context<Self>) {
        let Some(editor) = self.editors.get_mut(&id) else {
            return;
        };
        let (project, path) = (editor.project, editor.path.clone());
        // Unreachable servers are retried when their side connection opens.
        if self.transport(project, cx).is_err() {
            return;
        }
        if let Some(editor) = self.editors.get_mut(&id) {
            editor.requested = true;
        }
        self.files_request(
            project,
            FilesAction::Read(server_path(&path)),
            cx,
            move |model, result, cx| {
                model.editor_loaded(id, &path, result, cx);
            },
        );
    }

    fn editor_loaded(
        &mut self,
        id: PaneId,
        path: &str,
        result: Result<FilesReply, String>,
        cx: &mut Context<Self>,
    ) {
        let theme = self.theme.clone();
        let active = self.active_pane() == Some(id);
        let Some(editor) = self.editors.get_mut(&id) else {
            return;
        };
        editor.requested = false;
        let text = match result {
            Ok(FilesReply::Content(content)) if content.content.contains('\0') => {
                editor.content = EditorContent::Message(
                    tr!("This file is binary, so it can't be shown here.").to_string(),
                );
                cx.notify();
                return;
            }
            Ok(FilesReply::Content(content)) => content.content,
            Ok(_) => return,
            Err(error) => {
                if editor.view().is_none() && !super::connecting(&error) {
                    editor.content = EditorContent::Message(
                        unreadable(&error).unwrap_or_else(|| super::friendly_error(&error, false)),
                    );
                }
                cx.notify();
                return;
            }
        };
        editor.saved = Some(text.clone());
        let view = cx.new(|cx| EditorView::new(text, path, &theme, &self.terminal, cx));
        if let Some((line, column)) = editor.jump.take() {
            view.update(cx, |view, cx| view.go_to_line(line, column, cx));
        }
        let subscription = cx.subscribe(&view, move |model, view, event, cx| match event {
            EditorEvent::Changed => {
                let dirty = view.read(cx).is_dirty();
                if let Some(editor) = model.editors.get_mut(&id)
                    && editor.dirty != dirty
                {
                    editor.dirty = dirty;
                    cx.notify();
                }
            }
            EditorEvent::SaveRequested => model.save_editor(id, cx, |_, _, _| {}),
        });
        editor.content = EditorContent::Ready {
            view,
            _subscription: subscription,
        };
        editor.dirty = false;
        if active {
            self.focus_requested = true;
        }
        cx.notify();
    }

    /// Writes an editor pane's text to its file; `done` hears whether it was
    /// saved.
    pub(crate) fn save_editor(
        &mut self,
        id: PaneId,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, bool, &mut Context<Self>) + 'static,
    ) {
        let Some(editor) = self.editors.get(&id) else {
            done(self, false, cx);
            return;
        };
        let Some(view) = editor.view().cloned() else {
            done(self, false, cx);
            return;
        };
        let text = view.read(cx).text();
        let (project, path) = (editor.project, editor.path.clone());
        let action = FilesAction::Write {
            path: server_path(&path),
            content: text.clone(),
        };
        self.files_request(project, action, cx, move |model, result, cx| {
            let saved = result.is_ok();
            match result {
                Ok(_) => {
                    if let Some(editor) = model.editors.get_mut(&id) {
                        editor.saved = Some(text.clone());
                        editor.disk = None;
                        // Typing during the write leaves the newer text unsaved.
                        if view.read(cx).text() == text {
                            view.update(cx, EditorView::mark_saved);
                            editor.dirty = false;
                        }
                    }
                    if let Some(list) = model.native.file_lists.get_mut(&project) {
                        list.stale = true;
                    }
                    cx.notify();
                }
                Err(error) => {
                    model.fail(tr!("Could not save %@: %@", &path, &error).to_string(), cx);
                }
            }
            done(model, saved, cx);
        });
    }

    /// The file under an editor changed on disk: an editor without unsaved
    /// changes takes the new text, and one with them offers it.
    pub(super) fn check_disk(&mut self, id: PaneId, cx: &mut Context<Self>) {
        let Some(editor) = self.editors.get(&id) else {
            return;
        };
        if editor.view().is_none() {
            if editor.needs_load() {
                self.load_editor(id, cx);
            }
            return;
        }
        let (project, path) = (editor.project, editor.path.clone());
        self.files_request(
            project,
            FilesAction::Read(server_path(&path)),
            cx,
            move |model, result, cx| {
                let Ok(FilesReply::Content(content)) = result else {
                    return;
                };
                let Some(editor) = model.editors.get_mut(&id) else {
                    return;
                };
                let Some(view) = editor.view().cloned() else {
                    return;
                };
                if editor.saved.as_deref() == Some(content.content.as_str()) {
                    return;
                }
                if view.read(cx).is_dirty() {
                    editor.disk = Some(content.content);
                } else {
                    editor.saved = Some(content.content.clone());
                    editor.disk = None;
                    if view.read(cx).text() != content.content {
                        view.update(cx, |view, cx| view.set_text(content.content, cx));
                    }
                }
                cx.notify();
            },
        );
    }

    /// Answers an editor's changed-on-disk banner: take the disk's text, or
    /// keep the editor's.
    pub(crate) fn resolve_disk_change(&mut self, id: PaneId, reload: bool, cx: &mut Context<Self>) {
        let Some(editor) = self.editors.get_mut(&id) else {
            return;
        };
        let Some(disk) = editor.disk.take() else {
            return;
        };
        if reload && let Some(view) = editor.view().cloned() {
            editor.saved = Some(disk.clone());
            editor.dirty = false;
            view.update(cx, |view, cx| view.set_text(disk, cx));
        }
        cx.notify();
    }

    /// Whether the pane is an editor with unsaved changes.
    pub(crate) fn editor_dirty(&self, id: PaneId) -> bool {
        self.editors.get(&id).is_some_and(|editor| editor.dirty)
    }

    fn request_diff(
        &mut self,
        project: ProjectId,
        path: &str,
        staged: bool,
        cx: &mut Context<Self>,
    ) {
        let request = GitDiffRequest {
            path: Some(server_path(path)),
            staged,
            raw: false,
            line_limit: Some(DIFF_LINE_LIMIT),
            context: Some(1_000_000),
        };
        let sent = self.git_request(project, GitAction::Diff(request), cx);
        for diff in self.diffs.values_mut() {
            if diff.project == project && diff.path == path && diff.staged == staged {
                diff.unsent = !sent;
                if diff.view.is_none() {
                    diff.error =
                        (!sent).then(|| tr!("Connect to the server to see this diff.").to_string());
                }
            }
        }
    }

    /// Asks again for each open diff of `project`.
    pub(super) fn refresh_diffs(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let mut requests: Vec<_> = self
            .diffs
            .values()
            .filter(|diff| diff.project == project)
            .map(|diff| (diff.path.clone(), diff.staged))
            .collect();
        requests.sort();
        requests.dedup();
        for (path, staged) in requests {
            self.request_diff(project, &path, staged, cx);
        }
    }

    pub(super) fn receive_diff(
        &mut self,
        project: ProjectId,
        path: &str,
        staged: bool,
        result: &Result<GitReply, codesh_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        let (theme, metrics) = (self.theme.clone(), self.metrics);
        for diff in self.diffs.values_mut() {
            if diff.project != project || diff.path != path || diff.staged != staged {
                continue;
            }
            match result {
                Ok(GitReply::Diff(reply)) => {
                    diff.error = None;
                    if let Some(view) = &diff.view {
                        view.update(cx, |view, cx| view.set_diff(reply.clone(), cx));
                    } else {
                        let reply = reply.clone();
                        let view = cx.new(|cx| {
                            let mut view =
                                DiffView::new(reply, path, &theme, metrics, &self.terminal, cx);
                            view.set_staged(staged, cx);
                            view
                        });
                        diff.view = Some(view);
                    }
                }
                Ok(_) => {}
                Err(error) => diff.error = Some(super::friendly_error(&error.to_string(), false)),
            }
        }
        cx.notify();
    }

    /// Before panes close, asks whether to save the editors among them with
    /// unsaved changes, then goes on closing.
    pub(in crate::model) fn check_editor_closes(&mut self, cx: &mut Context<Self>) {
        let Some(request) = &self.close_request else {
            return;
        };
        let dirty: Vec<_> = request
            .panes
            .iter()
            .copied()
            .filter(|pane| self.editor_dirty(*pane))
            .collect();
        if dirty.is_empty() || request.restart.is_some() {
            self.check_next_close(cx);
            return;
        }
        let panes = request.panes.clone();
        let names: Vec<_> = dirty
            .iter()
            .filter_map(|pane| self.editors.get(pane))
            .map(|editor| {
                editor
                    .path
                    .rsplit('/')
                    .next()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect();
        let window = self.window;
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let answer = crate::views::confirm::prompt_unsaved(window, &names, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                if model
                    .close_request
                    .as_ref()
                    .is_none_or(|request| request.panes != panes)
                {
                    return;
                }
                match answer {
                    Unsaved::Save => model.save_then_close(dirty, &panes, cx),
                    Unsaved::Discard => model.check_next_close(cx),
                    Unsaved::Cancel => model.close_request = None,
                }
            });
        }));
    }

    fn save_then_close(&mut self, dirty: Vec<PaneId>, panes: &[PaneId], cx: &mut Context<Self>) {
        let remaining = std::rc::Rc::new(std::cell::Cell::new(dirty.len()));
        let failed = std::rc::Rc::new(std::cell::Cell::new(false));
        for pane in dirty {
            let (remaining, failed, panes) = (remaining.clone(), failed.clone(), panes.to_vec());
            self.save_editor(pane, cx, move |model, saved, cx| {
                failed.set(failed.get() || !saved);
                remaining.set(remaining.get() - 1);
                if remaining.get() > 0
                    || model
                        .close_request
                        .as_ref()
                        .is_none_or(|request| request.panes != panes)
                {
                    return;
                }
                if failed.get() {
                    model.close_request = None;
                } else {
                    model.check_next_close(cx);
                }
            });
        }
    }
}
