use codesh_app_core::{AppState, Project, ProjectId, ProjectStatus};
use codesh_ui::dialog::ConfirmationResponse;
use gpui::{AnyWindowHandle, AsyncApp, Context};

use super::menu::{Command, Item};
use crate::model::{AppModel, git::Repository};

/// The menu of a top-level project. `worktrees` is `None` when its folder is not
/// a Git repository, and otherwise whether its worktrees are shown.
pub(crate) fn items(state: &AppState, project: &Project, worktrees: Option<bool>) -> Vec<Item> {
    let id = project.id;
    if project.status() != ProjectStatus::Available {
        return removal(project);
    }
    let mut items = terminal_items(id);
    // Another computer's Home goes by its server's name.
    if !project.is_remote_home() {
        items.push(Item::action("Rename…", Command::EditProject(id)).separated());
    }
    if !project.home {
        items.push(Item::submenu("Workspaces", workspace_items(state, id)));
        if let Some(visible) = worktrees {
            items.push(
                Item::action("Show Worktrees", Command::Worktrees(id))
                    .checked_if(visible)
                    .separated(),
            );
        }
    }
    items.extend(path_items(project));
    items.extend(removal(project).into_iter().map(Item::separated));
    items
}

/// A folder on another computer can't be revealed here, but its path can be copied.
fn path_items(project: &Project) -> Vec<Item> {
    let copy = Item::action("Copy Path", Command::CopyPath(project.id));
    if project.server_id.is_local() {
        vec![
            Item::action(
                crate::opener::reveal_label(),
                Command::RevealPath(project.id),
            )
            .separated(),
            copy,
        ]
    } else {
        vec![copy.separated()]
    }
}

fn removal(project: &Project) -> Vec<Item> {
    if project.home {
        Vec::new()
    } else {
        vec![Item::action(
            "Remove Project…",
            Command::RemoveProject(project.id),
        )]
    }
}

/// A single-tab project opens terminals as splits of its one tab.
fn terminal_items(id: ProjectId) -> Vec<Item> {
    vec![
        Item::action("New Terminal", Command::NewProjectTab(id)),
        Item::action("Existing Terminals…", Command::ExistingSessions(id)),
    ]
}

pub(crate) fn workspace_items(state: &AppState, project: ProjectId) -> Vec<Item> {
    let mut items: Vec<_> = state
        .workspaces()
        .iter()
        .map(|workspace| {
            Item::action(
                workspace.name.clone(),
                Command::ToggleWorkspaceMember(workspace.id, project),
            )
            .checked_if(workspace.projects.contains(&project))
        })
        .collect();
    let create = Item::action("New Workspace…", Command::NewWorkspace(Some(project)));
    items.push(if items.is_empty() {
        create
    } else {
        create.separated()
    });
    items
}

/// The menu of a row in a project's worktree list. `primary` is the project's own row.
/// A worktree whose folder is gone can only be removed.
pub(crate) fn worktree_items(project: &Project, primary: bool) -> Vec<Item> {
    let id = project.id;
    if project.status() != ProjectStatus::Available {
        return if primary {
            Vec::new()
        } else {
            vec![Item::action("Remove Worktree…", Command::RemoveProject(id))]
        };
    }
    let mut items = terminal_items(id);
    if !primary {
        items.push(Item::action("Rename Worktree…", Command::EditProject(id)).separated());
    }
    items.extend(path_items(project));
    if !primary {
        items.push(
            Item::action("Remove Worktree and Files…", Command::RemoveWorktree(id)).separated(),
        );
    }
    items
}

impl AppModel {
    /// The sidebar menu of `project`. Worktree projects get the worktree menu.
    pub(crate) fn project_menu(&self, project: &Project) -> Vec<Item> {
        if project.parent_id.is_some() {
            return worktree_items(project, false);
        }
        items(
            &self.state,
            project,
            self.repository(project.id)
                .then(|| self.worktrees_visible(project.id)),
        )
    }

    /// Whether `project` is, or may be, a Git repository that can have worktrees.
    pub(crate) fn repository(&self, project: ProjectId) -> bool {
        self.state
            .projects()
            .iter()
            .any(|child| child.parent_id == Some(project))
            || !self
                .git
                .projects
                .get(&project)
                .is_some_and(Repository::not_a_repository)
    }

    /// Opens the new worktree form, showing the project's worktrees first.
    pub(crate) fn new_worktree(
        &mut self,
        project: ProjectId,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if !self.worktrees_visible(project) {
            self.toggle_worktree_visibility(project, cx);
        }
        self.open_git_form(project, true, cx);
        self.focus_git_form(window, cx);
    }

    pub(crate) fn confirm_remove_project(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let Some(record) = self.state.project(project).filter(|project| !project.home) else {
            return;
        };
        let gone = record.parent_id.is_some() && record.status() != ProjectStatus::Available;
        let message = if gone {
            format!(
                "Remove “{}” from every client? Its folder is already gone. All of its terminal sessions will end, including those displayed in other clients, and their saved output will be discarded.",
                &record.name
            )
        } else {
            format!(
                "Remove “{}” from every client? All of its terminal sessions will end, including those displayed in other clients, and their saved output will be discarded. The folder and its files will stay on disk.",
                &record.name
            )
        };
        self.confirm(
            if gone {
                "Remove Worktree?"
            } else {
                "Remove Project?"
            },
            message,
            "Remove",
            cx,
            move |model, cx| {
                if gone {
                    model.leave_worktree(project, cx);
                }
                model.remove_project_confirmed(project, cx);
            },
        );
    }

    /// `title` and `action` are English keys, shown translated; `message` is
    /// shown as is.
    pub(crate) fn confirm(
        &mut self,
        title: &'static str,
        message: String,
        action: &'static str,
        cx: &mut Context<Self>,
        confirmed: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        self.dismiss_overlay(cx);
        let window = self.window;
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = prompt(window, title, message, action, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                model.focus_requested = true;
                match response {
                    Ok(ConfirmationResponse::Confirmed { .. }) => confirmed(model, cx),
                    Ok(ConfirmationResponse::Cancelled) => {}
                    Err(error) => {
                        model.fail(format!("Could not show confirmation: {error}"), cx);
                    }
                }
                cx.notify();
            });
        }));
    }
}

#[cfg(not(test))]
async fn prompt(
    window: AnyWindowHandle,
    title: &'static str,
    message: String,
    action: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, cx| {
            codesh_ui::dialog::confirm(window, cx, title, &message, action, None, move |response| {
                let _ = sender.try_send(response);
            })
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(receiver.recv().await.unwrap_or_default())
}

#[cfg(test)]
async fn prompt(
    window: AnyWindowHandle,
    title: &'static str,
    message: String,
    action: &'static str,
    cx: &mut AsyncApp,
) -> Result<ConfirmationResponse, String> {
    let answer = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                title,
                Some(&message),
                &[action, "Cancel"],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(if answer.await == Ok(0) {
        ConfirmationResponse::Confirmed {
            dont_ask_again: false,
        }
    } else {
        ConfirmationResponse::Cancelled
    })
}
