//! Each server's side connection: a second connection to the same server, so
//! work such as listing folders, file watches and AI requests never queues
//! behind the app's own requests or terminal frames.

use std::collections::{BTreeSet, HashMap, HashSet};

use codesh_app_core::ServerId;
use codesh_protocol::{FileChanges, FilesAction, FilesRequest, ProjectId};
use gpui::Context;

use super::AppModel;
use crate::boot::Work;

/// How many projects one connection may watch, as the server allows.
const WATCH_LIMIT: usize = 32;

#[derive(Default)]
pub(crate) struct SideClients {
    /// Each server's side connection, kept apart from the app's own.
    clients: HashMap<ServerId, codesh_client::Client>,
    /// Which install each server's connection is, so events from one that was
    /// replaced are ignored.
    installs: HashMap<ServerId, u64>,
    next_install: u64,
    /// Connections being opened, by server and connection generation, so
    /// only one opens at a time.
    opening: HashSet<(ServerId, u64)>,
    /// Projects whose file changes native panels want, watched again whenever
    /// their server's side connection is replaced.
    watched: BTreeSet<ProjectId>,
}

impl SideClients {
    /// The server's side connection, if it is open, without opening one.
    pub(crate) fn client(&self, server: ServerId) -> Option<codesh_client::Client> {
        self.clients.get(&server).cloned()
    }

    /// Whether native panels watch `project`'s files.
    pub(crate) fn watches(&self, project: ProjectId) -> bool {
        self.watched.contains(&project)
    }
}

impl Drop for SideClients {
    fn drop(&mut self) {
        for (_, client) in self.clients.drain() {
            client.disconnect();
        }
    }
}

impl AppModel {
    /// The server's side connection, which app work that must not queue
    /// behind terminals may share. When none is open yet, one starts opening
    /// and this returns `None`; try again once it connects.
    pub(crate) fn side_client(
        &mut self,
        server: ServerId,
        cx: &mut Context<Self>,
    ) -> Option<codesh_client::Client> {
        if let Some(client) = self.side_clients.client(server) {
            return Some(client);
        }
        self.connect_side_client(server, cx);
        None
    }

