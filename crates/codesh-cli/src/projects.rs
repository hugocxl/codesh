//! The order projects are listed in: Home first, then each project followed
//! by its worktrees.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use codesh_protocol::{CatalogPage, ProjectDescriptor, ProjectId, ServerPath};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Entry<'a> {
    pub project: &'a ProjectDescriptor,
    pub worktree: bool,
    /// The last worktree under its parent.
    pub last: bool,
}

pub(crate) fn tree(catalog: &CatalogPage) -> Vec<Entry<'_>> {
    let tops: BTreeSet<ProjectId> = catalog
        .projects
        .iter()
        .filter(|project| project.parent_id.is_none())
        .map(|project| project.id)
        .collect();
    // A worktree whose parent is gone is listed on its own.
    let mut roots = Vec::new();
    let mut children: BTreeMap<ProjectId, Vec<&ProjectDescriptor>> = BTreeMap::new();
    for project in &catalog.projects {
        match project.parent_id.filter(|parent| tops.contains(parent)) {
            Some(parent) => children.entry(parent).or_default().push(project),
            None => roots.push(project),
        }
    }
    roots.sort_by_key(|project| !project.home);
    let mut entries = Vec::with_capacity(catalog.projects.len());
    for root in roots {
        entries.push(Entry {
            project: root,
            worktree: false,
            last: false,
        });
        let children = children.remove(&root.id).unwrap_or_default();
        let count = children.len();
        entries.extend(
            children
                .into_iter()
                .enumerate()
                .map(|(index, project)| Entry {
                    project,
                    worktree: true,
                    last: index + 1 == count,
                }),
        );
    }
    entries
}

/// The project before or after `current` in list order, wrapping around.
pub(crate) fn cycle(catalog: &CatalogPage, current: ProjectId, forward: bool) -> Option<ProjectId> {
    let entries = tree(catalog);
    let count = entries.len();
    let index = entries
        .iter()
        .position(|entry| entry.project.id == current)?;
    let next = if forward {
        (index + 1) % count
    } else {
        (index + count - 1) % count
    };
    Some(entries[next].project.id)
}

/// `directory` for display, with the home folder shortened to `~`.
pub(crate) fn display_path(catalog: &CatalogPage, directory: &ServerPath) -> String {
    let path = Path::new(OsStr::from_bytes(&directory.0));
    let shortened = catalog
        .projects
        .iter()
        .find(|project| project.home)
        .and_then(|home| {
            codesh_core::dirs::abbreviate_home(
                path,
                Path::new(OsStr::from_bytes(&home.directory.0)),
            )
        });
    crate::ui::clean(&shortened.as_deref().unwrap_or(path).to_string_lossy())
}
