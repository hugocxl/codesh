use super::*;
use codesh_app_core::Direction;
use gpui::{Bounds, MouseButton, Point, point};

mod docking;

fn state() -> (AppState, [TabId; 2], [PaneId; 4]) {
    let mut state = AppState::bootstrap().expect("state");
    let a = state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.window().active_pane.expect("pane");
    let second = state.split_pane(first, Direction::Right).expect("split");
    let third = state.split_pane(second, Direction::Down).expect("split");
    let other = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    let b = state.open_terminal_tab(other).expect("tab");
    let fourth = state.window().active_pane.expect("pane");
    for (index, pane) in [first, second, third, fourth].iter().enumerate() {
        state
            .set_pane_session(*pane, SessionId::new(200 + index as u64))
            .expect("session");
    }
    state.select_tab(state.home().id, a).expect("select");
    (state, [a, b], [first, second, third, fourth])
}

fn press(cx: &mut VisualTestContext, position: Point<gpui::Pixels>, command: bool) {
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        modifiers: Modifiers {
            platform: command,
            ..Default::default()
        },
        ..Default::default()
    });
    cx.run_until_parked();
}

fn pointer(cx: &mut VisualTestContext, position: Point<gpui::Pixels>, pressed: bool) {
    cx.simulate_event(gpui::MouseMoveEvent {
        position,
        pressed_button: pressed.then_some(MouseButton::Left),
        ..Default::default()
    });
    cx.run_until_parked();
}

fn release(cx: &mut VisualTestContext, position: Point<gpui::Pixels>) {
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
}

fn pane_bounds(
    _view: &Entity<AppModel>,
    pane: PaneId,
    cx: &mut VisualTestContext,
) -> Bounds<gpui::Pixels> {
    cx.debug_bounds(format!("pane-card-{pane}").leak())
        .expect("pane card")
}
