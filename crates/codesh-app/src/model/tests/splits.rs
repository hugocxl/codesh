use super::*;
use codesh_app_core::Direction;
use codesh_protocol::ChannelId;

fn split_state() -> (AppState, TabId, [PaneId; 3]) {
    let mut state = AppState::bootstrap().expect("state");
    let tab = state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right).expect("split");
    let third = state.split_pane(second, Direction::Down).expect("split");
    for (index, pane) in [first, second, third].into_iter().enumerate() {
        state
            .set_pane_session(pane, SessionId::new(100 + index as u64))
            .expect("session");
    }
    (state, tab, [first, second, third])
}

fn attach_panes(model: &mut AppModel, panes: &[PaneId], cx: &mut Context<AppModel>) {
    model.servers.local.connection = ConnectionState::Ready;
    for (index, pane) in panes.iter().enumerate() {
        let mut attachment = attachment();
        attachment.channel = ChannelId(u32::try_from(index + 1).expect("channel"));
        model.receive(
            (
                ServerId::local(),
                1,
                Update::Attached {
                    pane: *pane,
                    session: model.pane_session(*pane).expect("session"),
                    attachment,
                    created: false,
                },
            ),
            cx,
        );
    }
}

#[gpui::test]
fn zoom_and_project_switch_detach_only_hidden_leaves_and_restore_all(cx: &mut TestAppContext) {
    let (mut state, _, panes) = split_state();
    let home = state.home().id;
    let other = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.open_terminal_tab(other).expect("other container");
    state.select_project(home).expect("home");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        assert_eq!(model.grids.len(), 3);
        acknowledge_catalog(model, cx);
        attach_panes(model, &panes, cx);
        let _ = requests.try_iter().collect::<Vec<_>>();
        model.toggle_zoom_pane(cx);
        assert_eq!(model.visible_panes(), [panes[2]]);
        assert_eq!(model.grids.len(), 1);
        let detached: Vec<_> = requests
            .try_iter()
            .filter_map(|(_, work)| match work {
                Work::Detach(channel) => Some(channel),
                _ => None,
            })
            .collect();
        assert_eq!(detached.len(), 2);
        assert!(detached.contains(&ChannelId(1)) && detached.contains(&ChannelId(2)));
        model.toggle_zoom_pane(cx);
        assert_eq!(model.grids.len(), 3);
        assert_eq!(
            model
                .terminal(&panes[2])
                .expect("terminal")
                .view
                .read(cx)
                .channel(),
            Some(ChannelId(3))
        );
        for pane in &panes {
            model
                .terminal(pane)
                .expect("terminal")
                .view
                .update(cx, |pane, cx| {
                    pane.set_viewport(Size { cols: 30, rows: 12 }, cx);
                });
        }
        model.ensure_visible(cx);
        let attached: Vec<_> = requests
            .try_iter()
            .filter_map(|(_, work)| match work {
                Work::Attach { pane, session, .. } => Some((pane, session)),
                _ => None,
            })
            .collect();
        assert_eq!(attached.len(), 2);
        assert!(attached.contains(&(panes[0], model.pane_session(panes[0]))));
        assert!(attached.contains(&(panes[1], model.pane_session(panes[1]))));
        attach_panes(model, &panes, cx);
        let _ = requests.try_iter().collect::<Vec<_>>();
        model.select_project(other, cx);
        assert_eq!(model.grids.len(), 1);
        assert_eq!(
            requests
                .try_iter()
                .filter(|(_, work)| matches!(work, Work::Detach(_)))
                .count(),
            3
        );
        model.select_project(home, cx);
        assert_eq!(model.grids.len(), 3);
        assert_eq!(model.active_pane(), Some(panes[2]));
    });
}

#[gpui::test]
fn equalize_splits_evens_resized_panes_from_the_shortcut_and_the_terminal_menu(
    cx: &mut TestAppContext,
) {
    let (state, tab, panes) = split_state();
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    let resize = |model: &mut AppModel, cx: &mut Context<AppModel>| {
        model.state.set_ratio(tab, &[], 0.8).expect("ratio");
        model
            .state
            .set_ratio(tab, &[codesh_app_core::Branch::Second], 0.2)
            .expect("ratio");
        model.save_split_resize(cx);
    };
    let assert_even_and_saved = |model: &AppModel| {
        assert_eq!(
            model.state.home().tabs[0].layout.rects(),
            [
                (panes[0], [0.0, 0.0, 0.5, 1.0]),
                (panes[1], [0.5, 0.0, 0.5, 0.5]),
                (panes[2], [0.5, 0.5, 0.5, 0.5]),
            ]
        );
        assert_eq!(model.active_pane(), Some(panes[2]));
        assert_eq!(store::load(&model.path).expect("saved"), model.state);
    };
    view.update(cx, |model, cx| resize(model, cx));
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-ctrl-="
    } else {
        "ctrl-alt-="
    });
    view.read_with(cx, |model, _| assert_even_and_saved(model));

    view.update(cx, |model, cx| {
        resize(model, cx);
        model.terminal_menu(panes[0], gpui::point(px(100.0), px(100.0)), cx);
    });
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |model, _| model.menu_outline()[0][7].clone()),
        "Equalize Splits"
    );
    let equalize = cx.debug_bounds("menu-item-7").expect("Equalize Splits");
    cx.simulate_click(equalize.center(), Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert_even_and_saved(model));
}

