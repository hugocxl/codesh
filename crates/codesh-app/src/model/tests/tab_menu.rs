use super::*;

fn fixture() -> (AppState, [TabId; 4]) {
    let mut state = AppState::bootstrap().expect("state");
    let ids = std::array::from_fn(|_| state.open_terminal_tab(state.home().id).expect("tab"));
    state.select_tab(state.home().id, ids[2]).expect("select");
    (state, ids)
}

#[gpui::test]
fn closing_container_confirms_once_and_cancellation_preserves_every_pane(cx: &mut TestAppContext) {
    let (mut state, ids) = fixture();
    for index in [0, 1, 3] {
        let pane = state.home().tabs[0].panes[index].id;
        state
            .set_pane_session(pane, SessionId::new(index as u64 + 10))
            .expect("session");
    }
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for answer in ["Cancel", "Close"] {
        view.update(cx, |model, cx| {
            model.servers.local.connection = ConnectionState::Ready;
            model.close_tab(ids[2], cx);
            let request = model.close_request.as_ref().expect("request");
            let tab = request.tab;
            let session = model.pane_session(request.panes[0]).expect("session");
            model.receive_close_checked(
                tab,
                session,
                Ok(Some(codesh_protocol::ForegroundProcess {
                    name: "vim".into(),
                    is_shell: false,
                })),
                cx,
            );
        });
        cx.run_until_parked();
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer(answer);
        cx.run_until_parked();
        assert!(!cx.has_pending_prompt());
        view.read_with(cx, |model, _| {
            assert_eq!(
                model.state.home().tabs.len(),
                usize::from(answer == "Cancel")
            );
            assert_eq!(model.active_tab(), (answer == "Cancel").then_some(ids[2]));
            if answer == "Cancel" {
                assert_eq!(model.state.home().tabs[0].panes.len(), 4);
            }
            assert!(model.close_request.is_none());
        });
    }
}

#[gpui::test]
fn tab_edits_and_container_close_roll_back_on_save_failure(cx: &mut TestAppContext) {
    let (state, ids) = fixture();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let previous = model.state.clone();
        let path = model.path.clone();
        let blocked = path.with_file_name("blocked");
        std::fs::create_dir_all(blocked.parent().expect("parent")).expect("directory");
        std::fs::write(&blocked, "file").expect("blocked");
        model.path = blocked.join("state.json");
        assert!(!model.edit_tab(|state| state.toggle_tab_pin(ids[0]), cx));
        assert_eq!(model.state, previous);
        model.close_tab(ids[2], cx);
        assert_eq!(model.state, previous);
        model.path = path;
    });
}
