use codesh_app_core::settings::CommitChoices;
use codesh_protocol::ProjectId;
use codesh_ui::controls::Style;
use codesh_ui::dialog::{PromptCheckbox, PromptChoices, PromptPicker, PromptResponse};
use gpui::{AnyWindowHandle, AsyncApp};

use crate::ai::Provider;
use crate::repository_actions::Action;

/// Captured before confirmation and revalidated before any repository changes.
pub(crate) struct AiConfirmation {
    pub(crate) project: ProjectId,
    pub(crate) action: Action,
    pub(crate) provider: Provider,
    /// Providers a commit can switch to before confirming.
    pub(crate) providers: Vec<Provider>,
    /// The commit options to remember, even when `staged` overrides one.
    pub(crate) choices: CommitChoices,
    /// Whether anything is staged, so a commit can leave unstaged changes out.
    pub(crate) staged: bool,
    pub(crate) branch: String,
    pub(crate) head: Option<String>,
}

impl AiConfirmation {
    fn title(&self) -> String {
        match self.action {
            Action::Commit => format!("Commit to \"{}\"?", &self.branch),
            Action::CreatePullRequest => {
                format!("Create a pull request from \"{}\"?", &self.branch)
            }
        }
    }

    fn message(&self) -> String {
        match self.action {
            Action::Commit => {
                "Codesh will ask the AI provider for a commit message, then commit.".into()
            }
            Action::CreatePullRequest => format!(
                "Codesh will stage all changes, ask {} for a branch name, title, and summary, then create the branch, commit, push, and open the pull request on GitHub.",
                self.provider.name
            ),
        }
    }

    fn prompt_choices(&self) -> PromptChoices {
        if self.action != Action::Commit {
            return PromptChoices::default();
        }
        PromptChoices {
            picker: Some(PromptPicker {
                label: "AI Provider".into(),
                items: self
                    .providers
                    .iter()
                    .map(|provider| provider.name.into())
                    .collect(),
                selected: self
                    .providers
                    .iter()
                    .position(|provider| *provider == self.provider)
                    .unwrap_or_default(),
            }),
            checkboxes: vec![
                PromptCheckbox {
                    label: "Include unstaged changes".into(),
                    checked: self.commit_choices().include_unstaged,
                    enabled: self.staged,
                },
                PromptCheckbox {
                    label: "Push after committing".into(),
                    checked: self.choices.push,
                    enabled: true,
                },
            ],
        }
    }

    /// Takes the provider and options the sheet was confirmed with.
    pub(crate) fn confirm(&mut self, response: &PromptResponse) {
        if let Some(provider) = response.picked.and_then(|index| self.providers.get(index)) {
            self.provider = *provider;
        }
        if let [include_unstaged, push] = response.checked[..] {
            if self.staged {
                self.choices.include_unstaged = include_unstaged;
            }
            self.choices.push = push;
        }
    }

    /// What the action commits and whether it pushes. Pull requests always
    /// include every change and push.
    pub(crate) fn commit_choices(&self) -> CommitChoices {
        match self.action {
            Action::Commit => CommitChoices {
                include_unstaged: self.choices.include_unstaged || !self.staged,
                push: self.choices.push,
            },
            Action::CreatePullRequest => CommitChoices::default(),
        }
    }

    #[cfg(not(test))]
    pub(crate) async fn prompt(
        &self,
        window: AnyWindowHandle,
        style: Style<'_>,
        cx: &mut AsyncApp,
    ) -> Result<Option<PromptResponse>, String> {
        let (sender, receiver) = async_channel::bounded(1);
        let _dialog = window
            .update(cx, |_, window, cx| {
                codesh_ui::dialog::confirm_with_prompt(
                    window,
                    cx,
                    &self.title(),
                    &self.message(),
                    self.action.settings_title(),
                    &self.prompt_choices(),
                    style,
                    move |response| {
                        let _ = sender.try_send(response);
                    },
                )
            })
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        Ok(receiver.recv().await.ok().flatten())
    }

    #[cfg(test)]
    pub(crate) async fn prompt(
        &self,
        window: AnyWindowHandle,
        _style: Style<'_>,
        cx: &mut AsyncApp,
    ) -> Result<Option<PromptResponse>, String> {
        let response = window
            .update(cx, |_, window, cx| {
                window.prompt(
                    gpui::PromptLevel::Info,
                    &self.title(),
                    Some(&self.message()),
                    &[self.action.settings_title(), "Cancel"],
                    cx,
                )
            })
            .map_err(|error| error.to_string())?;
        Ok((response.await == Ok(0)).then(PromptResponse::default))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn confirmation(staged: bool) -> AiConfirmation {
        let providers = crate::ai::PROVIDERS[..2].to_vec();
        AiConfirmation {
            project: ProjectId::new(),
            action: Action::Commit,
            provider: providers[0],
            providers,
            choices: CommitChoices {
                include_unstaged: false,
                push: true,
            },
            staged,
            branch: "feature".into(),
            head: None,
        }
    }

    fn response(picked: usize, checked: [bool; 2]) -> PromptResponse {
        PromptResponse {
            prompt: String::new(),
            picked: Some(picked),
            checked: checked.to_vec(),
        }
    }

    #[test]
    fn nothing_staged_includes_unstaged_changes_but_keeps_the_remembered_choice() {
        let mut confirmation = confirmation(false);
        let checkbox = &confirmation.prompt_choices().checkboxes[0];
        assert!(checkbox.checked && !checkbox.enabled);

        confirmation.confirm(&response(1, [true, true]));

        assert!(!confirmation.choices.include_unstaged);
        assert!(confirmation.commit_choices().include_unstaged);
    }
}
