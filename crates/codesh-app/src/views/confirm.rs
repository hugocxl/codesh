use codesh_ui::dialog::ConfirmationResponse;
use gpui::{AnyWindowHandle, AsyncApp, Context};

use crate::model::AppModel;

pub(crate) const TITLE: &str = "Close Tab?";
pub(crate) const MESSAGE: &str =
    "A process is still running in this tab.\nAre you sure you want to close it?";

pub(crate) const PANE_TITLE: &str = "Close Pane?";
pub(crate) const PANE_MESSAGE: &str =
    "A process is still running in this pane.\nAre you sure you want to close it?";

impl AppModel {
    pub(crate) fn confirm_close(&mut self, tab: codesh_app_core::TabId, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        self.dismiss_overlay(cx);
        let window = self.window;
        let (title, message, action) = if self.restarting_terminal() {
            (
                "Restart Terminal?",
                "A process is still running in this pane.\nRestarting the terminal ends it.",
                "Restart",
            )
        } else if self.closing_multiple_tabs() {
            (
                "Close Tabs?",
                "A process is still running in these tabs.\nAre you sure you want to close them?",
                "Close",
            )
        } else if self.closing_one_pane() {
            (PANE_TITLE, PANE_MESSAGE, "Close")
        } else {
            (TITLE, MESSAGE, "Close")
        };
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = prompt(window, title, message, action, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_close_prompt(tab, response, cx);
            });
        }));
    }
}

#[cfg(not(test))]
async fn prompt(
    window: AnyWindowHandle,
    title: &'static str,
    message: &'static str,
    action: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, cx| {
            codesh_ui::dialog::confirm(
                window,
                cx,
                title,
                message,
                action,
                Some("Don't ask again"),
                move |response| {
                    let _ = sender.try_send(response);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(receiver.recv().await.unwrap_or_default())
}

#[cfg(test)]
pub(crate) const CLOSE_WITHOUT_ASKING: &str = "Close and don't ask again";

#[cfg(test)]
async fn prompt(
    window: AnyWindowHandle,
    title: &'static str,
    message: &'static str,
    _action: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                title,
                Some(message),
                &["Close", "Cancel", CLOSE_WITHOUT_ASKING],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(match answer.await {
        Ok(0) => ConfirmationResponse::Confirmed {
            dont_ask_again: false,
        },
        Ok(2) => ConfirmationResponse::Confirmed {
            dont_ask_again: true,
        },
        _ => ConfirmationResponse::Cancelled,
    })
}

/// Stops or restarts a server; `name` is another computer's, `None` this one's.
pub(crate) async fn prompt_server(
    window: AnyWindowHandle,
    restart: bool,
    name: Option<&str>,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = match (restart, name) {
        (true, None) => "Restart Server?".to_string(),
        (false, None) => "Stop Server?".to_string(),
        (true, Some(name)) => format!("Restart the Server on {name}?"),
        (false, Some(name)) => format!("Stop the Server on {name}?"),
    };
    let label = if restart { "Restart" } else { "Stop" };
    let message = match name {
        Some(name) => format!("All running terminal sessions on {name} will end. Saved terminal output and settings will remain."),
        None => "All running terminal sessions on this device will end. Saved terminal output and settings will remain.".to_string(),
    };
    server_prompt(window, &title, label, &message, cx).await
}

pub(crate) async fn prompt_install_server(
    window: AnyWindowHandle,
    name: &str,
    source: &crate::remote_install::Source,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = format!("Install codesh-server on {name}?");
    let message = source.description(name);
    server_prompt(window, &title, "Install", &message, cx).await
}

pub(crate) async fn prompt_forget_server(
    window: AnyWindowHandle,
    name: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = format!("Forget {name}?");
    let message = "Its projects and layouts leave this app; nothing on the server changes.";
    server_prompt(window, &title, "Forget", message, cx).await
}

pub(crate) async fn prompt_forget_servers(
    window: AnyWindowHandle,
    count: usize,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = format!("Forget {count} Servers?");
    let message = "Their projects and layouts leave this app; nothing on the servers changes.";
    server_prompt(window, &title, "Forget", message, cx).await
}

pub(crate) async fn prompt_revoke(
    window: AnyWindowHandle,
    device: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = format!("Revoke {device}?");
    let message = "The device disconnects now and must be paired again to reconnect.";
    server_prompt(window, &title, "Revoke", message, cx).await
}

pub(crate) async fn prompt_delete_file(
    window: AnyWindowHandle,
    name: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = format!("Delete “{name}”?");
    let message = "It moves to the Trash on the computer that has the project.";
    server_prompt(window, &title, "Delete", message, cx).await
}

/// What to do with unsaved editors that are closing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Unsaved {
    Save,
    Discard,
    Cancel,
}

pub(crate) async fn prompt_unsaved(
    window: AnyWindowHandle,
    names: &[String],
    cx: &mut AsyncApp,
) -> Unsaved {
    let title = match names {
        [name] => format!("Save changes to “{name}”?"),
        _ => format!("Save changes to {} files?", names.len()),
    };
    let message = "Your changes are lost if you don't save them.";
    let buttons = ["Save", "Don't Save", "Cancel"];
    let buttons: Vec<&str> = buttons.iter().map(AsRef::as_ref).collect();
    let Ok(answer) = window.update(cx, |_, window, cx| {
        window.prompt(
            gpui::PromptLevel::Warning,
            &title,
            Some(message),
            &buttons,
            cx,
        )
    }) else {
        return Unsaved::Cancel;
    };
    match answer.await {
        Ok(0) => Unsaved::Save,
        Ok(1) => Unsaved::Discard,
        _ => Unsaved::Cancel,
    }
}

pub(crate) async fn prompt_reset_terminal(
    window: AnyWindowHandle,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let title = "Reset Terminal Settings?";
    let message =
        "Every setting on the Terminal page, including key bindings, returns to its default.";
    server_prompt(window, title, "Reset", message, cx).await
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UpdateChoice {
    Install,
    Schedule,
    CancelSchedule,
    EndSessions,
    Later,
}

pub(crate) async fn prompt_update(
    window: AnyWindowHandle,
    version: &str,
    compatible: bool,
    scheduled: bool,
    sessions: usize,
    cx: &mut AsyncApp,
) -> Result<UpdateChoice, String> {
    if compatible && !scheduled {
        return prompt_install(window, version, cx).await;
    }
    let message = if scheduled {
        format!(
            "Codesh {version} will install and restart the app when all terminal sessions end. Currently {sessions} sessions are running, including idle shells and detached sessions."
        )
    } else {
        format!(
            "Codesh {version} requires a server restart. You can update automatically when all terminal sessions end, or end them now. Terminal panes will close. App-only panes and settings will remain."
        )
    };
    let labels = if scheduled {
        [
            "Keep Waiting",
            "Update and End Sessions…",
            "Cancel Scheduled Update",
        ]
    } else {
        [
            "Update When Sessions End",
            "Update and End Sessions…",
            "Not Now",
        ]
    };
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Info,
                "Codesh Update",
                Some(&message),
                &labels,
                cx,
            )
        })
        .map_err(|error| error.to_string())?
        .await
        .map_err(|error| error.to_string())?;
    let choice = match (scheduled, answer) {
        (true, 2) => UpdateChoice::CancelSchedule,
        (_, 1) => UpdateChoice::EndSessions,
        (false, 0) => UpdateChoice::Schedule,
        _ => UpdateChoice::Later,
    };
    if choice == UpdateChoice::EndSessions && !server_prompt(window, "Update and End All Sessions?", "Update and End Sessions", "All terminal processes on this device will end, including sessions used by other clients. Terminal panes will close. App-only panes and settings will remain.", cx).await? {
        return Ok(UpdateChoice::Later);
    }
    Ok(choice)
}