fn scroll_first_pane(model: &mut AppModel, panes: &[PaneId], cx: &mut Context<AppModel>) {
    let scrolled = model.terminal(&panes[0]).expect("terminal").view.clone();
    scrolled.update(cx, |pane, cx| pane.set_viewport(attachment().grid.size, cx));
    attach_panes(model, panes, cx);
    scrolled.update(cx, |pane, cx| {
        let grid = pane.grid.as_mut().expect("grid");
        grid.history = (0..10)
            .map(|index| codesh_protocol::Row {
                index,
                runs: vec![codesh_protocol::Run {
                    text: index.to_string(),
                    width: 1,
                    style: codesh_protocol::Style::default(),
                }],
            })
            .collect();
        grid.history_total = 10;
        pane.scroll_rows(4.0, cx);
    });
}

#[gpui::test]
fn a_pane_hidden_by_zoom_comes_back_scrolled_where_it_was_left(cx: &mut TestAppContext) {
    let (state, _, panes) = split_state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let top_row = |model: &AppModel, pane: PaneId, cx: &gpui::App| {
            let pane = model.terminal(&pane).expect("terminal").view.read(cx);
            let grid = pane.displayed_grid().expect("grid");
            grid.content_row(pane.visible_start(grid))
                .expect("row")
                .to_vec()
        };
        scroll_first_pane(model, &panes, cx);
        let left = top_row(model, panes[0], cx);
        assert_eq!(left[0].text, "6");
        model.toggle_zoom_pane(cx);
        assert_eq!(model.grids.len(), 1);
        model.toggle_zoom_pane(cx);
        assert_eq!(top_row(model, panes[0], cx), left);
        attach_panes(model, &panes, cx);
        assert_eq!(top_row(model, panes[0], cx), left);
        assert_eq!(top_row(model, panes[1], cx)[0].text, "final marker");
        assert!(model.scrolls.is_empty());
    });
}

#[gpui::test]
fn scroll_on_output_notices_output_attached_while_the_pane_was_hidden_by_zoom(
    cx: &mut TestAppContext,
) {
    let (state, _, panes) = split_state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        scroll_first_pane(model, &panes, cx);
        model.terminal.options.scroll_on_output = true;
        model.toggle_zoom_pane(cx);
        assert!(!model.grids.contains_key(&panes[0]));
        assert!(model.scrolls.contains_key(&panes[0]));
        let session = model.pane_session(panes[0]).expect("session");
        let mut output = attachment();
        output.grid.rows[1] = output.grid.rows[0].clone();
        let attached = || Update::Attached {
            pane: panes[0],
            session,
            attachment: output.clone(),
            created: false,
        };
        model.receive((ServerId::local(), 1, attached()), cx);
        model.toggle_zoom_pane(cx);
        model.receive((ServerId::local(), 1, attached()), cx);
        let pane = model.terminal(&panes[0]).expect("terminal").view.read(cx);
        assert!(pane.channel().is_some());
        assert!(pane.scroll.view.is_none());
    });
}

#[gpui::test]
fn closing_one_pane_discards_only_its_session_and_last_closes_tab(cx: &mut TestAppContext) {
    let (state, _, panes) = split_state();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        attach_panes(model, &panes, cx);
        let _ = requests.try_iter().collect::<Vec<_>>();
        model.close_pane(panes[1], cx);
        acknowledge_close(model, cx);
        assert_eq!(model.state.home().tabs[0].panes.len(), 2);
        assert_eq!(model.active_pane(), Some(panes[2]));
        assert_eq!(
            model.state.pending_discards(ServerId::local()),
            [SessionId::new(101).expect("session")]
        );
        let discarded: Vec<_> = requests
            .try_iter()
            .filter_map(|(_, work)| match work {
                Work::Discard(session, _) => Some(session),
                _ => None,
            })
            .collect();
        assert_eq!(discarded, model.state.pending_discards(ServerId::local()));
        let loaded = store::load(&model.path).expect("saved");
        assert_eq!(loaded.home().tabs[0].layout.leaves(), [panes[0], panes[2]]);
        model.close_pane(panes[2], cx);
        acknowledge_close(model, cx);
        assert_eq!(model.active_pane(), Some(panes[0]));
        model.close_pane(panes[0], cx);
        acknowledge_close(model, cx);
        assert!(model.state.home().tabs.is_empty());
        assert!(model.grids.is_empty());
    });
}

