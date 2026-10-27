use super::*;
use codesh_app_core::opener::{FileLocation, OpenContext};
use codesh_app_core::settings::SidePanel;
use codesh_protocol::{
    FileContent, FilesAction, FilesReply, GitAction, GitReply, GitRequest, ServerPath,
};
use std::cell::RefCell;

use crate::model::native::{EditorPane, Stub};
use crate::views::git_panel::GitPanelEvent;

type Writes = Rc<RefCell<Vec<(String, String)>>>;

/// A server whose files all read as `text` and that records writes.
fn stub_files(text: &'static str) -> (Rc<Stub>, Writes) {
    let writes = Writes::default();
    let recorded = writes.clone();
    let stub = Stub {
        files: Box::new(move |request| match request.action {
            FilesAction::Read(path) => Ok(FilesReply::Content(FileContent {
                path,
                content: text.into(),
                size: text.len() as u64,
            })),
            FilesAction::Write { path, content } => {
                recorded
                    .borrow_mut()
                    .push((String::from_utf8_lossy(&path.0).into_owned(), content));
                Ok(FilesReply::Path(path))
            }
            FilesAction::List(_) => Ok(FilesReply::Entries(Vec::new())),
            _ => Ok(FilesReply::Done),
        }),
        exec: Box::new(|_| Err("no commands here".into())),
    };
    (Rc::new(stub), writes)
}

fn editor_path(model: &AppModel, pane: PaneId) -> Option<String> {
    model
        .state
        .projects()
        .iter()
        .flat_map(|project| &project.tabs)
        .flat_map(|tab| &tab.panes)
        .find(|candidate| candidate.id == pane)
        .and_then(|pane| match &pane.content {
            PaneContent::Editor { path } => Some(path.clone()),
            _ => None,
        })
}

#[gpui::test]
fn opening_a_file_loads_an_editor_pane_and_saving_writes_it(cx: &mut TestAppContext) {
    let (state, project) = git::registered_current_project();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let (stub, writes) = stub_files("fn main() {}\n");
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.native.stub = Some(stub);
        model.open_project_file(project, "src/main.rs", Some((1, 4)), cx);
    });
    cx.run_until_parked();
    let (pane, editor) = view.update(cx, |model, cx| {
        let pane = model.active_pane().expect("editor pane");
        assert_eq!(editor_path(model, pane).as_deref(), Some("src/main.rs"));
        let editor = model
            .editors
            .get(&pane)
            .and_then(EditorPane::view)
            .expect("content loaded")
            .clone();
        assert_eq!(editor.read(cx).text(), "fn main() {}\n");
        assert!(!model.editor_dirty(pane));
        (pane, editor)
    });
    // Opening it again focuses the same pane instead of another.
    view.update(cx, |model, cx| {
        model.open_project_file(project, "src/main.rs", None, cx);
        assert_eq!(model.active_pane(), Some(pane));
    });
    editor.update(cx, |_, cx| {
        cx.emit(crate::views::editor::EditorEvent::SaveRequested);
    });
    cx.run_until_parked();
    assert_eq!(
        *writes.borrow(),
        [("src/main.rs".to_owned(), "fn main() {}\n".to_owned())]
    );
}

