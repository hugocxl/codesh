use gpui::prelude::FluentBuilder;
use gpui::{App, ClickEvent, Div, ElementId, InteractiveElement, Stateful, Styled, Window};

use crate::components::ButtonInteraction;
use crate::controls::Style;

pub fn item(
    style: Style,
    id: impl Into<ElementId>,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let Style { theme, metrics } = style;
    gpui::div()
        .id(id)
        .button_interaction(on_click)
        .flex()
        .items_center()
        .gap(metrics.spacing6())
        .px(metrics.spacing5())
        .h(metrics.spacing10())
        .rounded(metrics.scaled(5.0))
        .cursor_pointer()
        .text_size(metrics.font_headline())
        .text_color(if selected { theme.fg } else { theme.fg_muted })
        .when(selected, |row| row.bg(theme.surface))
        .hover(|row| row.bg(theme.hover))
        .focus(|row| row.bg(theme.accent_soft))
}

pub fn subitem(
    style: Style,
    id: impl Into<ElementId>,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let Style { theme, metrics } = style;
    item(style, id, false, on_click)
        .h_auto()
        .pl(metrics.scaled(34.0))
        .py(metrics.scaled(7.0))
        .rounded(metrics.radius_sm())
        .text_size(metrics.font_body())
        .when(selected, |row| {
            row.bg(theme.accent_soft).text_color(theme.accent)
        })
}
