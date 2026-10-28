use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use codesh_app_core::{ProjectId, ProjectStatus, ServerId};
use gpui::{Context, RenderImage};

use super::AppModel;
use crate::boot::Work;

/// Icons found among top-level projects' files, looked up once per folder.
#[derive(Default)]
pub(crate) struct ProjectIcons {
    next: u64,
    entries: HashMap<ProjectId, Entry>,
}

struct Entry {
    directory: PathBuf,
    request: u64,
    icon: Option<Arc<RenderImage>>,
}

impl AppModel {
    pub(crate) fn project_icon(&self, project: ProjectId) -> Option<Arc<RenderImage>> {
        self.project_icons.entries.get(&project)?.icon.clone()
    }

    pub(super) fn sync_project_icons(&mut self, cx: &mut Context<Self>) {
        let wanted: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| {
                !project.home
                    && project.parent_id.is_none()
                    && project.status() == ProjectStatus::Available
            })
            .map(|project| (project.id, project.server_id, project.directory.clone()))
            .collect();
        let mut retired = Vec::new();
        self.project_icons.entries.retain(|id, entry| {
            let keep = wanted
                .iter()
                .any(|(project, _, directory)| project == id && *directory == entry.directory);
            if !keep {
                retired.extend(entry.icon.take());
            }
            keep
        });
        drop_images(retired, cx);
        for (project, server, directory) in wanted {
            if self.project_icons.entries.contains_key(&project) || !self.ready(server) {
                continue;
            }
            self.project_icons.next = self.project_icons.next.wrapping_add(1);
            let request = self.project_icons.next;
            if self.send(server, Work::ProjectIcon { project, request }, cx) {
                self.project_icons.entries.insert(
                    project,
                    Entry {
                        directory,
                        request,
                        icon: None,
                    },
                );
            }
        }
    }

    pub(super) fn receive_project_icon(
        &mut self,
        project: ProjectId,
        request: u64,
        icon: Option<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self
            .project_icons
            .entries
            .get_mut(&project)
            .filter(|entry| entry.request == request)
        else {
            drop_images(icon, cx);
            return;
        };
        entry.icon = icon;
        cx.notify();
    }

    /// Looks again for icons `server` couldn't deliver once it reconnects.
    pub(super) fn forget_server_icons(&mut self, server: ServerId) {
        let projects: Vec<_> = self
            .project_icons
            .entries
            .iter()
            .filter(|(id, entry)| {
                entry.icon.is_none() && self.state.project_server(**id) == Some(server)
            })
            .map(|(id, _)| *id)
            .collect();
        for project in projects {
            self.project_icons.entries.remove(&project);
        }
    }
}

fn drop_images(images: impl IntoIterator<Item = Arc<RenderImage>>, cx: &mut Context<AppModel>) {
    for image in images {
        cx.defer(move |cx| cx.drop_image(image, None));
    }
}
