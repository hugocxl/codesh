use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Div, ParentElement, Styled, div, px, relative};

use crate::controls::Style;

pub fn row(
    style: Style,
    label: &str,
    description: Option<&str>,
    control: AnyElement,
    stacked: bool,
) -> Div {
    let Style { theme, metrics } = style;
    let text = div()
        .flex_1()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(metrics.scaled(5.0))
        .child(
            div()
                .text_size(metrics.scaled(13.0))
                .text_color(theme.fg)
                .child(label.to_owned()),
        )
        .when_some(description, |text, description| {
            text.child(
                div()
                    .text_size(metrics.font_emphasis())
                    .line_height(relative(1.55))
                    .text_color(theme.fg_muted)
                    .child(description.to_owned()),
            )
        });
    div()
        .flex()
        .min_w(px(0.0))
        .min_h(metrics.scaled(36.0))
        .py(metrics.scaled(10.0))
        .px(metrics.scaled(12.0))
        .gap(metrics.scaled(12.0))
        .map(|row| {
            if stacked {
                row.flex_col().gap(metrics.spacing6())
            } else {
                row.items_center()
            }
        })
        .child(text)
        .child(div().flex_none().min_w(px(0.0)).child(control))
}

pub fn section_heading(style: Style, title: &str) -> Div {
    let Style { theme, metrics } = style;
    div()
        .pt(metrics.scaled(24.0))
        .pb(metrics.scaled(8.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_size(metrics.scaled(13.0))
        .text_color(theme.fg)
        .child(title.to_owned())
}

pub fn note(style: Style, text: &str, error: bool) -> Div {
    let Style { theme, metrics } = style;
    div()
        .py(metrics.scaled(8.0))
        .px(metrics.scaled(12.0))
        .text_size(metrics.scaled(11.0))
        .text_color(if error { theme.danger } else { theme.fg_muted })
        .child(text.to_owned())
}
