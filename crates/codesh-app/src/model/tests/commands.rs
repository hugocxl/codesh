use super::*;
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
fn listed(
    model: &mut AppModel,
    ids: &[ShortcutId],
    cx: &mut Context<AppModel>,
) -> Vec<Option<bool>> {
    let registry = model.command_registry(true, cx);
    ids.iter()
        .map(|id| {
            registry
                .command(id.name())
                .map(codesh_ui::command_palette::Command::is_disabled)
        })
        .collect()
}

#[gpui::test]
fn the_palette_lists_the_commands_only_a_menu_bar_offered(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        assert_eq!(listed(model, &APP, cx), [Some(false); 6]);
        assert_eq!(listed(model, &TERMINAL, cx), [Some(true); 10]);
        assert_eq!(listed(model, &PANE_FOCUS, cx), [Some(true); 4]);

        model.new_tab(cx);
        assert_eq!(listed(model, &TERMINAL, cx), [Some(false); 10]);
        assert_eq!(listed(model, &PANE_FOCUS, cx), [Some(true); 4]);

        model.split_pane(Direction::Right, cx);
        assert_eq!(listed(model, &PANE_FOCUS, cx), [Some(false); 4]);
    });
}

#[gpui::test]
fn the_palette_leaves_out_commands_this_build_cannot_run(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let install = ShortcutId::InstallCommandLineTool.name();
    view.update(cx, |model, cx| {
        for feature in [Feature::Updates, Feature::CommandLineTool] {
            model.capabilities.set(feature, Capability::Supported);
        }
        let registry = model.command_registry(true, cx);
        assert!(registry.command("check_for_updates").is_some());
        assert!(registry.command(install).is_some());

        for feature in [Feature::Updates, Feature::CommandLineTool] {
            model.capabilities.set(feature, Capability::Unsupported);
        }
        let registry = model.command_registry(true, cx);
        assert!(registry.command("check_for_updates").is_none());
        assert!(registry.command(install).is_none());
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
    let tab = state.open_terminal_tab(state.home().id).expect("tab");
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
