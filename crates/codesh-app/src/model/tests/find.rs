use super::*;
use crate::views::terminal::find::Results;
use crate::views::terminal::pane::TerminalPane;
use codesh_protocol::{ErrorCode, HistoryCursor, SearchMatch, SearchPage};

fn found(row: u64) -> SearchMatch {
    SearchMatch {
        row,
        start: 0,
        end: 1,
    }
}

fn page(matches: Vec<SearchMatch>, next: Option<HistoryCursor>, total_rows: u64) -> SearchPage {
    SearchPage {
        matches,
        next,
        total_rows,
        scanned_rows: 2000,
    }
}

#[test]
fn search_replies_ignore_old_queries_continue_empty_pages_and_restart_stale_cursors() {
    let mut results = Results::default();
    results.query = "a".into();
    let old = results.restart().expect("request");
    results.query = "b".into();
    let fresh = results.restart().expect("request");
    assert!(
        results
            .receive(&old, Ok(page(vec![found(1)], None, 50)))
            .is_none()
    );
    assert!(results.matches.is_empty());
    let older = results
        .receive(&fresh, Ok(page(vec![], Some(HistoryCursor(42)), 50)))
        .expect("continue empty page");
    let restart = results
        .receive(
            &older,
            Err(codesh_client::ClientError::Server(
                codesh_protocol::ErrorReply {
                    code: ErrorCode::StaleHistoryCursor,
                    message: "changed".into(),
                },
            )),
        )
        .expect("restart");
    assert_eq!(restart.before, HistoryCursor(0));
    assert_ne!(restart.token, older.token);
    results.receive(&restart, Ok(page(vec![found(49), found(2)], None, 50)));
    assert_eq!(results.counter(), "1 of 2");
    results.step(true);
    assert_eq!(results.current, Some(1));
    results.step(false);
    assert_eq!(results.current, Some(0));
}

fn pane<'a>(model: &'a AppModel, cx: &'a gpui::App) -> &'a TerminalPane {
    model
        .terminal(&model.active_pane().expect("pane"))
        .expect("terminal")
        .view
        .read(cx)
}

#[gpui::test]
fn find_searches_each_edit_immediately_ignores_case_and_keeps_input_out_of_the_shell(
    cx: &mut TestAppContext,
) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(vec![])), cx);
        acknowledge_catalog(model, cx);
        model.new_tab(cx);
        let pane = model.active_pane().expect("pane");
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Attached {
                    pane,
                    session: SessionId::new(42).expect("ID"),
                    attachment: attachment(),
                    created: true,
                },
            ),
            cx,
        );
    });
    cx.run_until_parked();
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-f"
    } else {
        "ctrl-shift-f"
    });
    cx.run_until_parked();
    requests.try_iter().for_each(drop);
    for expected in ["f", "fi", "fin", "fina", "final"] {
        type_text(cx, &expected[expected.len() - 1..]);
        let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
        assert!(!work.iter().any(|work| matches!(work, Work::Input(..))));
        let (pane_id, request) = work
            .into_iter()
            .find_map(|work| match work {
                Work::Search { pane, request, .. } => Some((pane, request)),
                _ => None,
            })
            .expect("typing sends a search without Enter or advancing the clock");
        assert_eq!(request.query, expected);
        assert!(request.ignore_case);
        view.update(cx, |model, cx| {
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Search {
                        pane: pane_id,
                        request,
                        result: Ok(page(vec![found(0)], None, 0)),
                    },
                ),
                cx,
            );
        });
        assert_eq!(
            view.read_with(cx, |model, cx| pane(model, cx)
                .find
                .as_ref()
                .expect("find")
                .results
                .counter()),
            "1 of 1"
        );
    }
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, cx| pane(model, cx).find.is_none()));
    type_text(cx, "x");
    assert!(
        requests
            .try_iter()
            .any(|(_, work)| matches!(work, Work::Input(_, bytes) if bytes == b"x"))
    );
}

#[gpui::test]
fn palette_find_debounces_edits_researches_options_and_opens_unicode_columns(
    cx: &mut TestAppContext,
) {
    use crate::model::native::Stub;
    use crate::views::command_palette::PaletteEntry;
    use codesh_core::shortcuts::ShortcutId;
    use codesh_protocol::{ExecResult, FileContent, FilesAction, FilesReply};
    use codesh_ui::command_palette::PaletteEvent;
    use std::cell::RefCell;

    let (state, _) = git::registered_current_project();
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let recorded = calls.clone();
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, _| {
        model.servers.local.connection = ConnectionState::Ready;
        model.native.stub = Some(Rc::new(Stub {
            exec: Box::new(move |request| {
                recorded.borrow_mut().push(request.argv.clone());
                Ok(ExecResult {
                    stdout: r#"{"type":"match","data":{"path":{"text":"src/main.rs"},"lines":{"text":"é foo\n"},"line_number":1,"submatches":[{"start":3}]}}"#.into(),
                    stderr: String::new(), exit_code: 0, timed_out: false, truncated: false, cancelled: false,
                })
            }),
            files: Box::new(|request| match request.action {
                FilesAction::Read(path) => Ok(FilesReply::Content(FileContent {
                    path, content: "é foo\n".into(), size: 7,
                })),
                FilesAction::List(_) => Ok(FilesReply::Entries(Vec::new())),
                _ => Ok(FilesReply::Done),
            }),
        }));
    });
    cx.simulate_keystrokes(ShortcutId::FindInFiles.definition().keys[0]);
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, AppModel::palette_page),
        Some(PaletteEntry::Find)
    );
    calls.borrow_mut().clear();
    cx.simulate_input("fo");
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.simulate_input("o");
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(249));
    cx.run_until_parked();
    assert!(calls.borrow().is_empty(), "each edit restarts the debounce");
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert_eq!(calls.borrow().len(), 1);
    assert_eq!(calls.borrow()[0].last().map(String::as_str), Some("foo"));
    view.read_with(cx, |model, cx| {
        let palette = model.palette_entity().expect("palette");
        let palette = palette.read(cx);
        let items = palette.items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "src/main.rs:1:4");
        assert_eq!(items[0].highlights.len(), 1);
        assert_eq!(items[0].highlights[0], 3..6);
    });
    view.update(cx, |model, cx| {
        let palette = model.palette_entity().expect("palette");
        palette.update(cx, |_, cx| {
            cx.emit(PaletteEvent::AccessoryToggled {
                page: "find".into(),
                id: "case".into(),
                active: true,
            });
        });
    });
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, cx| {
        model
            .palette_entity()
            .expect("palette")
            .read(cx)
            .items()
            .is_empty()
    }));
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.run_until_parked();
    assert_eq!(calls.borrow().len(), 2);
    assert!(!calls.borrow()[1].iter().any(|argument| argument == "-i"));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(view.read_with(cx, |model, _| model.overlay.is_none()));
    cx.simulate_input("X");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        let pane = model.active_pane().expect("editor");
        let editor = model.editors[&pane].view().expect("loaded editor");
        assert_eq!(
            editor.read(cx).text(),
            "é Xfoo\n",
            "the caret uses a character column"
        );
    });
}
