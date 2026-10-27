use super::*;
use crate::views::tab_strip::TabDrop;

#[gpui::test]
fn tab_drop_cannot_create_groups_or_move_pane_focus(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::current_dir().expect("cwd"))
        .expect("project");
    state.select_project(project).expect("select");
    let tab = state.open_terminal_tab(project).expect("tab");
    state.open_terminal_tab(project).expect("split");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let before = model.state.clone();
        for drop in [
            TabDrop::Split {
                beside: tab,
                edge: Direction::Right,
            },
            TabDrop::Into {
                group: tab,
                before: None,
            },
        ] {
            model.drop_tab(tab, drop, Some(tab), cx);
            assert_eq!(model.state, before);
            assert!(model.state.current_project().groups().is_none());
            assert_eq!(model.state.current_project().tabs[0].panes.len(), 2);
        }
    });
}
