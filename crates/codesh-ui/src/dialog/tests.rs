use super::*;
use gpui::{
    Context, Entity, FocusHandle, InteractiveElement, IntoElement, Render, TestAppContext, div,
};

struct Host {
    focus: FocusHandle,
}

impl Host {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
        }
    }
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().track_focus(&self.focus)
    }
}

fn active(window: &Window, cx: &App) -> Entity<view::Dialog> {
    cx.global::<Dialogs>().active[&window.window_handle().window_id()]
        .upgrade()
        .expect("live prompt")
}

#[gpui::test]
fn cancelling_a_confirmation_cannot_confirm_or_dismiss_its_successor(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, cx| Host::new(cx));
    let answers = Rc::new(RefCell::new(Vec::new()));
    let first = cx.update(|window, cx| {
        let answers = answers.clone();
        confirm(
            window,
            cx,
            "Delete",
            "Delete data?",
            "Delete",
            None,
            move |answer| answers.borrow_mut().push(answer),
        )
        .expect("first prompt")
    });
    drop(first);
    cx.update(|window, cx| {
        active(window, cx).update(cx, |view, cx| view.finish(Some(0), cx));
    });
    cx.run_until_parked();
    assert_eq!(*answers.borrow(), [ConfirmationResponse::Cancelled]);
    let second = cx.update(|window, cx| {
        let answers = answers.clone();
        confirm(
            window,
            cx,
            "Delete",
            "Delete other data?",
            "Delete",
            None,
            move |answer| answers.borrow_mut().push(answer),
        )
        .expect("second prompt")
    });
    cx.update(|window, cx| {
        active(window, cx).update(cx, |view, cx| view.finish(Some(0), cx));
    });
    let third = cx.update(|window, cx| {
        confirm(window, cx, "Leave", "Leave session?", "Leave", None, |_| {}).expect("third prompt")
    });
    drop(second);
    cx.run_until_parked();
    cx.update(|window, cx| assert!(active(window, cx).read(cx).pending()));
    assert_eq!(
        *answers.borrow(),
        [
            ConfirmationResponse::Cancelled,
            ConfirmationResponse::Confirmed {
                dont_ask_again: false
            }
        ]
    );
    drop(third);
}

#[gpui::test]
fn a_confirmation_keeps_the_keyboard_when_the_window_refocuses_its_content(
    cx: &mut TestAppContext,
) {
    let (host, cx) = cx.add_window_view(|_, cx| Host::new(cx));
    let answers = Rc::new(RefCell::new(Vec::new()));
    let prompt = cx.update(|window, cx| {
        let answers = answers.clone();
        confirm(
            window,
            cx,
            "Close Pane?",
            "A process is still running.",
            "Close",
            None,
            move |answer| answers.borrow_mut().push(answer),
        )
        .expect("prompt")
    });
    cx.update(|window, cx| {
        let content = host.read(cx).focus.clone();
        window.focus(&content);
    });
    cx.simulate_keystrokes("escape");
    assert_eq!(*answers.borrow(), [ConfirmationResponse::Cancelled]);
    drop(prompt);
}