#[gpui::test]
fn closing_tab_checks_every_hidden_pane_and_cancel_keeps_all(cx: &mut TestAppContext) {
    let (state, tab, panes) = split_state();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        attach_panes(model, &panes, cx);
        model.toggle_zoom_pane(cx);
        model.close_tab(tab, cx);
        assert_eq!(model.pending_close, Some(tab));
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::CheckClose { session, .. } if Some(session) == SessionId::new(100))));
        model.receive_close_checked(tab, SessionId::new(100).expect("session"), Ok(None), cx);
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::CheckClose { session, .. } if Some(session) == SessionId::new(101))));
        model.receive_close_checked(tab, SessionId::new(101).expect("session"), Ok(Some(codesh_protocol::ForegroundProcess { name: "vim".into(), is_shell: false })), cx);
    });
    cx.run_until_parked();
    assert!(cx.has_pending_prompt());
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.home().tabs[0].panes.len(), 3);
        assert!(model.state.pending_discards(ServerId::local()).is_empty());
    });
}

#[gpui::test]
fn split_directory_inherits_only_when_configured_and_falls_back_to_project(
    cx: &mut TestAppContext,
) {
    for (setting, reported, inherit) in [
        (
            codesh_app_core::settings::NewPaneDirectory::Project,
            b"/tmp".as_slice(),
            false,
        ),
        (
            codesh_app_core::settings::NewPaneDirectory::Current,
            b"/tmp".as_slice(),
            true,
        ),
        (
            codesh_app_core::settings::NewPaneDirectory::Current,
            b"".as_slice(),
            false,
        ),
        (
            codesh_app_core::settings::NewPaneDirectory::Current,
            b"relative".as_slice(),
            false,
        ),
    ] {
        let mut state = AppState::bootstrap().expect("state");
        state.open_terminal_tab(state.home().id).expect("tab");
        let original = state.home().tabs[0].panes[0].id;
        state
            .set_pane_session(original, SessionId::new(100))
            .expect("session");
        let (mut boot, requests) = stub_boot(state);
        boot.settings.panes.new_pane_directory = setting;
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| {
            model.servers.local.connection = ConnectionState::Ready;
            let mut data = attachment();
            data.directory = codesh_protocol::ServerPath(reported.to_vec());
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane: original,
                        session: SessionId::new(100).expect("session"),
                        attachment: data,
                        created: false,
                    },
                ),
                cx,
            );
            let _ = requests.try_iter().collect::<Vec<_>>();
            model.split_pane(Direction::Right, cx);
            let new = model.active_pane().expect("split");
            let directory = requests
                .try_iter()
                .find_map(|(_, work)| match work {
                    Work::Attach {
                        pane,
                        directory,
                        session: None,
                        ..
                    } if pane == new => Some(directory),
                    _ => None,
                })
                .expect("creation request");
            assert_eq!(
                directory,
                if inherit {
                    PathBuf::from("/tmp")
                } else {
                    model.state.current_project().directory.clone()
                }
            );
            model.close_pane(new, cx);
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane: new,
                        session: SessionId::new(101).expect("session"),
                        attachment: attachment(),
                        created: true,
                    },
                ),
                cx,
            );
            assert!(
                model
                    .state
                    .pending_discards(ServerId::local())
                    .contains(&SessionId::new(101).expect("session"))
            );
            assert!(model.initial_directories.is_empty());
        });
    }
}

