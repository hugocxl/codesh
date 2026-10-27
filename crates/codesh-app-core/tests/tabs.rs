use codesh_app_core::{AppState, Layout, PaneId, TabCloseScope, TabId};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn bulk_close_targets_follow_project_order_and_skip_pinned_tabs() -> Result {
    let mut state = AppState::bootstrap()?;
    state.open_terminal_tab(state.home().id)?;
    let template = state.home().tabs[0].clone();
    let mut project = state.home().clone();
    project.tabs = (0..4)
        .map(|_| {
            let mut tab = template.clone();
            tab.id = TabId::new();
            tab.panes[0].id = PaneId::new();
            tab.layout = Layout::Leaf(tab.panes[0].id);
            tab
        })
        .collect();
    let [first, second, third, fourth] = std::array::from_fn(|index| project.tabs[index].id);
    project.tabs[0].pinned = true;
    assert_eq!(
        project.closable_tabs(third, TabCloseScope::Other),
        [second, fourth]
    );
    assert_eq!(project.closable_tabs(third, TabCloseScope::Left), [second]);
    assert_eq!(project.closable_tabs(third, TabCloseScope::Right), [fourth]);
    assert!(project.closable_tabs(first, TabCloseScope::Left).is_empty());
    assert!(
        project
            .closable_tabs(TabId::new(), TabCloseScope::Other)
            .is_empty()
    );
    Ok(())
}
