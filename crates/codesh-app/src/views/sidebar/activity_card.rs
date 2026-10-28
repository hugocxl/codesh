//! Card content is derived exclusively from model caches.
use codesh_app_core::{Pane, PaneContent, Project, Tab};
use codesh_protocol::{AgentProvider, AgentState};

use crate::model::AppModel;

/// The card's top line: what the worktree is doing right now.
pub(super) enum Headline {
    /// A command running in the active pane, as its terminal titles it.
    Command(String),
    /// A browser pane, by its page title.
    Browser(String),
    Editor(String),
    Diff(String),
    /// An agent and what it is working on.
    Agent {
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
    pub directory: Option<String>,
    pub branch: Option<String>,
    pub pr: Option<u64>,
    /// An agent in the worktree is working.
    pub working: bool,
    pub provider: Option<AgentProvider>,
}

impl Data {
    pub(super) fn new(project: &Project, model: &AppModel) -> Self {
        Self::from_tabs(project, &project.tabs, model)
    }

    pub(super) fn for_tab(project: &Project, tab: &Tab, model: &AppModel) -> Self {
        Self::from_tabs(project, std::slice::from_ref(tab), model)
    }

    fn from_tabs(project: &Project, tabs: &[Tab], model: &AppModel) -> Self {
        let repo = model
            .git
            .projects
            .get(&project.id)
            .filter(|_| !project.home);
        let summary = repo.and_then(|repo| repo.summary.as_ref());
        let pr = repo.and_then(|repo| repo.pull_request.as_ref());
        let snapshot = model
            .activity(project.server_id)
            .map(|activity| &activity.snapshot);
        // Session IDs are server-scoped. Resolve through this worktree's panes,
        // as the existing agents panel does, including mirrored sessions.
        let agent_in = |pane: &Pane| {
            let PaneContent::Terminal {
                session: Some(session),
            } = pane.content
            else {
                return None;
            };
            snapshot?
                .agents
                .iter()
                .find(|agent| agent.session == session)
        };
        let working = tabs
            .iter()
            .flat_map(|tab| &tab.panes)
            .filter_map(agent_in)
            .any(|agent| agent.state == AgentState::Working);
        // The card speaks for the pane the user has selected, so switching
        // panes switches the headline.
        let selected = model.state.window().selected_tab.get(&project.id);
        let tab = tabs
            .iter()
            .find(|tab| Some(&tab.id) == selected)
            .or_else(|| tabs.first());
        let shown_id = tab.and_then(|tab| model.state.shown_pane(tab));
        let shown_pane = tab.and_then(|tab| tab.displayed_pane(shown_id));
        let shown = tab.map(|tab| tab.title(shown_id).trim());
        let agent = shown_pane.and_then(agent_in);
        let browsing =
            shown_pane.is_some_and(|pane| matches!(pane.content, PaneContent::Browser(_)));
        let headline =
            if let Some(headline) = shown_pane.and_then(|pane| file_headline(&pane.content)) {
                headline
            } else if let Some(agent) = agent {
                // Agents title their terminal with the current task after a
                // spinner glyph; until they do, the card names the agent. An older
                // session's prompt would describe some other conversation.
                let task = shown.and_then(|title| task_title(title, agent.provider.name()));
                Headline::Agent { task }
            } else {
                match shown.filter(|title| !title.is_empty()) {
                    None => Headline::Empty,
                    Some(title) if browsing => Headline::Browser(title.to_owned()),
                    Some(title) => match idle_directory(title) {
                        Some(directory) => Headline::Shell {
                            shell: shell_name(),
                            // The worktree's own folder goes without saying, and
                            // a standalone terminal's card names it below.
                            directory: (!project.home && directory != project.name)
                                .then_some(directory),
                        },
                        None => Headline::Command(without_host(title).to_owned()),
                    },
                }
            };
        Self {
            headline,
            directory: shown_pane.and_then(|pane| idle_directory(&pane.title)),
            // A branch named like its worktree adds nothing to the card.
            branch: summary
                .and_then(|summary| summary.branch.clone())
                .filter(|branch| *branch != project.name),
            pr: pr.map(|pr| pr.number),
            working,
            provider: agent.map(|agent| agent.provider),
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

/// Drops the `(host) ` prefix some prompts, like Pure, put before the
/// directory or running command.
fn without_host(title: &str) -> &str {
    title
        .strip_prefix('(')
        .and_then(|rest| rest.split_once(") "))
        .filter(|(host, _)| !host.is_empty() && !host.contains(char::is_whitespace))
        .map_or(title, |(_, rest)| rest.trim_start())
}

/// An idle shell titles its terminal with its directory, as `user@host:dir`,
/// `(host) dir` or the bare path; returns the directory, shortened to its last
/// component (or `~`).
fn idle_directory(title: &str) -> Option<String> {
    let title = without_host(title.trim());
    let directory = if title.starts_with(['~', '/']) {
        // A bare path with spaces is more likely a command line.
        if title.contains(char::is_whitespace) {
            return None;
        }
        title
    } else {
        let (user, rest) = title.split_once('@')?;
        let (host, directory) = rest.split_once(':')?;
        if user.is_empty()
            || host.is_empty()
            || user.contains(char::is_whitespace)
            || host.contains(char::is_whitespace)
        {
            return None;
        }
        directory
    };
    if directory.is_empty() {
        return None;
    }
    let directory = directory.trim_end_matches('/');
    if directory.is_empty() {
        return Some("/".to_owned());
    }
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
        assert_eq!(idle_directory("hugo@mini:/").as_deref(), Some("/"));
        assert_eq!(idle_directory("hugo@mini:"), None);
        assert_eq!(idle_directory("htop"), None);
        assert_eq!(idle_directory("ssh hugo@mini-pro: deploy"), None);
        assert_eq!(
            idle_directory("(mac-mini-m2-pro) ~/repos/vitruve").as_deref(),
            Some("vitruve")
        );
        assert_eq!(idle_directory("(mac-mini-m2-pro) ~").as_deref(), Some("~"));
        assert_eq!(idle_directory("(mac-mini-m2-pro) cargo test"), None);
        assert_eq!(without_host("(mac-mini-m2-pro) cargo test"), "cargo test");
        assert_eq!(without_host("(cd x) && make"), "(cd x) && make");
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