#[gpui::test]
fn binary_files_show_a_message_instead_of_an_editor(cx: &mut TestAppContext) {
    let (state, project) = git::registered_current_project();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let stub = Stub {
        files: Box::new(|_| Err("File is not valid UTF-8".into())),
        exec: Box::new(|_| Err("no commands here".into())),
    };
    view.update(cx, |model, cx| {
        model.native.stub = Some(Rc::new(stub));
        model.open_project_file(project, "logo.png", None, cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        let pane = model.active_pane().expect("editor pane");
        let editor = model.editors.get(&pane).expect("editor");
        assert!(editor.view().is_none());
        assert!(matches!(
            editor.content,
            crate::model::native::EditorContent::Message(_)
        ));
    });
}

#[gpui::test]
fn staging_from_the_source_control_panel_asks_git(cx: &mut TestAppContext) {
    let (state, project) = git::registered_current_project();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let panel = view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.appearance.side_panel = SidePanel::SourceControl;
        model.sync_side_panel(cx);
        model.native.git.get(&project).expect("panel").0.clone()
    });
    panel.update(cx, |_, cx| {
        cx.emit(GitPanelEvent::Stage(vec!["notes.txt".into()]));
    });
    cx.run_until_parked();
    let mut staged = None;
    // Earlier reads finish first; the change waits for them.
    for _ in 0..16 {
        let sent: Vec<GitRequest> = requests
            .try_iter()
            .filter_map(|(_, work)| match work {
                Work::Git(request) => Some(request),
                _ => None,
            })
            .collect();
        if let Some(request) = sent
            .iter()
            .find(|request| matches!(request.action, GitAction::Stage(_)))
        {
            staged = Some(request.clone());
            break;
        }
        view.update(cx, |model, cx| {
            for request in &sent {
                model.receive_git(request, Ok(GitReply::Done), cx);
            }
        });
        cx.run_until_parked();
    }
    let staged = staged.expect("stage request");
    assert_eq!(staged.project, project);
    assert_eq!(
        staged.action,
        GitAction::Stage(vec![ServerPath(b"notes.txt".to_vec())])
    );
}

#[gpui::test]
fn terminal_links_to_project_files_open_the_editor(cx: &mut TestAppContext) {
    let (state, project) = git::registered_current_project();
    let directory = state.project(project).expect("project").directory.clone();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let (stub, _) = stub_files("one\ntwo\nthree\n");
    let context = OpenContext {
        project,
        pane: PaneId::new(),
        server: ServerId::local(),
        directory: directory.clone(),
        project_directory: directory.clone(),
    };
    let link = |path: &str| FileLocation {
        path: directory.join(path),
        line: Some(3),
        column: Some(2),
    };
    view.update(cx, |model, cx| {
        model.native.stub = Some(stub);
        model.settings.openers.file = "system.editor".into();
        assert!(!model.open_file_link(&context, &link("src/lib.rs"), cx));
        model.settings.openers.file = codesh_app_core::settings::CODESH_EDITOR.into();
        assert!(model.open_file_link(&context, &link("src/lib.rs"), cx));
        let pane = model.active_pane().expect("editor pane");
        assert_eq!(editor_path(model, pane).as_deref(), Some("src/lib.rs"));
        let outside = FileLocation {
            path: PathBuf::from("/elsewhere/file.rs"),
            line: None,
            column: None,
        };
        assert!(!model.open_file_link(&context, &outside, cx));
    });
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        let pane = model.active_pane().expect("editor pane");
        let editor = model.editors.get(&pane).and_then(EditorPane::view);
        assert_eq!(editor.expect("loaded").read(cx).text(), "one\ntwo\nthree\n");
    });
}

#[gpui::test]
fn files_panel_retries_when_its_server_side_connection_opens(cx: &mut TestAppContext) {
    let (state, project) = git::registered_current_project();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let requests = Rc::new(RefCell::new(Vec::new()));
    view.update(cx, |model, cx| {
        // The initial deferred listing cannot reach a side connection yet.
        model.files_panel(project, cx);
    });
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        let recorded = requests.clone();
        model.native.stub = Some(Rc::new(Stub {
            files: Box::new(move |request| {
                recorded.borrow_mut().push(request.action);
                Ok(FilesReply::Entries(Vec::new()))
            }),
            exec: Box::new(|_| Err("unused".into())),
        }));
        model.native_side_ready(ServerId::local(), cx);
    });
    cx.run_until_parked();
    assert!(requests.borrow().iter().any(|action| matches!(action,
        FilesAction::List(path) if path.0.is_empty())));
}