#[gpui::test]
fn card_controls_target_their_own_pane_and_keep_the_close_workflow(cx: &mut TestAppContext) {
    let result = (|| -> Result {
        let (state, tab, panes) = split_state();
        let (boot, _requests) = stub_boot(state);
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        view.update(cx, |model, cx| attach_panes(model, &panes, cx));
        cx.run_until_parked();
        let header = cx
            .debug_bounds(format!("pane-header-{}", panes[0]).leak())
            .ok_or("missing pane header")?;
        cx.simulate_mouse_move(header.center(), None, Modifiers::default());
        cx.run_until_parked();
        let split = cx
            .debug_bounds(format!("pane-{}-right", panes[0]).leak())
            .ok_or("missing split control")?;
        cx.simulate_click(split.center(), Modifiers::default());
        cx.run_until_parked();
        view.read_with(cx, |model, _| -> Result {
            let layout = &model.tab(tab).ok_or("missing tab")?.layout;
            // Equalization may rebalance the binary tree; the new pane must
            // still be immediately right of the control's pane, with the
            // opposite-axis subtree left intact.
            let leaves = layout.leaves();
            assert_eq!(leaves.len(), 4);
            assert_eq!(leaves[0], panes[0]);
            assert_eq!(&leaves[2..], &panes[1..]);
            assert!(!panes.contains(&leaves[1]));
            assert_eq!(model.active_pane(), Some(leaves[1]));
            Ok(())
        })?;
        // The split focuses its new pane. Hover the original card again to
        // reveal its controls without changing the active pane.
        let header_after_split = cx
            .debug_bounds(format!("pane-header-{}", panes[0]).leak())
            .ok_or("missing original pane header")?;
        assert_eq!(header.size.height, header_after_split.size.height);
        cx.simulate_mouse_move(header_after_split.center(), None, Modifiers::default());
        cx.run_until_parked();
        let zoom = cx
            .debug_bounds(format!("pane-{}-zoom", panes[0]).leak())
            .ok_or("missing zoom control")?;
        cx.simulate_click(zoom.center(), Modifiers::default());
        cx.run_until_parked();
        view.read_with(cx, |model, _| -> Result {
            assert_eq!(model.tab(tab).ok_or("missing tab")?.zoomed, Some(panes[0]));
            Ok(())
        })?;
        let close = cx
            .debug_bounds(format!("pane-{}-close", panes[0]).leak())
            .ok_or("missing close control")?;
        cx.simulate_click(close.center(), Modifiers::default());
        cx.run_until_parked();
        view.update(cx, |model, cx| -> Result {
            acknowledge_close(model, cx);
            let leaves = model.tab(tab).ok_or("missing tab")?.layout.leaves();
            assert!(!leaves.contains(&panes[0]));
            assert!(leaves.contains(&panes[1]) && leaves.contains(&panes[2]));
            assert_eq!(leaves.len(), 3);
            Ok(())
        })
    })();
    assert!(result.is_ok(), "{result:?}");
}

#[gpui::test]
fn pane_focused_by_direction_takes_input_and_draws_its_output(cx: &mut TestAppContext) {
    let (state, _, panes) = split_state();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        acknowledge_catalog(model, cx);
        attach_panes(model, &panes, cx);
    });
    cx.run_until_parked();
    let modifier = if cfg!(target_os = "macos") {
        "cmd-alt"
    } else {
        "alt"
    };
    for key in ["left", "right", "down", "up", "left"] {
        let _ = requests.try_iter().collect::<Vec<_>>();
        cx.simulate_keystrokes(&format!("{modifier}-{key}"));
        // Releasing the modifiers re-renders the workspace without the pane.
        cx.simulate_modifiers_change(gpui::Modifiers::alt());
        cx.simulate_modifiers_change(gpui::Modifiers::none());
        cx.run_until_parked();
        let active = view
            .read_with(cx, |model, _| model.active_pane())
            .expect("active");
        let channel = ChannelId(
            u32::try_from(panes.iter().position(|pane| *pane == active).expect("pane") + 1)
                .expect("channel"),
        );
        cx.simulate_input("x");
        cx.run_until_parked();
        assert!(
            requests.try_iter().any(
                |(_, work)| matches!(work, Work::Input(sent, bytes) if sent == channel && bytes == b"x")
            ),
            "typing after {key} missed the focused pane"
        );
        let renders = |cx: &mut gpui::VisualTestContext| {
            view.read_with(cx, |model, cx| {
                model.grids[&active].view.read(cx).render_count
            })
        };
        let before = renders(cx);
        view.update(cx, |model, cx| {
            let output = ClientEvent::Frame {
                channel,
                frame: frame_with("x"),
            };
            model.receive((ServerId::local(), 1, Update::Event(output)), cx);
        });
        cx.run_until_parked();
        assert!(renders(cx) > before, "echo after {key} was not drawn");
    }
}

fn frame_with(text: &str) -> codesh_protocol::ScreenFrame {
    codesh_protocol::ScreenFrame {
        size: Size { cols: 80, rows: 24 },
        graphics: None,
        seq: 7,
        reset: false,
        rows: vec![codesh_protocol::Row {
            index: 0,
            runs: vec![codesh_protocol::Run {
                text: text.into(),
                width: u16::try_from(text.len()).expect("width"),
                style: codesh_protocol::Style::default(),
            }],
        }],
        cursor: codesh_protocol::Cursor {
            shape: codesh_protocol::CursorShape::default(),
            row: 0,
            col: 0,
            visible: true,
        },
        modes: codesh_protocol::Modes::default(),
    }
}
