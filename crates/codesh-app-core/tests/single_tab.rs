use codesh_app_core::{
    AppState, Axis, Direction, Layout, Pane, PaneContent, PaneId, ServerId, Tab, TabId, TabSide,
    browser::BrowserPane, codesh, project_layouts::Config,
};
use codesh_protocol::SessionId;
use serde_json::json;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn active(state: &AppState) -> std::result::Result<PaneId, Box<dyn std::error::Error>> {
    state
        .window()
        .active_pane
        .ok_or_else(|| "active pane".into())
}

// Construct additional persisted tabs without widening the production constructor API.
fn new_tab(template: &Tab) -> Tab {
    let mut tab = template.clone();
    tab.id = TabId::new();
    tab.panes[0].id = PaneId::new();
    tab.layout = Layout::Leaf(tab.panes[0].id);
    tab
}

#[test]
fn terminal_and_adjacent_creation_split_the_focused_leaf() -> Result {
    let mut state = AppState::bootstrap()?;
    let project_id = state.add_project(ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project_id)?;
    let tab = state.open_terminal_tab(project_id)?;
    let first = active(&state)?;
    assert_eq!(state.open_terminal_tab(project_id)?, tab);
    let second = active(&state)?;
    assert_ne!(first, second);
    state.focus_pane(first)?;
    let mut expected = vec![first];
    for side in [TabSide::Left, TabSide::Right] {
        assert_eq!(
            state.open_terminal_tab_adjacent(project_id, tab, side)?,
            tab
        );
        expected.push(active(&state)?);
    }
    let fourth = active(&state)?;
    let container = &state.project(project_id).ok_or("project")?.tabs[0];
    assert_eq!(state.project(project_id).ok_or("project")?.tabs.len(), 1);
    assert_eq!(container.panes.len(), 4);
    expected.push(second);
    assert_eq!(container.layout.leaves(), expected);
    for ((pane, rect), (expected_pane, x)) in container
        .layout
        .rects()
        .into_iter()
        .zip(expected.into_iter().zip([0.0, 0.25, 0.5, 0.75]))
    {
        assert_eq!(pane, expected_pane);
        for (actual, expected) in rect.into_iter().zip([x, 0.0, 0.25, 1.0]) {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
    }
    assert!(container.layout.contains(fourth));
    Ok(())
}

#[test]
fn browser_background_opens_preserve_focus_and_foreground_opens_focus_new_pane() -> Result {
    let mut state = AppState::bootstrap()?;
    let project_id = state.add_project(ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project_id)?;
    let tab = state.open_terminal_tab(project_id)?;
    let first = active(&state)?;
    let (browser_tab, background) =
        state.open_browser(project_id, BrowserPane::default(), false)?;
    assert_eq!(browser_tab, tab);
    assert_eq!(active(&state)?, first);
    let (browser_tab, foreground) = state.open_browser(project_id, BrowserPane::default(), true)?;
    assert_eq!(browser_tab, tab);
    assert_eq!(active(&state)?, foreground);
    assert_ne!(background, foreground);
    let other = state.add_project(ServerId::local(), std::env::current_dir()?)?;
    state.open_terminal_tab(other)?;
    let window = state.window().clone();
    state.open_browser(project_id, BrowserPane::default(), false)?;
    assert_eq!(state.window(), &window);
    assert_eq!(state.project(project_id).ok_or("project")?.tabs.len(), 1);
    Ok(())
}

#[test]
fn missing_focus_splits_last_leaf_without_changing_window_focus() -> Result {
    let mut state = AppState::bootstrap()?;
    let project_id = state.add_project(ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project_id)?;
    state.open_terminal_tab(project_id)?;
    state.open_terminal_tab(project_id)?;
    let last = active(&state)?;
    let new = Pane {
        id: PaneId::new(),
        title: "Terminal".into(),
        content: PaneContent::Terminal { session: None },
        proxy: false,
    };
    let id = new.id;
    state.open_pane_in_container(project_id, new, None)?;
    assert_eq!(active(&state)?, last);
    assert_eq!(
        state.project(project_id).ok_or("project")?.tabs[0]
            .layout
            .neighbor(last, Direction::Right),
        Some(id)
    );
    Ok(())
}

#[test]
fn load_merges_three_subtrees_preserving_sessions_order_and_selected_pane() -> Result {
    let mut state = AppState::bootstrap()?;
    let project_id = state.add_project(ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project_id)?;
    let first = state.open_terminal_tab(project_id)?;
    let mut tabs = vec![
        state.project(project_id).ok_or("project")?.tabs[0].clone(),
        new_tab(&state.project(project_id).ok_or("project")?.tabs[0]),
        new_tab(&state.project(project_id).ok_or("project")?.tabs[0]),
    ];
    let extra = Pane {
        id: PaneId::new(),
        title: "nested".into(),
        content: PaneContent::Terminal {
            session: SessionId::new(42),
        },
        proxy: false,
    };
    let anchor = tabs[1].panes[0].id;
    tabs[1].layout.split(anchor, extra.id, Direction::Down);
    tabs[1].panes.push(extra);
    let expected: Vec<_> = tabs.iter().flat_map(|tab| &tab.panes).cloned().collect();
    let selected = tabs[2].id;
    let focused = tabs[2].panes[0].id;
    tabs[2].zoomed = Some(focused);
    let subtrees: Vec<_> = tabs.iter().map(|tab| tab.layout.clone()).collect();
    let mut stored = serde_json::to_value(&state)?;
    stored["projects"][1]["tabs"] = serde_json::to_value(tabs)?;
    stored["window"]["selected_tab"][project_id.to_string()] = json!(selected);
    stored["window"]["active_pane"] = json!(focused);
    let loaded: AppState = serde_json::from_value(stored)?;
    assert_eq!(loaded.project(project_id).ok_or("project")?.tabs.len(), 1);
    let tab = &loaded.project(project_id).ok_or("project")?.tabs[0];
    assert_eq!(tab.id, first);
    assert_eq!(tab.panes, expected);
    assert_eq!(
        tab.layout.leaves(),
        expected.iter().map(|pane| pane.id).collect::<Vec<_>>()
    );
    assert_eq!(
        tab.layout,
        Layout::Split {
            axis: Axis::Horizontal,
            ratio: 0.5,
            first: Box::new(Layout::Split {
                axis: Axis::Horizontal,
                ratio: 0.5,
                first: Box::new(subtrees[0].clone()),
                second: Box::new(subtrees[1].clone())
            }),
            second: Box::new(subtrees[2].clone())
        }
    );
    assert_eq!(loaded.window().selected_tab[&project_id], first);
    assert_eq!(active(&loaded)?, focused);
    assert!(tab.zoomed.is_none());
    assert!(
        loaded
            .project(project_id)
            .ok_or("project")?
            .groups()
            .is_none()
    );
    let mut project = loaded.project(project_id).ok_or("project")?.clone();
    codesh::normalize(&mut project);
    assert_eq!(&project, loaded.project(project_id).ok_or("project")?);
    Ok(())
}

#[test]
fn layouts_merge_extra_tabs_and_keep_startup_commands() -> Result {
    let mut state = AppState::bootstrap()?;
    let project_id = state.add_project(ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project_id)?;
    let config = Config::parse(
        r#"{"tabs":[{"name":"one","command":"first"},{"name":"two","command":"second"},{"name":"three","command":"third"}]}"#,
    )?;
    let panes = state.apply_project_layout(project_id, &config)?;
    assert_eq!(state.project(project_id).ok_or("project")?.tabs.len(), 1);
    assert_eq!(
        state.project(project_id).ok_or("project")?.tabs[0]
            .layout
            .leaves(),
        panes
    );
    for (pane, command) in panes.into_iter().zip(["first", "second", "third"]) {
        assert_eq!(state.startup_command(pane), Some(command));
    }
    Ok(())
}

#[test]
fn closing_last_pane_leaves_empty_project_then_new_terminal_recreates_container() -> Result {
    let mut state = AppState::bootstrap()?;
    let project_id = state.add_project(ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project_id)?;
    let old = state.open_terminal_tab(project_id)?;
    let first = active(&state)?;
    state.open_terminal_tab(project_id)?;
    state.close_pane(first)?;
    state.close_pane(active(&state)?)?;
    assert!(state.project(project_id).ok_or("project")?.tabs.is_empty());
    assert!(state.window().active_pane.is_none());
    let new = state.open_terminal_tab(project_id)?;
    assert_ne!(old, new);
    assert_eq!(state.project(project_id).ok_or("project")?.tabs.len(), 1);
    assert_eq!(
        state.project(project_id).ok_or("project")?.tabs[0]
            .panes
            .len(),
        1
    );
    Ok(())
}

#[test]
fn servers_content_reuses_and_focuses_its_pane_without_replacing_terminal() -> Result {
    let mut state = AppState::bootstrap()?;
    let project_id = state.add_project(ServerId::local(), std::env::current_dir()?)?;
    state.select_project(project_id)?;
    let tab = state.open_terminal_tab(project_id)?;
    let terminal = active(&state)?;
    assert_eq!(state.show_servers_tab(project_id, "Servers")?, tab);
    let servers = active(&state)?;
    state.focus_pane(terminal)?;
    assert_eq!(state.show_servers_tab(project_id, "Servers")?, tab);
    assert_eq!(active(&state)?, servers);
    assert_eq!(
        state.project(project_id).ok_or("project")?.tabs[0]
            .panes
            .len(),
        2
    );
    state.split_tab(tab, tab, Direction::Right)?;
    assert!(
        state
            .project(project_id)
            .ok_or("project")?
            .groups()
            .is_none()
    );
    Ok(())
}

#[test]
fn worktree_projects_obey_the_same_policy() -> Result {
    let mut state = AppState::bootstrap()?;
    let directory = std::env::current_dir()?;
    let parent = state.add_project(ServerId::local(), directory.clone())?;
    let child = state.add_project(ServerId::local(), directory.join("src"))?;
    let mut stored = serde_json::to_value(&state)?;
    stored["projects"][2]["parent_id"] = json!(parent);
    let mut state: AppState = serde_json::from_value(stored)?;
    let tab = state.open_terminal_tab(child)?;
    assert_eq!(state.open_terminal_tab(child)?, tab);
    let project = state.project(child).ok_or("worktree")?;
    assert_eq!(project.parent_id, Some(parent));
    assert_eq!(project.tabs.len(), 1);
    assert_eq!(project.tabs[0].panes.len(), 2);
    assert!(state.project(parent).ok_or("parent")?.tabs.is_empty());
    Ok(())
}

#[test]
fn home_tabs_keep_their_layouts_sessions_and_selection_through_normalize_and_restore() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let first = state.open_terminal_tab(home)?;
    let pane = active(&state)?;
    state.set_pane_session(pane, SessionId::new(42))?;
    state.split_pane(pane, Direction::Down)?;
    let second = state.open_terminal_tab(home)?;
    let focused = active(&state)?;
    let mut project = state.home().clone();
    codesh::normalize(&mut project);
    assert_eq!(&project, state.home());
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("state.json");
    codesh_app_core::store::save(&path, &state)?;
    let loaded = codesh_app_core::store::load(&path)?;
    assert_eq!(loaded, state);
    assert_eq!(
        loaded
            .home()
            .tabs
            .iter()
            .map(|tab| tab.id)
            .collect::<Vec<_>>(),
        [first, second]
    );
    assert_eq!(loaded.window().selected_tab[&home], second);
    assert_eq!(loaded.window().active_pane, Some(focused));
    Ok(())
}

#[test]
fn closing_a_standalone_terminal_returns_to_recent_project_and_keeps_home() -> Result {
    let mut state = AppState::bootstrap()?;
    let home = state.home().id;
    let first = state.open_terminal_tab(home)?;
    let project = state.add_project(ServerId::local(), std::env::current_dir()?)?;
    let project_tab = state.open_terminal_tab(project)?;
    let recent = active(&state)?;
    let last = state.open_terminal_tab(home)?;
    let pane = active(&state)?;
    state.close_pane(pane)?;
    assert_eq!(state.window().current_project, project);
    assert_eq!(active(&state)?, recent);
    assert_eq!(state.window().selected_tab[&project], project_tab);
    assert_eq!(state.home().tabs.len(), 1);
    assert!(!state.home().tabs.iter().any(|tab| tab.id == last));
    state.select_tab(home, first)?;
    state.close_pane(active(&state)?)?;
    assert!(state.project(home).ok_or("Home")?.tabs.is_empty());
    assert_eq!(state.window().current_project, project);
    Ok(())
}
