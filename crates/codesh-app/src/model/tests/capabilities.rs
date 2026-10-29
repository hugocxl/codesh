use super::*;
use codesh_core::capability::{Capability, Feature};
use codesh_core::shortcuts::ShortcutId;

fn without(model: &mut AppModel, features: &[Feature]) {
    for feature in features {
        model.capabilities.set(*feature, Capability::Unsupported);
    }
}

fn reason(feature: Feature) -> String {
    feature.unsupported_reason().to_owned()
}

#[gpui::test]
fn commands_without_a_backend_report_why_and_start_nothing(cx: &mut TestAppContext) {
    let (boot, requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, _window, cx| {
        model.new_tab(cx);
        without(model, &[Feature::CommandLineTool]);
        let tabs = model.state.home().tabs.len();
        let _ = requests.try_iter().count();

        model.install_command_line(cx);
        assert_eq!(model.error, Some(reason(Feature::CommandLineTool)));

        assert_eq!(model.state.home().tabs.len(), tabs);
        assert_eq!(requests.try_iter().count(), 0);
    });
}

#[gpui::test]
fn shortcuts_for_features_without_a_backend_report_why(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.new_tab(cx);
        without(model, &[Feature::AppHiding]);
    });
    for (shortcut, feature) in [
        (ShortcutId::HideApp, Feature::AppHiding),
        (ShortcutId::HideOthers, Feature::AppHiding),
    ] {
        view.update(cx, |model, _| model.error = None);
        cx.simulate_keystrokes(shortcut.definition().keys[0]);
        view.update(cx, |model, _| {
            assert_eq!(model.error, Some(reason(feature)));
        });
    }
}

#[gpui::test]
#[gpui::test]
fn standalone_dictation_without_a_backend_reports_why_and_starts_no_recorder(
    cx: &mut TestAppContext,
) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update_in(cx, |model, window, cx| {
        without(model, &[Feature::Voice]);
        model.toggle_voice(window, cx);
        assert!(model.voice.view.is_none());
        assert_eq!(model.error, Some(reason(Feature::Voice)));
    });
}
