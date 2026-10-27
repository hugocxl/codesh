use codesh_app_core::{AppState, Branch, Direction, Layout, PaneId, ServerId, restore};
use codesh_protocol::{ServerPath, SessionId, SessionInfo};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn nested_splits_focus_zoom_and_collapse_survive_roundtrip() -> Result {
    let mut state = AppState::bootstrap()?;
    let tab = state.open_terminal_tab(state.home().id)?;
    let left = state.home().tabs[0].panes[0].id;
    let right = state.split_pane(left, Direction::Right)?;
    let bottom = state.split_pane(right, Direction::Down)?;
    assert_eq!(state.home().tabs[0].layout.leaves(), [left, right, bottom]);
    assert_eq!(state.neighbor(left, Direction::Right), Some(right));
    assert_eq!(state.neighbor(right, Direction::Down), Some(bottom));
    assert_eq!(state.neighbor(bottom, Direction::Up), Some(right));
    assert_eq!(state.neighbor(bottom, Direction::Left), Some(left));
    assert_eq!(state.neighbor(left, Direction::Left), None);
    assert_eq!(state.neighbor(right, Direction::Right), None);
    state.set_ratio(tab, &[], 0.6)?;
    state.set_ratio(tab, &[Branch::Second], 0.3)?;
    state.focus_pane(right)?;
    state.set_pane_title(right, "editor")?;
    assert_eq!(
        state.home().tabs[0].title(state.window().active_pane),
        "editor"
    );
    state.toggle_zoom(right)?;
    assert_eq!(state.home().tabs[0].visible_panes(), [right]);
    let restored: AppState = serde_json::from_value(serde_json::to_value(&state)?)?;
    assert_eq!(restored, state);
    state.focus_pane(bottom)?;
    assert_eq!(state.home().tabs[0].visible_panes(), [bottom]);
    state.toggle_zoom(bottom)?;
    assert_eq!(state.home().tabs[0].visible_panes(), [left, right, bottom]);
    state.close_pane(right)?;
    assert_eq!(state.home().tabs[0].layout.leaves(), [left, bottom]);
    for (_, rect) in state.home().tabs[0].layout.rects() {
        assert!((rect[2] - 0.5).abs() < 1e-5);
    }
    assert_eq!(state.window().active_pane, Some(bottom));
    state.toggle_zoom(bottom)?;
    state.close_pane(bottom)?;
    assert_eq!(state.home().tabs[0].layout, Layout::Leaf(left));
    assert_eq!(state.window().active_pane, Some(left));
    assert_eq!(state.home().tabs[0].zoomed, None);
    state.close_pane(left)?;
    assert!(state.home().tabs.is_empty());
    assert!(state.window().selected_tab.is_empty());
    Ok(())
}

#[test]
fn equalizing_splits_evens_a_long_row_and_keeps_the_zoom() -> Result {
    let mut state = AppState::bootstrap()?;
    state.open_terminal_tab(state.home().id)?;
    let first = state.home().tabs[0].panes[0].id;
    let mut last = first;
    for _ in 0..9 {
        last = state.split_pane(last, Direction::Right)?;
    }
    let panes = state.home().tabs[0].layout.leaves();
    let tab_id = state.home().tabs[0].id;
    state.set_ratio(tab_id, &[], 0.7)?;
    state.toggle_zoom(last)?;
    state.equalize_splits(first)?;
    let tab = &state.home().tabs[0];
    assert_eq!(tab.zoomed, Some(last));
    assert_eq!(tab.layout.leaves(), panes);
    let mut left = 0.0;
    for (pane, [x, y, width, height]) in tab.layout.rects() {
        for (actual, expected) in [(x, left), (y, 0.0), (width, 0.1), (height, 1.0)] {
            assert!((actual - expected).abs() < 1e-4, "{pane:?}: {actual}");
        }
        left += 0.1;
    }
    let restored: AppState = serde_json::from_value(serde_json::to_value(&state)?)?;
    assert_eq!(restored, state);
    let before = state.clone();
    assert!(state.equalize_splits(PaneId::new()).is_err());
    assert_eq!(state, before);
    Ok(())
}

