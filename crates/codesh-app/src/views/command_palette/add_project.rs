//! Open Project: a folder browser over this computer's folders, or another
//! computer's through its server. `/` or `~/` browses paths, Tab completes
//! the highlighted folder, Enter opens and ⌘↩ adds.

#![expect(clippy::unused_self, reason = "A stub until its page is implemented")]

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use codesh_app_core::ServerId;
use codesh_ui::command_palette::{PageContent, PageSpec, Status};
use gpui::{App, Context, Window};

use super::{Handler, PaletteEntry};
use crate::model::{AppModel, PendingRemote, RemoteIntent};
use crate::picker::path_service::{self, DirectoryItem, TypedPathState};
use crate::picker::remote::RemoteFolders;
use crate::picker::search::SearchService;
use crate::picker::session::Session;
use crate::views::overlays::Overlay;

/// The folder browser behind an open Open Project page. Dropping it stops
/// its searches.
#[expect(dead_code, reason = "Open Project's page browses with it")]
pub(crate) struct AddProjectPage {
    /// Tells this page apart from one opened later.
    pub(crate) id: usize,
    pub(crate) server: ServerId,
    pub(crate) session: Session,
    pub(crate) search: SearchService,
    /// Another computer's folders, instead of this one's.
    pub(crate) remote: Option<RemoteFolders>,
    /// Why the folder couldn't be listed, when the server said.
    pub(crate) listing_error: Option<String>,
    pub(crate) generation: usize,
    pub(crate) cancelled: Arc<AtomicBool>,
    pub(crate) directory_cache: HashMap<String, Vec<DirectoryItem>>,
    pub(crate) directory_cache_order: Vec<String>,
    /// Folders that are already projects.
    pub(crate) added: HashSet<String>,
}

impl AddProjectPage {
    fn new(
        server: ServerId,
        session: Session,
        search: SearchService,
        remote: Option<RemoteFolders>,
        project_paths: Vec<String>,
    ) -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            server,
            session,
            search,
            remote,
            listing_error: None,
            generation: 0,
            cancelled: Arc::new(AtomicBool::new(false)),
            directory_cache: HashMap::new(),
            directory_cache_order: Vec::new(),
            added: project_paths.into_iter().collect(),
        }
    }

    /// This computer's folders, from `search_root`, warming the folder index
    /// in the background.
    pub(crate) fn local(
        search: SearchService,
        search_root: &str,
        project_paths: Vec<String>,
        cx: &mut App,
    ) -> Self {
        let session = Session::new(search_root, project_paths.clone());
        let page = Self::new(ServerId::local(), session, search, None, project_paths);
        let warm_root = page.session.search_root_path.clone();
        let warm_search = page.search.clone();
        let cancelled = page.cancelled.clone();
        cx.background_executor()
            .spawn(async move { warm_search.prepare(&warm_root, &cancelled) })
            .detach();
        page
    }

    /// Another computer's folders, from its Home.
    pub(crate) fn remote(
        server: ServerId,
        folders: RemoteFolders,
        project_paths: Vec<String>,
    ) -> Self {
        let session = Session::remote(&folders.home, project_paths.clone());
        Self::new(
            server,
            session,
            SearchService::new(),
            Some(folders),
            project_paths,
        )
    }

    fn entry(&self) -> PaletteEntry {
        if self.server.is_local() {
            PaletteEntry::AddProject
        } else {
            PaletteEntry::AddProjectRemote(self.server)
        }
    }
}

impl Drop for AddProjectPage {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl AppModel {
    pub(crate) fn open_project_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_palette(PaletteEntry::AddProject, window, cx);
    }

