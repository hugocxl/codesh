use codesh_ui::components::{IconGlyph, Tooltip};
use codesh_ui::icon::Icon;
use codesh_ui::tr;
use gpui::{
    AnyElement, App, AppContext, Context, Div, ElementId, InteractiveElement, IntoElement,
    MouseButton, ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled, Window,
    div,
};

use crate::model::AppModel;

#[cfg(target_os = "linux")]
pub(crate) mod frame;
#[cfg(target_os = "linux")]
pub(crate) mod linux;

pub(crate) fn content_size(window: &Window) -> gpui::Size<gpui::Pixels> {
    #[cfg(target_os = "linux")]
    return frame::bounds(window).size;
    #[cfg(not(target_os = "linux"))]
    window.viewport_size()
}

pub(crate) fn overlay(element: impl IntoElement, window: &Window) -> AnyElement {
    #[cfg(target_os = "linux")]
    return frame::overlay(element, window);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = window;
        element.into_any_element()
    }
}

pub(super) fn top_corners<T: Styled>(element: T, window: &Window, left: bool, right: bool) -> T {
    #[cfg(target_os = "linux")]
    return frame::top_corners(element, window, left, right);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (window, left, right);
        element
    }
}

#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = codesh, no_json)]
pub(crate) struct BeginWindowMove;

pub(crate) fn begin_window_move(
    model: &mut AppModel,
    _: &BeginWindowMove,
    window: &mut Window,
    _: &mut Context<AppModel>,
) {
    #[cfg(target_os = "macos")]
    let _ = &window;
    if model.overlay.is_some() || model.close_prompt.is_some() {
        return;
    }
    #[cfg(target_os = "macos")]
    if let Some(drag) = &model.window_drag {
        drag.begin();
    }
    #[cfg(not(target_os = "macos"))]
    window.start_window_move();
}

pub(crate) fn background(id: &'static str) -> Stateful<Div> {
    let background = div()
        .id(id)
        .debug_selector(move || id.into())
        .on_mouse_down(MouseButton::Left, |event, window, cx| {
            cx.stop_propagation();
            if event.click_count == 1 {
                window.dispatch_action(Box::new(BeginWindowMove), cx);
            }
        })
        .on_click(background_click);
    #[cfg(target_os = "linux")]
    let background = background.on_mouse_down(MouseButton::Right, |event, window, cx| {
        cx.stop_propagation();
        if window.window_controls().window_menu {
            window.show_window_menu(event.position);
        }
    });
    background
}

fn background_click(event: &gpui::ClickEvent, window: &mut Window, cx: &mut App) {
    if event.click_count() == 2 {
        cx.stop_propagation();
        window.dispatch_action(Box::new(super::workspace::Zoom), cx);
    }
}

pub(crate) fn action_button(
    id: impl Into<ElementId>,
    tooltip: impl Into<SharedString>,
    model: &AppModel,
) -> Stateful<Div> {
    let m = model.metrics;
    let theme = model.theme.clone();
    let tooltip = tooltip.into();
    div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(m.control_medium())
        .mr(m.spacing2())
        .rounded(m.radius_sm())
        .cursor_pointer()
        .hover(|style| style.bg(theme.hover))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .tooltip(move |_, cx| {
            cx.new(|_| {
                Tooltip::new(
                    tooltip.clone(),
                    theme.raised(),
                    theme.fg,
                    theme.border,
                    theme.bg,
                )
            })
            .into()
        })
}

pub(super) fn icon_button(
    id: impl Into<ElementId>,
    icon: Icon,
    tooltip: impl Into<SharedString>,
    model: &AppModel,
) -> Stateful<Div> {
    action_button(id, tooltip, model).child(chrome_glyph(icon, model.theme.fg_muted, model))
}

/// The glyph every chrome button draws, so icons measure the same everywhere.
pub(crate) fn chrome_glyph(icon: Icon, color: gpui::Hsla, model: &AppModel) -> IconGlyph {
    IconGlyph::new(icon, model.metrics.icon_md(), color).fit()
}

pub(crate) fn navigation(
    model: &AppModel,
    sidebar_width: f32,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    super::codesh::sidebar_chrome::top(model, sidebar_width, window, cx)
}

pub(crate) fn sidebar_toggle(model: &AppModel, cx: &Context<AppModel>) -> Stateful<Div> {
    icon_button(
        "toggle-sidebar",
        Icon::PanelLeft,
        tr!("Toggle Sidebar"),
        model,
    )
    .debug_selector(|| "sidebar-toggle".into())
    .on_click(cx.listener(|model, _, window, cx| {
        cx.stop_propagation();
        model.toggle_sidebar(window, cx);
    }))
}

pub(super) fn navigation_width(model: &AppModel, _: &Window) -> f32 {
    if cfg!(target_os = "linux") {
        return f32::from(model.metrics.sidebar_collapsed_width() + model.metrics.scaled(52.0));
    }
    let arrows = model.metrics.navigation_arrows_width();
    f32::from(if cfg!(target_os = "macos") {
        model.metrics.traffic_light_width() + arrows
    } else {
        arrows
    })
}
