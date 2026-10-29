//! Remote Servers: Add Server…, Manage Servers…, and a page per server of
//! what can be done with it now.

#![expect(
    clippy::unused_self,
    reason = "Every page function takes the model, used or not"
)]

use std::rc::Rc;

use codesh_app_core::ServerId;
use codesh_ui::command_palette::{Item, PageContent, PageSpec};
use codesh_ui::fuzzy;
use gpui::{App, Context, Window};

use super::{Handler, Kind, PaletteEntry};
use crate::model::{AppModel, ServerStatus};
use crate::views::remote_servers::status_label;

impl AppModel {
    /// One server's actions in the command palette.
    pub(crate) fn open_remote_servers(
        &mut self,
        server: ServerId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.settings.server(server).is_none() {
            return;
        }
        self.open_palette(PaletteEntry::Server(server), window, cx);
    }

    pub(super) fn servers_spec(&self, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Servers.id(), "Search servers…").breadcrumb("Remote Servers")
    }

    pub(super) fn servers_items(
        &mut self,
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        let mut items = vec![
            command(
                "remote-add-server",
                "Add Server…",
                Rc::new(|model, _, cx| {
                    model.open_server_form(None, cx);
                }),
            ),
            command(
                "remote-manage-servers",
                "Manage Servers…",
                Rc::new(AppModel::open_servers_tab),
            ),
        ];
        items.extend(self.remote_servers().into_iter().map(|server| {
            let page = PaletteEntry::Server(server.entry.id).id();
            Item::page(page.clone(), server.entry.name.clone(), page)
                .kind(Kind::Server.label())
                .tile(Kind::Server.tile())
                .subtitle(status_label(&server))
        }));
        filtered(items, query)
    }

    pub(super) fn servers_opened(&mut self, _cx: &mut Context<Self>) {}

    pub(super) fn servers_action(
        &mut self,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    pub(super) fn server_spec(&self, server: ServerId, _cx: &App) -> PageSpec {
        let name = self.server_name(server).unwrap_or("Server").to_owned();
        PageSpec::new(PaletteEntry::Server(server).id(), "Search actions…").breadcrumb(name)
    }

    pub(super) fn server_items(
        &mut self,
        server: ServerId,
        query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        let Some(listed) = self
            .remote_servers()
            .into_iter()
            .find(|listed| listed.entry.id == server)
        else {
            return PageContent::empty("This remote server is no longer listed");
        };
        let handler =
            |action: fn(&mut AppModel, ServerId, &mut Window, &mut Context<Self>)| -> Handler {
                Rc::new(move |model, window, cx| {
                    if model.settings.server(server).is_some() {
                        action(model, server, window, cx);
                    }
                })
            };
        let connected = matches!(
            listed.status,
            ServerStatus::Connected | ServerStatus::Connecting
        );
        let mut items = vec![
            command(
                "new-tab",
                "New Terminal",
                handler(|model, id, _, cx| model.new_remote_home_tab(id, cx)),
            ),
            command(
                "add-project",
                "Add Project…",
                handler(|model, id, _, cx| model.open_remote_project_picker(id, cx)),
            ),
            command(
                "connect",
                if connected { "Reconnect" } else { "Connect" },
                handler(|model, id, _, cx| model.reconnect_remote_server(id, cx)),
            ),
        ];
        if connected {
            items.push(command(
                "disconnect",
                "Disconnect",
                handler(|model, id, _, cx| model.disconnect_remote_servers(&[id], cx)),
            ));
        }
        items.push(command(
            "edit",
            "Edit…",
            handler(|model, id, _, cx| model.open_server_form(Some(id), cx)),
        ));
        if listed.missing && crate::model::released_version().is_some() {
            items.push(command(
                "install",
                "Install Codesh…",
                handler(|model, id, window, cx| {
                    model.confirm_install_server(id, window.window_handle(), cx);
                }),
            ));
        }
        if listed.incompatible && crate::model::released_version().is_some() {
            items.push(command(
                "copy-update-command",
                "Copy Update Command",
                handler(|model, _, _, cx| model.copy_update_command(cx)),
            ));
        }
        items.push(command(
            "remove",
            "Remove…",
            handler(|model, id, window, cx| {
                model.confirm_forget_servers(vec![id], window.window_handle(), cx);
            }),
        ));
        filtered(items, query)
    }

    pub(super) fn server_opened(&mut self, _server: ServerId, _cx: &mut Context<Self>) {}

    pub(super) fn server_action(
        &mut self,
        _server: ServerId,
        _item: &str,
        _action: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    /// Shows servers' new states on an open Remote Servers page.
    pub(crate) fn refresh_servers_page(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.palette_page(cx),
            Some(PaletteEntry::Servers | PaletteEntry::Server(_))
        ) {
            self.refresh_palette(cx);
        }
    }
}

fn command(id: &'static str, title: &'static str, handler: Handler) -> Item<Handler> {
    Item::run(id, title, handler)
        .kind(Kind::Command.label())
        .tile(Kind::Server.tile())
}

fn filtered(items: Vec<Item<Handler>>, query: &str) -> PageContent<Handler> {
    let items = if query.is_empty() {
        items
    } else {
        fuzzy::rank(items, |item| item.title.as_ref(), query, usize::MAX)
            .into_iter()
            .map(|found| found.item.highlights(found.ranges))
            .collect()
    };
    if items.is_empty() {
        return PageContent::empty("No results");
    }
    PageContent::items(items)
}
