use gpui::{
    AnyElement, Context, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Pixels, Point, Styled, Window, div,
};

use super::menu::{self, Item, Menu};
use crate::model::AppModel;
use crate::model::tips::TipPlacement;

pub(crate) enum Overlay {
    Updates,
    Server,
    /// The tip popover beside the collapsed sidebar's tip button.
    Tip,
    /// The command palette; `add_project` is the folder browser behind its
    /// Open Project page.
    Palette {
        palette: Entity<codesh_ui::command_palette::Palette<super::command_palette::Handler>>,
        add_project: Option<Box<super::command_palette::add_project::AddProjectPage>>,
    },
    Git(super::git::GitPicker),
    GitForm(Box<super::git::Form>),
    AiProvider(super::git::AiProviderMenu),
    PullRequest(super::git::PullRequestPopover),
    Menu(Menu),
    ProjectEditor(super::project_editor::Editor),
    /// The popover above the sidebar's Remote section.
    RemoteServers,
    Agents,
    SessionProviders,
    ServerForm(Box<super::remote_servers::ServerForm>),
    Password(Box<super::remote_servers::PasswordPrompt>),
    /// The status bar's proxy popover.
    Proxy(Box<super::proxy::ProxyForm>),
}

impl Overlay {
    /// Whether native browser views can avoid the area the overlay covers.
    pub(crate) fn marks_bounds(&self) -> bool {
        matches!(
            self,
            Self::Updates
                | Self::Server
                | Self::Tip
                | Self::RemoteServers
                | Self::Agents
                | Self::SessionProviders
                | Self::Git(_)
                | Self::GitForm(_)
                | Self::AiProvider(_)
                | Self::PullRequest(_)
                | Self::Menu(_)
                | Self::ProjectEditor(_)
                | Self::ServerForm(_)
                | Self::Password(_)
                | Self::Proxy(_)
        )
    }
}

impl AppModel {
    pub(crate) fn dismiss_overlay(&mut self, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::ServerForm(_))) {
            self.clear_server_probe();
        }
        self.git.interaction = self.git.interaction.wrapping_add(1);
        if matches!(self.overlay, Some(Overlay::Password(_))) {
            self.pending_remote = None;
        }
        let closed = self.overlay.take();
        let back_to_remote =
            matches!(&closed, Some(Overlay::Menu(menu)) if menu.over_remote_servers);
        self.focus_return = match closed {
            Some(Overlay::Menu(menu)) => menu.returns_focus_to,
            Some(Overlay::SessionProviders) if self.appearance.agent_sessions_visible => self
                .agent_sessions
                .input
                .as_ref()
                .map(|input| gpui::Focusable::focus_handle(input, cx).downgrade()),
            _ => None,
        };
        self.overlay_subscription = None;
        self.focus_requested = true;
        if back_to_remote {
            self.overlay = Some(Overlay::RemoteServers);
        }
        cx.notify();
    }

    pub(crate) fn open_menu(
        &mut self,
        items: Vec<Item>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::Menu(Menu::new(items, position)));
        self.overlay_focus.focus(window);
        // A focusable ancestor of the clicked row would otherwise take focus back.
        window.prevent_default();
        cx.notify();
    }
}

pub(crate) use codesh_ui::popover::clamp_to_viewport as clamp;

/// `content` with a mark of the area it covers.
fn marked(content: AnyElement, model: &AppModel) -> AnyElement {
    div()
        .relative()
        .child(content)
        .child(model.webview_occlusion())
        .into_any_element()
}

/// The Remote popover above the sidebar's Remote section.
fn remote_servers(content: AnyElement, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let content = marked(content, model);
    let anchor = model.remote_anchor.clone();
    let model = cx.entity().downgrade();
    codesh_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
        let _ = model.update(cx, AppModel::dismiss_overlay);
    })
}

fn centered(content: AnyElement) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(content)
        .into_any_element()
}