    /// Opens the same page on another computer's folders, from its Home.
    /// A server that isn't ready yet connects first, asking for its password
    /// if needed, and the page opens once it is.
    pub(crate) fn open_remote_project_picker(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if self.server_name(server).is_none() {
            return;
        }
        if self.remote_folders(server).is_none() {
            self.dismiss_overlay(cx);
            self.pending_remote = Some(PendingRemote::new(server, RemoteIntent::AddProject));
            if self.server_ready(server) && self.side_clients.client(server).is_none() {
                self.connect_side_client(server, cx);
            } else {
                self.connect_remote_server(server, cx);
            }
            cx.notify();
            return;
        }
        self.pending_remote = None;
        let window = self.window;
        let model = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = model.update(cx, |model, cx| {
                    model.open_palette(PaletteEntry::AddProjectRemote(server), window, cx);
                });
            });
        });
    }

    /// `server`'s folders through its side connection, once it is ready.
    fn remote_folders(&self, server: ServerId) -> Option<RemoteFolders> {
        let name = self.server_name(server)?.to_owned();
        let home = self.state.server_home(server)?;
        let (project, home) = (home.id, home.directory.to_string_lossy().into_owned());
        if !self.server_ready(server) || !self.confirmed(server) {
            return None;
        }
        let client = self.side_clients.client(server)?;
        Some(RemoteFolders::through(client, name, project, &home))
    }

    /// The open Open Project page's browser.
    pub(crate) fn add_project_page(&mut self) -> Option<&mut AddProjectPage> {
        match &mut self.overlay {
            Some(Overlay::Palette { add_project, .. }) => add_project.as_deref_mut(),
            _ => None,
        }
    }

    /// Which Open Project page is open, if one is.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "Tests tell pages apart with it")
    )]
    pub(crate) fn add_project_page_id(&self) -> Option<usize> {
        match &self.overlay {
            Some(Overlay::Palette {
                add_project: Some(page),
                ..
            }) => Some(page.id),
            _ => None,
        }
    }

    pub(super) fn add_project_spec(&self, server: ServerId, _cx: &App) -> PageSpec {
        if server.is_local() {
            return PageSpec::new(
                PaletteEntry::AddProject.id(),
                "Search folders or enter a path…",
            )
            .breadcrumb("Open Project")
            .completes_on_tab();
        }
        let name = self.server_name(server).unwrap_or("the server");
        PageSpec::new(
            PaletteEntry::AddProjectRemote(server).id(),
            format!("Enter a path on {name}…"),
        )
        .breadcrumb("Open Project")
        .completes_on_tab()
    }

    pub(super) fn add_project_items(
        &mut self,
        _server: ServerId,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
    }

    /// Starts the browser the page shows, unless it is already there.
    pub(super) fn add_project_opened(&mut self, server: ServerId, cx: &mut Context<Self>) {
        let Some(Overlay::Palette { add_project, .. }) = &self.overlay else {
            return;
        };
        if add_project
            .as_ref()
            .is_some_and(|page| page.server == server)
        {
            return;
        }
        let paths = self.project_paths(server);
        let page = if server.is_local() {
            let root = self
                .settings
                .projects
                .search_root
                .as_ref()
                .unwrap_or(&self.state.home().directory)
                .to_string_lossy()
                .into_owned();
            AddProjectPage::local(self.picker_search.clone(), &root, paths, cx)
        } else {
            let Some(folders) = self.remote_folders(server) else {
                return;
            };
            AddProjectPage::remote(server, folders, paths)
        };
        if let Some(Overlay::Palette { add_project, .. }) = &mut self.overlay {
            *add_project = Some(Box::new(page));
        }
    }

    pub(super) fn add_project_action(
        &mut self,
        _server: ServerId,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    pub(super) fn add_project_query_changed(
        &mut self,
        _server: ServerId,
        _query: &str,
        _cx: &mut Context<Self>,
    ) {
    }

    pub(super) fn add_project_completion(
        &mut self,
        _server: ServerId,
        _query: &str,
        _item: Option<&str>,
        _cx: &mut Context<Self>,
    ) {
    }

    pub(super) fn add_project_selection(
        &mut self,
        _server: ServerId,
        _item: Option<&str>,
        _cx: &mut Context<Self>,
    ) {
    }

    pub(crate) fn refresh_add_project_page(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.palette_page(cx),
            Some(PaletteEntry::AddProject | PaletteEntry::AddProjectRemote(_))
        ) {
            self.refresh_palette(cx);
        }
    }

    /// The folders of the projects the sidebar lists on `server`, besides
    /// another computer's Home, which is where its picker starts.
    fn project_paths(&self, server: ServerId) -> Vec<String> {
        self.state
            .projects()
            .iter()
            .filter(|project| {
                project.server_id == server
                    && self.project_listed(project)
                    && !project.is_remote_home()
            })
            .map(|project| project.directory.to_string_lossy().into_owned())
            .collect()
    }

    #[expect(
        dead_code,
        reason = "Open Project's page confirms remote paths with it"
    )]
    fn confirm_remote_project_path(
        &mut self,
        server: ServerId,
        path: &str,
        allow_create: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        let Some(page) = self.add_project_page() else {
            return;
        };
        let Some(folders) = page.remote.clone() else {
            return;
        };
        let (id, generation, entry) = (page.id, page.generation, page.entry());
        let path = path.to_owned();
        let checked_path = path.clone();
        let checking = folders.clone();
        let job = crate::picker::remote::next_job();
        let window = self.window;
        if let Some(palette) = self.palette_entity() {
            palette.update(cx, |palette, cx| {
                palette.set_status(&entry.id(), Status::Loading("Checking folder…".into()), cx);
            });
        }
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let mut result = cx
                .background_executor()
                .spawn(async move { checking.check(&checked_path, job).map(Some) })
                .await;
            let current = |cx: &gpui::AsyncApp| {
                model
                    .read_with(cx, |model, _| current_page(model, id, generation))
                    .unwrap_or(false)
            };
            if current(cx) && allow_create && result == Ok(Some(TypedPathState::Missing)) {
                let message = format!(
                    "Codesh will create \"{}\" on {} and add it as a project.",
                    &path, &folders.name
                )
                .to_string();
                result = match create_prompt(window, message, cx).await {
                    Ok(true) if current(cx) => {
                        let creating_path = path.clone();
                        cx.background_executor()
                            .spawn(async move {
                                folders
                                    .create(&creating_path, job)
                                    .map(|()| Some(TypedPathState::Directory))
                            })
                            .await
                    }
                    Ok(_) => Ok(None),
                    Err(error) => Err(error),
                };
            }
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                if !current_page(model, id, generation) {
                    return;
                }
                match result {
                    Ok(Some(TypedPathState::Directory)) => {
                        model.open_remote_project_path(server, &path, cx);
                    }
                    Ok(Some(TypedPathState::Missing)) => model.project_picker_error(
                        "Choose an existing folder, or use Create & Add Project.".to_string(),
                        cx,
                    ),
                    Ok(Some(TypedPathState::NotDirectory)) => model.project_picker_error(
                        "This path is a file. Choose a folder for the project.".to_string(),
                        cx,
                    ),
                    Ok(None) => model.refresh_add_project_page(cx),
                    Err(error) => model.project_picker_error(error, cx),
                }
                cx.notify();
            });
        }));
    }

    /// Adds a folder on another computer; its server checks that it exists.
    fn open_remote_project_path(&mut self, server: ServerId, path: &str, cx: &mut Context<Self>) {
        let path = path_service::standardize(path);
        let existing = self
            .state
            .projects()
            .iter()
            .find(|project| {
                project.server_id == server
                    && path_service::standardize(&project.directory.to_string_lossy()) == path
            })
            .map(|project| project.id);
        if self
            .state
            .server_home(server)
            .is_some_and(|home| Some(home.id) == existing)
        {
            let message = match self.server_name(server) {
                Some(name) => format!("Choose a folder inside the home folder on {name}."),
                None => "Choose a folder inside the home folder on the server.".to_string(),
            };
            self.project_picker_error(message.clone(), cx);
        } else if let Some(id) = existing {
            self.join_active_workspace(id, cx);
            self.select_project(id, cx);
            self.dismiss_overlay(cx);
        } else if self
            .add_project_on(server, PathBuf::from(path), cx)
            .is_some()
        {
            self.dismiss_overlay(cx);
        } else if let Some(error) = self.error.clone() {
            self.project_picker_error(error, cx);
        }
    }

    #[expect(dead_code, reason = "Open Project's page confirms local paths with it")]
    fn confirm_project_path(
        &mut self,
        path: &str,
        create_if_missing: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        let path = PathBuf::from(path_service::standardize(path));
        if !path.exists() && create_if_missing {
            let window = self.window;
            let message = format!(
                "Codesh will create \"{}\" and add it as a project.",
                path.display()
            )
            .to_string();
            self.close_prompt = Some(cx.spawn(async move |model, cx| {
                let response = create_prompt(window, message, cx).await;
                if response.as_ref().is_ok_and(|confirmed| *confirmed) {
                    let created_path = path.clone();
                    let result = cx
                        .background_executor()
                        .spawn(async move { std::fs::create_dir_all(created_path) })
                        .await;
                    let _ = model.update(cx, |model, cx| {
                        model.finish_confirmation(cx);
                        match result {
                            Ok(()) => model.open_project_path(path, cx),
                            Err(error) => model.project_picker_error(
                                format!("Could not create folder: {error}"),
                                cx,
                            ),
                        }
                    });
                } else {
                    let _ = model.update(cx, |model, cx| {
                        model.finish_confirmation(cx);
                        if let Err(error) = response {
                            model.project_picker_error(error, cx);
                        }
                        cx.notify();
                    });
                }
            }));
        } else {
            self.open_project_path(path, cx);
        }
    }

    pub(crate) fn open_folders_from(
        folders: async_channel::Receiver<Vec<String>>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Ok(urls) = folders.recv().await {
                if this
                    .update(cx, |model, cx| {
                        if urls.is_empty() {
                            model.raise_window(cx);
                        }
                        model
                            .pending_folders
                            .extend(urls.iter().filter_map(|url| folder_from_url(url)));
                        if model.can_open_local_folders() {
                            model.resume_folders(ServerId::local(), cx);
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

    pub(crate) fn resume_folders(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if server.is_local() {
            for folder in std::mem::take(&mut self.pending_folders) {
                self.open_folder(folder, cx);
            }
        }
    }

    fn open_folder(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        let canonical = folder.canonicalize().ok();
        let existing = self
            .state
            .projects()
            .iter()
            .find(|project| {
                project.server_id.is_local()
                    && canonical.is_some()
                    && project.directory.canonicalize().ok() == canonical
            })
            .map(|project| project.id);
        match existing {
            Some(id) => self.show_project(id, cx),
            None => self.open_project_path(folder, cx),
        }
    }

    fn open_project_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.is_dir() {
            self.project_picker_error(
                "Choose an existing folder, or use Create & Add Project.".to_string(),
                cx,
            );
            return;
        }
        let existing = self
            .state
            .projects()
            .iter()
            .find(|project| {
                project.server_id.is_local()
                    && path_service::standardize(&project.directory.to_string_lossy())
                        == path.to_string_lossy()
            })
            .map(|project| project.id);
        if let Some(id) = existing {
            self.show_project(id, cx);
        } else if self.add_project(path, cx).is_some() {
            self.dismiss_overlay(cx);
        } else if let Some(error) = self.error.clone() {
            self.project_picker_error(error, cx);
        }
    }

    fn show_project(&mut self, id: codesh_app_core::ProjectId, cx: &mut Context<Self>) {
        self.refresh_project_statuses(cx);
        self.join_active_workspace(id, cx);
        self.select_project(id, cx);
        self.dismiss_overlay(cx);
    }

    /// Shows `error` on the open Open Project page, or as the app's error.
    fn project_picker_error(&mut self, error: String, cx: &mut Context<Self>) {
        let entry = self.add_project_page().map(|page| page.entry());
        match (entry, self.palette_entity()) {
            (Some(entry), Some(palette)) => palette.update(cx, |palette, cx| {
                palette.set_status(&entry.id(), Status::Error(error.into()), cx);
            }),
            _ => self.fail(error, cx),
        }
    }

    #[expect(
        dead_code,
        reason = "Open Project's page offers Finder and its search location"
    )]
    fn choose_project_folder(
        &mut self,
        directory: String,
        search_location: bool,
        cx: &mut Context<Self>,
    ) {
        if self.close_prompt.is_some() {
            return;
        }
        self.dismiss_overlay(cx);
        let window = self.window;
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = folder_prompt(window, directory.into(), search_location, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                match response {
                    Ok(Some(path)) if search_location => model.save_project_search_root(path, cx),
                    Ok(Some(path)) => model.open_project_path(path, cx),
                    Ok(None) => {}
                    Err(error) => model.fail(error, cx),
                }
                cx.notify();
            });
        }));
    }
}

