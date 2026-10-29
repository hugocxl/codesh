//! Remote Servers: Add Server…, Manage Servers…, and a page per server of
//! what can be done with it now.

#![expect(clippy::unused_self, reason = "A stub until its page is implemented")]

use codesh_app_core::ServerId;
use codesh_ui::command_palette::{PageContent, PageSpec};
use gpui::{App, Context, Window};

use super::{Handler, PaletteEntry};
use crate::model::AppModel;

impl AppModel {
    /// One server's actions in the command palette.
    pub(crate) fn open_remote_servers(
        &mut self,
        server: ServerId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_palette(PaletteEntry::Server(server), window, cx);
    }

    pub(super) fn servers_spec(&self, _cx: &App) -> PageSpec {
        PageSpec::new(PaletteEntry::Servers.id(), "Search servers…").breadcrumb("Remote Servers")
    }

    pub(super) fn servers_items(
        &mut self,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
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
        _server: ServerId,
        _query: &str,
        _cx: &mut Context<Self>,
    ) -> PageContent<Handler> {
        PageContent::empty("Not implemented yet")
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