#[allow(
    clippy::too_many_lines,
    reason = "Exhaustive overlay rendering dispatch"
)]
pub(crate) fn layer(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let content = match &model.overlay {
        None => return div().into_any_element(),
        Some(Overlay::Server) => {
            let content = marked(super::server_status::render(model, window, cx), model);
            let anchor = model.server_anchor();
            if !model.appearance.status_bar_visible {
                anchor.set(None);
            }
            let model = cx.entity().downgrade();
            codesh_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            })
        }
        Some(Overlay::Proxy(form)) => {
            let content = marked(super::proxy::render(form, model, window, cx), model);
            if model.appearance.status_bar_visible {
                let anchor = model.proxy.anchor.clone();
                let model = cx.entity().downgrade();
                codesh_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
                    let _ = model.update(cx, AppModel::dismiss_overlay);
                })
            } else {
                centered(content)
            }
        }
        Some(Overlay::Updates) => {
            let content = marked(super::updates::render(model, window, cx), model);
            let anchor = model.update_anchor();
            if !model.appearance.status_bar_visible {
                anchor.set(None);
            }
            let model = cx.entity().downgrade();
            codesh_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            })
        }
        Some(Overlay::Tip) => {
            let content = marked(super::sidebar::tips::popover_card(model, cx), model);
            let anchor = model.tips.anchor.clone();
            if model.tip_placement() != Some(TipPlacement::Button) {
                anchor.set(None);
            }
            let model = cx.entity().downgrade();
            codesh_ui::popover::anchored_popover_above(anchor, content, move |_, cx| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            })
        }
        Some(Overlay::RemoteServers) => {
            remote_servers(super::remote_servers::popover(model, window, cx), model, cx)
        }
        Some(Overlay::Agents) => super::agents_panel::popup(model, window, cx),
        Some(Overlay::SessionProviders) => {
            let content = marked(super::agent_sessions::providers(model, window, cx), model);
            let anchor = model.agent_sessions.anchor.clone();
            let owner = cx.weak_entity();
            codesh_ui::popover::anchored_popover(anchor, content, move |_, cx| {
                let _ = owner.update(cx, AppModel::dismiss_overlay);
            })
        }
        Some(Overlay::ServerForm(form)) => centered(marked(
            super::remote_servers::render_form(form, model, window, cx),
            model,
        )),
        Some(Overlay::Password(prompt)) => centered(marked(
            super::remote_servers::render_password(prompt, model, window, cx),
            model,
        )),
        Some(Overlay::GitForm(form)) => div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(marked(
                super::git::render_form(form, model, window, cx),
                model,
            ))
            .into_any_element(),
        Some(Overlay::AiProvider(menu)) => {
            let entity = cx.entity().downgrade();
            codesh_ui::popover::anchored_popover_above(
                model.ai.anchors[menu.action.index()].clone(),
                marked(
                    super::git::render_provider_menu(menu, model, window, cx),
                    model,
                ),
                move |_, cx| {
                    let _ = entity.update(cx, AppModel::dismiss_overlay);
                },
            )
        }
        Some(Overlay::Git(picker)) => {
            let content = marked(picker.picker.clone().into_any_element(), model);
            let model = cx.entity().downgrade();
            let dismiss = move |_: &mut Window, cx: &mut gpui::App| {
                let _ = model.update(cx, AppModel::dismiss_overlay);
            };
            codesh_ui::popover::anchored_popover_above(picker.anchor.clone(), content, dismiss)
        }
        Some(Overlay::PullRequest(popover)) => {
            let model_entity = cx.entity().downgrade();
            codesh_ui::popover::anchored_popover_above(
                model.git.pull_request_anchor.clone(),
                marked(super::git::render_pr(popover, model, cx), model),
                move |_, cx| {
                    let _ = model_entity.update(cx, AppModel::dismiss_overlay);
                },
            )
        }
        Some(Overlay::Menu(menu)) if menu.over_remote_servers => div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(remote_servers(
                super::remote_servers::list(model, window, cx).into_any_element(),
                model,
                cx,
            ))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|model, _, _, cx| {
                            model.dismiss_overlay(cx);
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|model, _, _, cx| {
                            model.dismiss_overlay(cx);
                            cx.stop_propagation();
                        }),
                    ),
            )
            .child(menu::render(menu, model, window, cx))
            .into_any_element(),
        Some(Overlay::Menu(menu)) => menu::render(menu, model, window, cx),
        Some(Overlay::ProjectEditor(editor)) => {
            super::project_editor::render(editor, model, window, cx)
        }
        Some(Overlay::Palette { palette, .. }) => palette.clone().into_any_element(),
    };
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|model, _, _, cx| {
                        model.dismiss_overlay(cx);
                        cx.stop_propagation();
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|model, _, _, cx| {
                        model.dismiss_overlay(cx);
                        cx.stop_propagation();
                    }),
                ),
        )
        .child(content)
        .into_any_element()
}
