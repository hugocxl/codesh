use codesh_app_core::{AppState, Direction, GroupId, Layout, PaneId, TabId};
use serde_json::json;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn tab_group_drag_routes_do_not_change_container_or_focus() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let tab = state.open_terminal_tab(home)?;
    state.open_terminal_tab(home)?;
    let before = state.clone();
    for direction in [
        Direction::Left,
        Direction::Right,
        Direction::Up,
        Direction::Down,
    ] {
        state.split_tab(tab, tab, direction)?;
        state.move_tab_to_group(tab, tab, Some(tab))?;
        assert_eq!(state, before);
        assert!(state.home().groups().is_none());
    }
    Ok(())
}

#[test]
fn persisted_tab_groups_merge_without_losing_panes_or_focus() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let first = state.open_terminal_tab(home)?;
    let mut second = state.home().tabs[0].clone();
    second.id = TabId::new();
    second.panes[0].id = PaneId::new();
    second.layout = Layout::Leaf(second.panes[0].id);
    let second_id = second.id;
    let pane = second.panes[0].id;
    let a = GroupId::new();
    let b = GroupId::new();
    let mut stored = serde_json::to_value(&state)?;
    stored["projects"][0]["tabs"]
        .as_array_mut()
        .ok_or("tabs")?
        .push(serde_json::to_value(second)?);
    stored["projects"][0]["groups"] = json!({
        "layout": {"Split": {"axis": "horizontal", "ratio": 0.3, "first": {"Leaf": a}, "second": {"Leaf": b}}},
        "groups": [{"id": a, "tabs": [first], "selected": first}, {"id": b, "tabs": [second_id], "selected": second_id}]
    });
    stored["window"]["selected_tab"][home.to_string()] = json!(second_id);
    stored["window"]["active_pane"] = json!(pane);
    let loaded: AppState = serde_json::from_value(stored)?;
    assert!(loaded.home().groups().is_none());
    assert_eq!(loaded.home().tabs.len(), 1);
    assert_eq!(loaded.home().tabs[0].panes.len(), 2);
    assert_eq!(loaded.window().active_pane, Some(pane));
    assert_eq!(loaded.window().selected_tab[&home], first);
    Ok(())
}
