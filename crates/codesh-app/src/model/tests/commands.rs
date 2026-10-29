use super::*;
use codesh_app_core::settings::AppLayout;
use codesh_core::capability::{Capability, Feature};
use codesh_core::shortcuts::ShortcutId;

const TERMINAL: [ShortcutId; 10] = [
    ShortcutId::Find,
    ShortcutId::FindNext,
    ShortcutId::FindPrevious,
    ShortcutId::PreviousPrompt,
    ShortcutId::NextPrompt,
    ShortcutId::SelectCommandOutput,
    ShortcutId::ScrollToBottom,
    ShortcutId::IncreaseFontSize,
    ShortcutId::DecreaseFontSize,
    ShortcutId::DetachTerminal,
];
const PANE_FOCUS: [ShortcutId; 4] = [
    ShortcutId::FocusPaneLeft,
    ShortcutId::FocusPaneRight,
    ShortcutId::FocusPaneUp,
    ShortcutId::FocusPaneDown,
];
const APP: [ShortcutId; 6] = [
    ShortcutId::ExistingTerminals,
    ShortcutId::ToggleFullScreen,
    ShortcutId::Minimize,
    ShortcutId::Zoom,
    ShortcutId::Quit,
    ShortcutId::EndAllSessionsAndQuit,
];

/// Whether the palette lists each command, and lists it disabled.
fn listed(model: &AppModel, ids: &[ShortcutId]) -> Vec<Option<bool>> {
    let catalog = model.root_catalog();
    ids.iter()
        .map(|id| catalog.command(id.name()).map(|command| command.disabled))
        .collect()
}

#[gpui::test]
fn the_palette_lists_the_commands_only_a_menu_bar_offered(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        assert_eq!(listed(model, &APP), [Some(false); 6]);
        assert_eq!(listed(model, &TERMINAL), [Some(true); 10]);
        assert_eq!(listed(model, &PANE_FOCUS), [Some(true); 4]);

        model.new_tab(cx);
        assert_eq!(listed(model, &TERMINAL), [Some(false); 10]);
        assert_eq!(listed(model, &PANE_FOCUS), [Some(true); 4]);

        model.split_pane(Direction::Right, cx);
        assert_eq!(listed(model, &PANE_FOCUS), [Some(false); 4]);
    });
}

#[gpui::test]
fn the_palette_leaves_out_commands_this_build_cannot_run(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let install = ShortcutId::InstallCommandLineTool.name();
    view.update(cx, |model, _| {
        for feature in [Feature::Updates, Feature::CommandLineTool] {
            model.capabilities.set(feature, Capability::Supported);
        }
        let catalog = model.root_catalog();
        assert!(catalog.command("check_for_updates").is_some());
        assert!(catalog.command(install).is_some());

        for feature in [Feature::Updates, Feature::CommandLineTool] {
            model.capabilities.set(feature, Capability::Unsupported);
        }
        let catalog = model.root_catalog();
        assert!(catalog.command("check_for_updates").is_none());
        assert!(catalog.command(install).is_none());
    });
}

#[gpui::test]
fn palette_queries_still_run_the_commands_they_found_first(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.new_tab(cx);
        model.split_pane(Direction::Right, cx);
    });
    cx.simulate_keystrokes(ShortcutId::ToggleCommandPalette.definition().keys[0]);
    cx.simulate_input("zoom");
    cx.simulate_keystrokes("enter");
    view.read_with(cx, |model, _| {
        let tab = model.active_tab().and_then(|tab| model.tab(tab));
        assert!(tab.expect("tab").zoomed.is_some());
    });
}

#[gpui::test]
fn custom_command_titles_only_the_new_terminal_pane(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::current_dir().expect("cwd"))
        .expect("project");
    state.select_project(project).expect("select");
    let tab = state.open_terminal_tab(project).expect("tab");
    state
        .set_tab_title(tab, Some("Container".into()))
        .expect("title");
    let original = state.window().active_pane.expect("pane");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let command =
            codesh_app_core::settings::CustomCommand::new("Build".into(), "cargo build".into());
        let id = command.id.clone();
        model.settings.commands.push(command);
        model.run_custom_command(&id, cx);
        let pane = model.active_pane().expect("new pane");
        assert_ne!(pane, original);
        let container = &model.state.current_project().tabs[0];
        assert_eq!(container.custom_title.as_deref(), Some("Container"));
        assert_eq!(
            container
                .panes
                .iter()
                .find(|item| item.id == pane)
                .expect("pane")
                .title,
            "Build"
        );
        assert_eq!(model.state.startup_command(pane), Some("cargo build"));
    });
}

#[gpui::test]
fn new_tab_always_creates_and_selects_a_local_home_terminal(cx: &mut TestAppContext) {
    let (state, project) = git::registered_current_project();
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for _ in 0..2 {
        view.update(cx, |model, cx| {
            model.select_project(project, cx);
        });
        cx.simulate_keystrokes(ShortcutId::NewTab.definition().keys[0]);
        view.read_with(cx, |model, _| {
            assert_eq!(model.state.current_project().id, model.state.home().id);
            assert!(model.active_pane().is_some());
        });
    }
    view.read_with(cx, |model, _| {
        let home = model.state.home();
        assert!(home.server_id.is_local());
        assert_eq!(home.tabs.len(), 2);
        assert_eq!(model.active_tab(), Some(home.tabs[1].id));
        assert!(home.tabs.iter().all(|tab| tab.panes.len() == 1));
    });
}

#[gpui::test]
fn standalone_sidebar_cards_select_and_close_their_own_tab_in_every_layout(
    cx: &mut TestAppContext,
) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let first = state.open_terminal_tab(home).expect("first");
    let second = state.open_terminal_tab(home).expect("second");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for layout in [
        AppLayout::ProjectFocused,
        AppLayout::TabFocused,
        AppLayout::AgentsFocused,
    ] {
        view.update(cx, |model, cx| {
            model.appearance.sidebar_expanded = true;
            model.set_layout(layout, cx);
        });
        cx.run_until_parked();
        let first_bounds = cx
            .debug_bounds(format!("terminal-{first}").leak())
            .expect("first card");
        let second_bounds = cx
            .debug_bounds(format!("terminal-{second}").leak())
            .expect("second card");
        assert!(first_bounds.top() < second_bounds.top());
        assert!(cx.debug_bounds("project-row-0").is_none());
        cx.simulate_click(first_bounds.center(), Modifiers::default());
        view.read_with(cx, |model, _| {
            assert_eq!(model.state.current_project().id, home);
            assert_eq!(model.active_tab(), Some(first));
        });
    }
    let second_bounds = cx
        .debug_bounds(format!("terminal-{second}").leak())
        .expect("second card");
    cx.simulate_mouse_down(
        second_bounds.center(),
        gpui::MouseButton::Right,
        Modifiers::default(),
    );
    cx.simulate_mouse_up(
        second_bounds.center(),
        gpui::MouseButton::Right,
        Modifiers::default(),
    );
    cx.run_until_parked();
    let close = cx.debug_bounds("menu-item-0").expect("Close Terminal");
    cx.simulate_click(close.center(), Modifiers::default());
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.state.home().tabs.len(), 1);
        assert_eq!(model.active_tab(), Some(first));
    });
}
