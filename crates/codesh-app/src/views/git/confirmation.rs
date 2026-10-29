use gpui::{AnyWindowHandle, AsyncApp};

#[cfg(not(test))]
pub(super) async fn prompt(
    window: AnyWindowHandle,
    message: &str,
    has_hooks: bool,
    cx: &mut AsyncApp,
) -> Option<bool> {
    let (send, receive) = async_channel::bounded(1);
    let dialog = window.update(cx, |_, window, cx| {
        codesh_ui::dialog::confirm(
            window,
            cx,
            "Confirm Git Operation",
            message,
            "Confirm",
            has_hooks.then_some("Run the teardown commands shown above"),
            move |answer| {
                let _ = send.try_send(answer);
            },
        )
    });
    let Ok(Ok(_dialog)) = dialog else {
        return None;
    };
    match receive.recv().await {
        Ok(codesh_ui::dialog::ConfirmationResponse::Confirmed { dont_ask_again }) => {
            Some(dont_ask_again)
        }
        _ => None,
    }
}

#[cfg(test)]
pub(super) async fn prompt(
    window: AnyWindowHandle,
    message: &str,
    has_hooks: bool,
    cx: &mut AsyncApp,
) -> Option<bool> {
    let answer = window
        .update(cx, |_, window, cx| {
            let mut buttons = vec!["Confirm", "Cancel"];
            if has_hooks {
                buttons.push("Confirm with teardown commands");
            }
            window.prompt(
                gpui::PromptLevel::Warning,
                "Confirm Git Operation",
                Some(message),
                &buttons,
                cx,
            )
        })
        .ok()?;
    match answer.await {
        Ok(0) => Some(false),
        Ok(2) if has_hooks => Some(true),
        _ => None,
    }
}
