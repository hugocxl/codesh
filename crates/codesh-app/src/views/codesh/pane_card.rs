use codesh_app_core::{Direction, PaneContent, PaneId};
use codesh_ui::{components::IconGlyph, icon::Icon};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, px,
};

use super::style;
use crate::model::AppModel;

#[derive(Clone, Copy)]
enum Action {
    Right,
    Down,
    Zoom,
    Close,
}

#[allow(
    clippy::too_many_lines,
    reason = "Shared pane chrome and its target-specific actions"
)]
pub(crate) fn render(
    id: PaneId,
    body: AnyElement,
    model: &AppModel,
    cx: &Context<AppModel>,
) -> AnyElement {
    let pane = model
        .state
        .current_project()
        .tabs
        .iter()
        .flat_map(|tab| &tab.panes)
        .find(|pane| pane.id == id);
    let (title, icon) = pane.map_or_else(
        || ("Terminal".to_owned(), Icon::Terminal),
        |pane| {
            (
                crate::views::native_panels::pane_title(id, pane.title.clone(), model),
                match &pane.content {
                    PaneContent::Terminal { .. } => Icon::Terminal,
                    PaneContent::Browser(_) => Icon::Globe,
                    PaneContent::Servers => Icon::Network,
                    PaneContent::Settings => Icon::Settings,
                    PaneContent::Editor { .. } => Icon::File,
                    PaneContent::Diff { .. } => Icon::GitBranch,
                },
            )
        },
    );
    let (title, suffix) = title
        .rsplit_once(' ')
        .filter(|(_, tail)| tail.starts_with('~') || tail.starts_with('/'))
        .unwrap_or((title.as_str(), ""));
    let zoomed = model
        .state
        .current_project()
        .tabs
        .iter()
        .any(|tab| tab.zoomed == Some(id));
    let m = model.metrics;
    let theme = &model.theme;
    let leading = leading_toggle(id, model, cx);
    let glyph = if model.state.browser(id).is_some() {
        model.browser_glyph(id, m.scaled(16.0), theme.fg_muted, cx)
    } else {
        IconGlyph::new(icon, m.scaled(16.0), theme.fg_muted).into_any_element()
    };
    let mut controls = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(m.scaled(4.0))
        .when(model.active_pane() != Some(id), |controls| {
            controls
                .invisible()
                .group_hover("pane-card", Styled::visible)
        });
    for (name, icon, tooltip, action) in [
        ("right", Icon::Columns, "Split Right", Action::Right),
        ("down", Icon::Rows, "Split Down", Action::Down),
        (
            "zoom",
            if zoomed {
                Icon::Restore
            } else {
                Icon::Maximize
            },
            "Zoom / Restore",
            Action::Zoom,
        ),
        ("close", Icon::X, "Close Pane", Action::Close),
    ] {
        controls = controls.child(
            crate::views::titlebar::icon_button(
                SharedString::from(format!("pane-{id}-{name}")),
                icon,
                tooltip,
                model,
            )
            .debug_selector(move || format!("pane-{id}-{name}"))
            .mr(px(0.0))
            .on_click(cx.listener(move |model, _, window, cx| {
                cx.stop_propagation();
                model.focus_pane(id, cx);
                if model.active_pane() == Some(id) {
                    match action {
                        Action::Right => model.split_pane(Direction::Right, cx),
                        Action::Down => model.split_pane(Direction::Down, cx),
                        Action::Zoom => model.toggle_zoom_pane(cx),
                        Action::Close => model.close_pane(id, cx),
                    }
                }
                model.focus_active(window, cx);
            })),
        );
    }
    div()
        .group("pane-card")
        .debug_selector(move || format!("pane-card-{id}"))
        .flex()
        .flex_col()
        .size_full()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .rounded(m.scaled(style::RADIUS))
        .border_1()
        .border_color(style::border(theme))
        .bg(style::card(theme))
        .overflow_hidden()
        .child(
            div()
                .id(SharedString::from(format!("pane-header-{id}")))
                .debug_selector(move || format!("pane-header-{id}"))
                .flex()
                .flex_none()
                .items_center()
                .h(px(style::HEADER))
                .px(style::spacing())
                .gap(m.scaled(8.0))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |model, _, window, cx| {
                        cx.stop_propagation();
                        model.focus_pane(id, cx);
                        model.focus_active(window, cx);
                    }),
                )
                .children(leading)
                .child(glyph)
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .gap(m.scaled(4.0))
                        .text_size(m.scaled(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.fg)
                        .child(div().truncate().child(title.to_owned()))
                        .child(div().text_color(theme.fg_muted).child(suffix.to_owned())),
                )
                .child(controls),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .rounded(m.scaled(style::RADIUS))
                .p(style::spacing())
                .overflow_hidden()
                .child(body),
        )
        .into_any_element()
}

/// With the sidebar hidden, its toggle leads the top-left pane's header, after
/// room for the window buttons when they are shown.
fn leading_toggle(id: PaneId, model: &AppModel, cx: &Context<AppModel>) -> Option<AnyElement> {
    if !style::HIDE_COLLAPSED_SIDEBAR || model.appearance.sidebar_expanded {
        return None;
    }
    let tab = model.state.current_project().tabs.first()?;
    let top_left = tab.zoomed.map_or_else(
        || {
            tab.layout
                .rects()
                .into_iter()
                .find(|(_, rect)| rect[0] < 1e-4 && rect[1] < 1e-4)
                .map(|(pane, _)| pane)
        },
        Some,
    );
    (top_left == Some(id)).then(|| sidebar_toggle(model, cx))
}

/// The sidebar toggle as shown inside a card, past the window buttons.
pub(crate) fn sidebar_toggle(model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    // The card and its header are each inset by the gutter; the traffic
    // lights end `window_buttons` from the window edge.
    let reserve = model.window_buttons - 2.0 * style::GUTTER;
    div()
        .flex_none()
        .flex()
        .items_center()
        .when(reserve > 0.0, |toggle| toggle.pl(px(reserve)))
        .child(crate::views::titlebar::sidebar_toggle(model, cx))
        .into_any_element()
}
