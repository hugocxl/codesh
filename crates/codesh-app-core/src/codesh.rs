//! Codesh's per-project single split-container policy.

use crate::{
    AppError, AppState, Axis, Direction, Layout, Pane, PaneId, Project, ProjectId, Tab, TabId,
};

pub const SINGLE_TAB: bool = true;

/// Merge whole layout subtrees in tab order, retaining every pane and session.
/// Empty projects stay empty; no terminal session is created by normalization.
pub fn normalize(project: &mut Project) {
    if !SINGLE_TAB || project.home {
        return;
    }
    let merged = project.tabs.len() > 1 || project.groups.is_some();
    project.groups = None;
    let mut tabs = std::mem::take(&mut project.tabs).into_iter();
    if let Some(mut survivor) = tabs.next() {
        if merged {
            survivor.zoomed = None;
        }
        for tab in tabs {
            survivor.layout = Layout::Split {
                axis: Axis::Horizontal,
                ratio: 0.5,
                first: Box::new(survivor.layout),
                second: Box::new(tab.layout),
            };
            survivor.panes.extend(tab.panes);
        }
        project.tabs.push(survivor);
    }
}

impl AppState {
    pub(crate) fn single_tab(&self, project: ProjectId) -> bool {
        SINGLE_TAB && self.project(project).is_some_and(|project| !project.home)
    }

    /// Insert typed content to the right of the project's focused leaf, or its
    /// last leaf. Does not change window focus; callers choose whether to select.
    pub fn open_pane_in_container(
        &mut self,
        project: ProjectId,
        pane: Pane,
        focus_leaf: Option<PaneId>,
    ) -> Result<(TabId, PaneId), AppError> {
        let target = self
            .project(project)
            .ok_or(AppError::UnknownProject(project))?;
        target.require_available()?;
        let id = pane.id;
        if let Some(tab) = target.tabs.first() {
            let tab_id = tab.id;
            let leaf = focus_leaf
                .filter(|leaf| tab.layout.contains(*leaf))
                .or_else(|| tab.layout.leaves().last().copied())
                .ok_or_else(|| AppError::InvalidState("container has no panes".into()))?;
            self.insert_split_pane(leaf, pane, Direction::Right)?;
            return Ok((tab_id, id));
        }
        let mut tab = Tab::terminal();
        tab.layout = Layout::Leaf(id);
        tab.panes = vec![pane];
        let tab_id = tab.id;
        self.add_tab(project, tab, None)?;
        Ok((tab_id, id))
    }

    /// Normalize persisted/imported projects and repair window references.
    pub(crate) fn normalize_projects(&mut self) {
        if !SINGLE_TAB {
            return;
        }
        for project in &mut self.projects {
            if project.home {
                continue;
            }
            normalize(project);
            if let Some(tab) = project.tabs.first() {
                self.window.selected_tab.insert(project.id, tab.id);
            } else {
                self.window.selected_tab.remove(&project.id);
            }
        }
        self.focus_selected_tab();
    }
}
