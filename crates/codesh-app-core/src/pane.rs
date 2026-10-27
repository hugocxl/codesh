use codesh_protocol::SessionId;
use serde::{Deserialize, Serialize};

use crate::{AppError, AppState, PaneId, ProjectId, Tab};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Pane {
    pub id: PaneId,
    pub title: String,
    pub content: PaneContent,
    /// The terminal started with the app's proxy.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub proxy: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaneContent {
    Terminal {
        session: Option<SessionId>,
    },
    Settings,
    Browser(crate::browser::BrowserPane),
    /// The Remote Servers tab, which manages other computers' servers.
    Servers,
    /// A file editor; `path` is project-relative with `/` separators.
    Editor {
        path: String,
    },
    /// One file's working tree (or, when `staged`, index) diff.
    Diff {
        path: String,
        staged: bool,
    },
}

impl PaneContent {
    /// The tab title for an editor or diff pane of `path`.
    pub fn native_title(&self) -> Option<String> {
        match self {
            Self::Editor { path } => Some(basename(path).into()),
            Self::Diff { path, staged } => Some(if *staged {
                format!("{} (staged)", basename(path))
            } else {
                basename(path).into()
            }),
            Self::Terminal { .. } | Self::Settings | Self::Browser(_) | Self::Servers => None,
        }
    }
}

fn basename(path: &str) -> &str {
    path.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(path)
}

pub(crate) fn relative_path(path: &str) -> Result<String, AppError> {
    let path = path.trim_start_matches("./").trim_end_matches('/');
    if path.is_empty() || path.starts_with('/') || path.split('/').any(|part| part == "..") {
        return Err(AppError::InvalidState(
            "pane path must be relative to the project".into(),
        ));
    }
    Ok(path.into())
}

impl AppState {
    /// Opens `path` in an editor pane beside the focused pane, or focuses the
    /// project's existing editor for it.
    pub fn open_editor_pane(&mut self, project: ProjectId, path: &str) -> Result<PaneId, AppError> {
        let path = relative_path(path)?;
        self.open_native_pane(project, PaneContent::Editor { path })
    }

    /// Opens `path`'s diff beside the focused pane, or focuses the project's
    /// existing diff of the same path and side.
    pub fn open_diff_pane(
        &mut self,
        project: ProjectId,
        path: &str,
        staged: bool,
    ) -> Result<PaneId, AppError> {
        let path = relative_path(path)?;
        self.open_native_pane(project, PaneContent::Diff { path, staged })
    }

    fn open_native_pane(
        &mut self,
        project: ProjectId,
        content: PaneContent,
    ) -> Result<PaneId, AppError> {
        let existing = self
            .project(project)
            .ok_or(AppError::UnknownProject(project))?
            .tabs
            .iter()
            .flat_map(|tab| &tab.panes)
            .find(|pane| pane.content == content)
            .map(|pane| pane.id);
        if let Some(pane) = existing {
            self.select_project(project)?;
            self.focus_pane(pane)?;
            return Ok(pane);
        }
        let title = content.native_title().unwrap_or_default();
        if crate::codesh::SINGLE_TAB {
            let pane = Pane {
                id: PaneId::new(),
                title,
                content,
                proxy: false,
            };
            let (tab, pane) =
                self.open_pane_in_container(project, pane, self.window.active_pane)?;
            self.select_project(project)?;
            self.select_tab(project, tab)?;
            self.focus_pane(pane)?;
            return Ok(pane);
        }
        let tab = Tab::with_content(content, &title);
        let id = tab.id;
        let pane = tab.panes[0].id;
        let focused = self.window.selected_tab.get(&project).copied();
        self.add_tab(project, tab, focused)?;
        self.select_project(project)?;
        self.select_tab(project, id)?;
        Ok(pane)
    }
}