#[test]
fn missing_and_malformed_layouts_are_rejected() -> Result {
    let mut state = AppState::bootstrap()?;
    state.open_terminal_tab(state.home().id)?;
    let mut legacy = serde_json::to_value(&state)?;
    let tab = legacy["projects"][0]["tabs"][0]
        .as_object_mut()
        .ok_or("tab")?;
    tab.remove("layout");
    tab.remove("zoomed");
    assert!(serde_json::from_value::<AppState>(legacy).is_err());
    let first = state.home().tabs[0].panes[0].id;
    state.split_pane(first, Direction::Right)?;
    let valid = serde_json::to_value(&state)?;
    for (pointer, replacement) in [
        (
            "/layout/Split/first",
            serde_json::to_value(Layout::Leaf(PaneId::new()))?,
        ),
        (
            "/layout/Split/second",
            serde_json::to_value(Layout::Leaf(first))?,
        ),
        ("/layout/Split/ratio", serde_json::json!(1.5)),
        ("/zoomed", serde_json::to_value(PaneId::new())?),
        ("/panes", serde_json::json!([])),
    ] {
        let mut invalid = valid.clone();
        *invalid["projects"][0]["tabs"][0]
            .pointer_mut(pointer)
            .ok_or("pointer")? = replacement;
        assert!(
            serde_json::from_value::<AppState>(invalid).is_err(),
            "{pointer}"
        );
    }
    Ok(())
}

#[test]
fn restore_accounts_for_each_leaf_even_when_zoomed() -> Result {
    let mut state = AppState::bootstrap()?;
    state.open_terminal_tab(state.home().id)?;
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right)?;
    let third = state.split_pane(second, Direction::Down)?;
    let live = SessionId::new(23).ok_or("session")?;
    let ended = SessionId::new(24).ok_or("session")?;
    state.set_pane_session(first, Some(live))?;
    state.set_pane_session(second, Some(ended))?;
    state.toggle_zoom(third)?;
    let plan = restore::plan(
        &state,
        ServerId::local(),
        &[SessionInfo {
            project: state.home().id,
            id: live,
            directory: ServerPath(b"/tmp".to_vec()),
        }],
    );
    assert_eq!(plan.attach, [(first, live)]);
    assert_eq!(plan.close, [(second, ended)]);
    assert_eq!(plan.create, [third]);
    Ok(())
}

#[test]
fn window_owns_focus_and_closing_chooses_neighbors_without_stealing_focus() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let first_tab = state.open_terminal_tab(home)?;
    let first = state.window().active_pane.ok_or("focus")?;
    let middle = state.split_pane(first, Direction::Right)?;
    let last = state.split_pane(middle, Direction::Right)?;
    state.close_pane(last)?;
    assert_eq!(state.window().active_pane, Some(middle));
    let container = state.open_terminal_tab(home)?;
    assert_eq!(container, first_tab);
    let added = state.window().active_pane.ok_or("focus")?;
    let right = state.split_pane(added, Direction::Right)?;
    state.focus_pane(middle)?;
    state.close_pane(first)?;
    assert_eq!(state.window().active_pane, Some(middle));
    let other = state.add_project(ServerId::local(), std::env::current_dir()?)?;
    state.open_terminal_tab(other)?;
    let other_pane = state.window().active_pane;
    state.close_pane(right)?;
    assert_eq!(state.window().active_pane, other_pane);
    state.close_tab(home, container)?;
    assert_eq!(state.window().active_pane, other_pane);
    assert!(state.home().tabs.is_empty());
    Ok(())
}

#[test]
fn closing_tabs_in_a_missing_project_keeps_saved_window_state_loadable() -> Result {
    let mut state = AppState::bootstrap()?;
    let directory = std::env::temp_dir().join(format!("codesh-missing-{}", PaneId::new()));
    std::fs::create_dir(&directory)?;
    let project = state.add_project(ServerId::local(), directory.clone())?;
    let container = state.open_terminal_tab(project)?;
    assert_eq!(state.open_terminal_tab(project)?, container);
    std::fs::remove_dir(&directory)?;
    state.refresh_project_statuses();
    state.close_tab(project, container)?;
    let restored: AppState = serde_json::from_value(serde_json::to_value(&state)?)?;
    assert_eq!(restored, state);
    assert!(state.project(project).ok_or("project")?.tabs.is_empty());
    assert_eq!(state.window().active_pane, None);
    let restored: AppState = serde_json::from_value(serde_json::to_value(&state)?)?;
    assert_eq!(restored, state);
    Ok(())
}
