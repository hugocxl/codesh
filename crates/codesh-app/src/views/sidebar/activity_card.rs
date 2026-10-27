//! Card content is derived exclusively from model caches.
use std::time::{SystemTime, UNIX_EPOCH};

use codesh_app_core::{PaneContent, Project};
use codesh_protocol::{AgentProvider, AgentState};
use codesh_ui::tr;

use crate::model::AppModel;

/// The card's top line: what the worktree is doing right now.
pub(super) enum Headline {
    /// A command running in the active pane, as its terminal titles it.
    Command(String),
    /// A browser pane, by its page title.
    Browser(String),
    Editor(String),
    Diff(String),
    /// An agent, with its state and what it is working on.
    Agent {
        state: AgentState,
        task: Option<String>,
    },
    /// An idle shell: its name and working directory.
    Shell {
        shell: String,
        directory: Option<String>,
    },
    /// No terminal is open in the worktree.
    Empty,
}

pub(super) struct Data {
    pub headline: Headline,
    pub branch: Option<String>,
    pub pr: Option<u64>,
    pub age: Option<String>,
    pub provider: Option<AgentProvider>,
}

impl Data {
    pub(super) fn new(project: &Project, model: &AppModel) -> Self {
        let repo = model.git.projects.get(&project.id);
        let summary = repo.and_then(|repo| repo.summary.as_ref());
        let pr = repo.and_then(|repo| repo.pull_request.as_ref());
        let snapshot = model
            .activity(project.server_id)
            .map(|activity| &activity.snapshot);
        // Session IDs are server-scoped. Resolve through this worktree's panes,
        // as the existing agents panel does, including mirrored sessions.
        let agent = snapshot.and_then(|snapshot| {
            project
                .tabs
                .iter()
                .flat_map(|tab| tab.panes.iter().map(move |pane| (tab, pane)))
                .filter_map(|(tab, pane)| {
                    let PaneContent::Terminal {
                        session: Some(session),
                    } = pane.content
                    else {
                        return None;
                    };
                    snapshot
                        .agents
                        .iter()
                        .find(|agent| agent.session == session)
                        .map(|agent| (agent, tab, pane))
                })
                .min_by_key(|(agent, _, _)| match agent.state {
                    AgentState::Blocked => 0,
                    AgentState::Working => 1,
                    _ => 2,
                })
        });
        let latest = model.agent_sessions.latest.get(&project.id);
        let shown_pane = project
            .tabs
            .first()
            .and_then(|tab| tab.displayed_pane(model.state.shown_pane(tab)));
        let shown = project
            .tabs
            .first()
            .map(|tab| tab.title(model.state.shown_pane(tab)).trim());
        let browsing =
            shown_pane.is_some_and(|pane| matches!(pane.content, PaneContent::Browser(_)));
        let headline =
            if let Some(headline) = shown_pane.and_then(|pane| file_headline(&pane.content)) {
                headline
            } else if let Some((agent, tab, pane)) = agent {
                // Agents title their terminal with the current task after a
                // spinner glyph; until they do, the card names the agent. An older
                // session's prompt would describe some other conversation.
                let task = task_title(tab.title(Some(pane.id)), agent.provider.name());
                Headline::Agent {
                    state: agent.state,
                    task,
                }
            } else {
                match shown.filter(|title| !title.is_empty()) {
                    None => Headline::Empty,
                    Some(title) if browsing => Headline::Browser(title.to_owned()),
                    Some(title) => match idle_directory(title) {
                        Some(directory) => Headline::Shell {
                            shell: shell_name(),
                            // The worktree's own folder goes without saying.
                            directory: (directory != project.name).then_some(directory),
                        },
                        None => Headline::Command(title.to_owned()),
                    },
                }
            };
        let timestamp = snapshot
            .into_iter()
            .flat_map(|snapshot| &snapshot.events)
            .filter(|event| event.project == project.id)
            .map(|event| event.timestamp)
            .chain(
                latest
                    .and_then(|session| session.modified.duration_since(UNIX_EPOCH).ok())
                    .map(|age| age.as_secs()),
            )
            .chain(summary.and_then(|summary| summary.commit_timestamp))
            .max();
        let working = agent.is_some_and(|(agent, _, _)| agent.state == AgentState::Working);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            headline,
            // A branch named like its worktree adds nothing to the card.
            branch: summary
                .and_then(|summary| summary.branch.clone())
                .filter(|branch| *branch != project.name),
            pr: pr.map(|pr| pr.number),
            age: if working {
                Some(relative_age(0))
            } else {
                timestamp.map(|time| relative_age(now.saturating_sub(time)))
            },
            provider: agent.map(|(agent, _, _)| agent.provider),
        }
    }
}

fn file_headline(content: &PaneContent) -> Option<Headline> {
    match content {
        PaneContent::Editor { path } => Some(Headline::Editor(path.rsplit('/').next()?.to_owned())),
        PaneContent::Diff { path, .. } => Some(Headline::Diff(path.rsplit('/').next()?.to_owned())),
        _ => None,
    }
}

/// An idle shell titles its terminal `user@host:directory`; returns the
/// directory, shortened to its last component (or `~`).
fn idle_directory(title: &str) -> Option<String> {
    let (user, rest) = title.split_once('@')?;
    let (host, directory) = rest.split_once(':')?;
    if user.is_empty()
        || host.is_empty()
        || user.contains(char::is_whitespace)
        || host.contains(char::is_whitespace)
    {
        return None;
    }
    let directory = directory.trim_end_matches('/');
    Some(match directory.rsplit_once('/') {
        Some((_, last)) if !last.is_empty() => last.to_owned(),
        _ => directory.to_owned(),
    })
}

/// The agent's task from its terminal title, without spinner glyphs; `None`
/// when the title only names the agent.
fn task_title(title: &str, provider: &str) -> Option<String> {
    let task = title
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim();
    (!task.is_empty() && !task.eq_ignore_ascii_case(provider) && task != "Claude Code")
        .then(|| task.to_owned())
}

fn shell_name() -> String {
    std::env::var("SHELL")
        .ok()
        .and_then(|shell| shell.rsplit('/').next().map(str::to_owned))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "shell".to_owned())
}

fn relative_age(seconds: u64) -> String {
    match seconds {
        0..60 => tr!("now").to_string(),
        60..3600 => tr!("%lldm", seconds / 60).to_string(),
        3600..86_400 => tr!("%lldh", seconds / 3600).to_string(),
        _ => tr!("%lldd", seconds / 86_400).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_shell_titles_give_their_directory() {
        assert_eq!(
            idle_directory("hugo@Hugos-MacBook-Air:~/repos/codesh").as_deref(),
            Some("codesh")
        );
        assert_eq!(idle_directory("hugo@mini:~").as_deref(), Some("~"));
        assert_eq!(idle_directory("htop"), None);
        assert_eq!(idle_directory("ssh hugo@mini-pro: deploy"), None);
    }

    #[test]
    fn agent_titles_drop_spinners_and_bare_product_names() {
        assert_eq!(
            task_title("✳ Fix the merge errors", "Claude").as_deref(),
            Some("Fix the merge errors")
        );
        assert_eq!(task_title("⠂ Claude Code", "Claude"), None);
        assert_eq!(task_title("Codex", "Codex"), None);
    }
}