    /// Opens `server`'s side connection, through its worker, unless one is
    /// already opening for this connection.
    pub(crate) fn connect_side_client(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if !self.ready(server) {
            return;
        }
        let (sender, receiver) = async_channel::bounded(1);
        let generation = self.generation(server);
        if self.side_clients.opening.contains(&(server, generation)) {
            return;
        }
        if !self.send(server, Work::SideConnection(sender), cx) {
            return;
        }
        self.side_clients.opening.insert((server, generation));
        cx.spawn(async move |model, cx| {
            crate::diagnostics::event(
                "side_client.wait",
                format_args!("server={server} generation={generation}"),
            );
            let connected = receiver.recv().await;
            crate::diagnostics::event(
                "side_client.reply",
                format_args!("server={server} generation={generation} result={connected:?}"),
            );
            let _ = model.update(cx, |model, _| {
                model.side_clients.opening.remove(&(server, generation));
            });
            let Ok(connected) = connected else {
                return;
            };
            let connected = connected.and_then(forward_events);
            let (client, events) = match connected {
                Ok(connected) => connected,
                Err(error) => {
                    let _ = model.update(cx, |model, _| {
                        let message = model.server_message(server, &error);
                        crate::diagnostics::event("side_client", format_args!("{message}"));
                    });
                    return;
                }
            };
            let installed = model
                .update(cx, |model, cx| {
                    if model.generation(server) != generation || !model.ready(server) {
                        return None;
                    }
                    model.disconnect_side_client(server);
                    model.side_clients.next_install += 1;
                    let install = model.side_clients.next_install;
                    model.side_clients.clients.insert(server, client.clone());
                    model.side_clients.installs.insert(server, install);
                    crate::diagnostics::event(
                        "side_client.installed",
                        format_args!("server={server} generation={generation} install={install}"),
                    );
                    model.rewatch_files(server, cx);
                    model.native_side_ready(server, cx);
                    model.resume_remote(server, cx);
                    model.check_remote_projects(server, cx);
                    Some(install)
                })
                .ok()
                .flatten();
            let Some(install) = installed else {
                client.disconnect();
                return;
            };
            drop(client);
            while let Ok(event) = events.recv().await {
                if model
                    .update(cx, |model, cx| {
                        if model.generation(server) != generation
                            || model.side_clients.installs.get(&server) != Some(&install)
                        {
                            return;
                        }
                        match event {
                            codesh_client::ClientEvent::FilesChanged { project, changes } => {
                                model.files_changed(project, &changes, cx);
                            }
                            codesh_client::ClientEvent::Disconnected => {
                                model.disconnect_side_client(server);
                                let message =
                                    model.server_message(server, "side connection closed");
                                crate::diagnostics::event("side_client", format_args!("{message}"));
                                if model.ready(server) && model.quitting == super::Quitting::Idle {
                                    model.connect_side_client(server, cx);
                                }
                            }
                            _ => (),
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    /// Closes `server`'s side connection. Its file watches end with it.
    pub(super) fn disconnect_side_client(&mut self, server: ServerId) {
        self.side_clients.installs.remove(&server);
        if let Some(client) = self.side_clients.clients.remove(&server) {
            client.disconnect();
        }
    }

    /// A watched project's files changed on its server.
    ///
    /// Native panels (Files, editors) subscribe here.
    pub(crate) fn files_changed(
        &mut self,
        project: ProjectId,
        changes: &FileChanges,
        cx: &mut Context<Self>,
    ) {
        self.native_files_changed(project, changes, cx);
    }

    /// A project's repository changed on its server.
    ///
    /// Native panels (Source Control, diffs) subscribe here.
    pub(crate) fn git_changed(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.git_invalidated(project, cx);
        self.native_git_changed(project, cx);
    }

    /// Asks `project`'s server to report its file changes to
    /// [`Self::files_changed`], over the side connection. The watch is
    /// renewed whenever that connection is replaced. A server watches at most
    /// 32 projects per connection, so further ones are refused.
    pub(crate) fn watch_files(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        if self.side_clients.watched.contains(&project) {
            return;
        }
        let Some(server) = self.state.project_server(project) else {
            return;
        };
        let watching = self
            .side_clients
            .watched
            .iter()
            .filter(|watched| self.state.project_server(**watched) == Some(server))
            .count();
        if watching >= WATCH_LIMIT {
            crate::diagnostics::event(
                "side_client.watch",
                format_args!("refused project={project:?} reason=limit"),
            );
            return;
        }
        self.side_clients.watched.insert(project);
        if let Some(client) = self.side_client(server, cx) {
            Self::send_watches(client, vec![(project, FilesAction::Watch)], cx);
        }
    }

    /// Stops [`Self::watch_files`]' watch on `project`.
    pub(crate) fn unwatch_files(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        if !self.side_clients.watched.remove(&project) {
            return;
        }
        let Some(client) = self
            .state
            .project_server(project)
            .and_then(|server| self.side_clients.client(server))
        else {
            return;
        };
        Self::send_watches(client, vec![(project, FilesAction::Unwatch)], cx);
    }

    /// Watches again, on `server`'s new side connection, the projects native
    /// panels watch.
    fn rewatch_files(&mut self, server: ServerId, cx: &mut Context<Self>) {
        let Some(client) = self.side_clients.client(server) else {
            return;
        };
        let requests: Vec<_> = self
            .side_clients
            .watched
            .iter()
            .filter(|project| self.state.project_server(**project) == Some(server))
            .map(|project| (*project, FilesAction::Watch))
            .collect();
        if !requests.is_empty() {
            Self::send_watches(client, requests, cx);
        }
    }

    fn send_watches(
        client: codesh_client::Client,
        requests: Vec<(ProjectId, FilesAction)>,
        cx: &mut Context<Self>,
    ) {
        cx.background_executor()
            .spawn(async move {
                for (project, action) in requests {
                    if let Err(error) = client.files_async(FilesRequest { project, action }).await {
                        crate::diagnostics::event(
                            "side_client.watch",
                            format_args!("project={project:?} error={error}"),
                        );
                    }
                }
            })
            .detach();
    }
}

/// Forwards the side connection's file changes and its end from a thread of
/// their own, as the connection's events block.
fn forward_events(
    client: codesh_client::Client,
) -> Result<
    (
        codesh_client::Client,
        async_channel::Receiver<codesh_client::ClientEvent>,
    ),
    String,
> {
    let events = client
        .events()
        .ok_or("side connection events already taken")?;
    let (sender, receiver) = async_channel::bounded(64);
    std::thread::Builder::new()
        .name("side-client-events".into())
        .spawn(move || {
            for event in events {
                if matches!(
                    event,
                    codesh_client::ClientEvent::FilesChanged { .. }
                        | codesh_client::ClientEvent::Disconnected
                ) && sender.send_blocking(event).is_err()
                {
                    break;
                }
            }
        })
        .map_err(|error| error.to_string())?;
    Ok((client, receiver))
}