const INSTALL: &str = "Update and Restart App";

async fn prompt_install(
    window: AnyWindowHandle,
    version: &str,
    cx: &mut AsyncApp,
) -> Result<UpdateChoice, String> {
    let message = format!(
        "Install Codesh {version} and restart the app? Running terminals will continue, and the server will update when all terminal sessions end. Restarting the server now ends all terminal sessions on this device, including sessions used by other clients."
    );
    Ok(match install_prompt(window, &message, cx).await? {
        Some(true) => UpdateChoice::EndSessions,
        Some(false) => UpdateChoice::Install,
        None => UpdateChoice::Later,
    })
}

/// Reports whether the server should restart too, or `None` when declined.
#[cfg(not(test))]
async fn install_prompt(
    window: AnyWindowHandle,
    message: &str,
    cx: &mut AsyncApp,
) -> Result<Option<bool>, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, cx| {
            codesh_ui::dialog::confirm_with_checkbox(
                window,
                cx,
                "Codesh Update",
                message,
                [INSTALL, "Not Now"],
                "Also restart the server",
                move |restart_server| {
                    let _ = sender.try_send(restart_server);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(receiver.recv().await.unwrap_or_default())
}

#[cfg(test)]
pub(crate) const INSTALL_AND_RESTART_SERVER: &str = "Update and Restart App and Server";

#[cfg(test)]
async fn install_prompt(
    window: AnyWindowHandle,
    message: &str,
    cx: &mut AsyncApp,
) -> Result<Option<bool>, String> {
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Info,
                "Codesh Update",
                Some(message),
                &[INSTALL, "Not Now", INSTALL_AND_RESTART_SERVER],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(match answer.await {
        Ok(0) => Some(false),
        Ok(2) => Some(true),
        _ => None,
    })
}

#[cfg(not(test))]
async fn server_prompt(
    window: AnyWindowHandle,
    title: &str,
    label: &str,
    message: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, cx| {
            codesh_ui::dialog::confirm(window, cx, title, message, label, None, move |response| {
                let _ = sender.try_send(matches!(response, ConfirmationResponse::Confirmed { .. }));
            })
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(receiver.recv().await.unwrap_or(false))
}

#[cfg(test)]
async fn server_prompt(
    window: AnyWindowHandle,
    title: &str,
    label: &str,
    message: &str,
    cx: &mut AsyncApp,
) -> Result<bool, String> {
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                title,
                Some(message),
                &[label, "Cancel"],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(answer.await == Ok(0))
}