/// Whether the Open Project page `id` is still open at `generation`.
fn current_page(model: &AppModel, id: usize, generation: usize) -> bool {
    matches!(
        &model.overlay,
        Some(Overlay::Palette { add_project: Some(page), .. })
            if page.id == id && page.generation == generation
    )
}

fn folder_from_url(url: &str) -> Option<PathBuf> {
    let url = reqwest::Url::parse(url).ok()?;
    if url.scheme() != "file" || url.query().is_some() || url.fragment().is_some() {
        return None;
    }
    let path = url.to_file_path().ok()?;
    path.is_dir().then(|| path.components().collect())
}

#[cfg(not(test))]
async fn create_prompt(
    window: gpui::AnyWindowHandle,
    message: String,
    cx: &mut gpui::AsyncApp,
) -> Result<bool, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, window, cx| {
            codesh_ui::dialog::confirm(
                window,
                cx,
                "Create Project Folder?",
                &message,
                "Create & Add",
                None,
                move |response| {
                    let _ = sender.try_send(response);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    Ok(matches!(
        receiver.recv().await,
        Ok(codesh_ui::dialog::ConfirmationResponse::Confirmed { .. })
    ))
}

#[cfg(test)]
async fn create_prompt(
    window: gpui::AnyWindowHandle,
    message: String,
    cx: &mut gpui::AsyncApp,
) -> Result<bool, String> {
    let response = window
        .update(cx, |_, window, cx| {
            window.prompt(
                gpui::PromptLevel::Warning,
                "Create Project Folder?",
                Some(&message),
                &["Create & Add", "Cancel"],
                cx,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(response.await == Ok(0))
}

#[cfg(not(test))]
async fn folder_prompt(
    window: gpui::AnyWindowHandle,
    directory: PathBuf,
    search_location: bool,
    cx: &mut gpui::AsyncApp,
) -> Result<Option<PathBuf>, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let _dialog = window
        .update(cx, |_, _, cx| {
            codesh_ui::dialog::choose_folder(
                cx,
                if search_location {
                    "Select where Codesh searches for project folders"
                } else {
                    "Select a project folder"
                },
                &directory,
                move |path| {
                    let _ = sender.try_send(path);
                },
            )
        })
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    match receiver.recv().await {
        Ok(answer) => answer.map_err(|error| error.to_string()),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
async fn folder_prompt(
    _: gpui::AnyWindowHandle,
    _: PathBuf,
    _: bool,
    cx: &mut gpui::AsyncApp,
) -> Result<Option<PathBuf>, String> {
    let response = cx.update(|cx| {
        cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        })
    });
    response
        .map_err(|error| error.to_string())?
        .await
        .map_err(|error| error.to_string())?
        .map(|paths| paths.and_then(|paths| paths.into_iter().next()))
        .map_err(|error| error.to_string())
}
