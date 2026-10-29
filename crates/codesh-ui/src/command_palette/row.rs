//! Section headers, item rows, status messages and footer pieces.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, ClickEvent, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};

use super::model::{Item, Target};
use super::{keycaps, layout, tile};
use crate::components::IconGlyph;
use crate::fuzzy;
use crate::icon::Icon;
use crate::theme::{Metrics, Theme};

pub(super) fn section_header(title: SharedString, theme: &Theme, metrics: Metrics) -> AnyElement {
    div()
        .w_full()
        .h(metrics.scaled(layout::SECTION))
        .flex()
        .items_end()
        .px(metrics.scaled(layout::ROW_INSET * 2.0))
        .pb(metrics.spacing3())
        .text_size(metrics.font_footnote())
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.fg_muted)
        .child(title)
        .into_any_element()
}

#[allow(
    clippy::too_many_arguments,
    reason = "A row draws from its item, cached keycaps, state and appearance."
)]
pub(super) fn item<A>(
    index: usize,
    item: &Item<A>,
    keycaps: &[SharedString],
    selected: bool,
    theme: &Theme,
    metrics: Metrics,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let id = item.id.clone();
    let title = div()
        .min_w(px(0.0))
        .when(item.subtitle.is_some(), |title| title.max_w(relative(0.7)))
        .flex_none()
        .truncate()
        .text_size(metrics.font_headline())
        .font_weight(if item.current {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .child(fuzzy::highlighted(
            item.title.clone(),
            &item.highlights,
            theme.accent,
        ));
    let text = div()
        .min_w(px(0.0))
        .flex_1()
        .flex()
        .items_center()
        .gap(metrics.spacing4())
        .overflow_hidden()
        .child(title)
        .when_some(item.subtitle.clone(), |text, subtitle| {
            text.child(
                div()
                    .min_w(px(0.0))
                    .flex_1()
                    .truncate()
                    .text_size(metrics.font_body())
                    .text_color(theme.fg_muted)
                    .child(subtitle),
            )
        });
    let content = div()
        .id(("palette-row", index))
        .debug_selector(move || format!("palette-row-{id}"))
        .size_full()
        .flex()
        .items_center()
        .gap(metrics.spacing5())
        .px(metrics.spacing4())
        .rounded(metrics.scaled(layout::ROW_RADIUS))
        .text_color(if item.disabled {
            theme.fg_dim
        } else {
            theme.fg
        })
        .when(selected, |row| row.bg(theme.fg_alpha(0.08)))
        .when(!selected && !item.disabled, |row| {
            row.hover(|style| style.bg(theme.hover))
        })
        .when(item.disabled, |row| row.opacity(0.6))
        .when(!item.disabled, |row| {
            row.cursor_pointer().on_click(on_click)
        })
        .child(tile::render(&item.tile, theme, metrics))
        .child(text)
        .when(item.current, |row| {
            row.child(IconGlyph::new(Icon::Check, metrics.icon_sm(), theme.accent))
        })
        .when_some(item.badge.clone(), |row, badge| {
            row.child(pill(badge, theme, metrics))
        })
        .when(!keycaps.is_empty(), |row| {
            row.child(keycaps::keycaps(keycaps, theme, metrics))
        })
        .when_some(item.kind.clone(), |row, kind| {
            row.child(
                div()
                    .flex_none()
                    .text_size(metrics.font_body())
                    .text_color(theme.fg_muted)
                    .child(kind),
            )
        })
        .when(matches!(item.target, Target::OpenPage(_)), |row| {
            row.child(IconGlyph::new(
                Icon::ChevronRight,
                metrics.icon_sm(),
                theme.fg_muted,
            ))
        });
    div()
        .w_full()
        .h(metrics.scaled(layout::ROW))
        .px(metrics.scaled(layout::ROW_INSET))
        .child(content)
        .into_any_element()
}

fn pill(label: SharedString, theme: &Theme, metrics: Metrics) -> gpui::Div {
    div()
        .flex_none()
        .px(metrics.spacing3())
        .py(px(1.0))
        .rounded_full()
        .bg(theme.fg_alpha(0.08))
        .text_size(metrics.font_caption())
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.fg_muted)
        .child(label)
}

pub(super) fn status_message(message: SharedString, color: Hsla, metrics: Metrics) -> AnyElement {
    div()
        .flex_grow()
        .min_h(px(0.0))
        .flex()
        .items_center()
        .justify_center()
        .px(metrics.spacing7())
        .text_size(metrics.font_body())
        .text_color(color)
        .child(message)
        .into_any_element()
}

/// A footer button: a label followed by its keycaps.
pub(super) fn footer_button(
    id: &'static str,
    label: SharedString,
    caps: &[SharedString],
    enabled: bool,
    theme: &Theme,
    metrics: Metrics,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .flex()
        .flex_none()
        .items_center()
        .gap(metrics.spacing3())
        .h(metrics.control_large() - metrics.spacing2())
        .px(metrics.spacing4())
        .rounded(metrics.radius_md())
        .text_size(metrics.font_body())
        .font_weight(FontWeight::MEDIUM)
        .text_color(if enabled { theme.fg } else { theme.fg_dim })
        .when(enabled, |button| {
            button.cursor_pointer().hover(|style| style.bg(theme.hover))
        })
        .child(label)
        .child(keycaps::keycaps(caps, theme, metrics))
}

pub(super) fn footer_separator(theme: &Theme, metrics: Metrics) -> gpui::Div {
    div()
        .flex_none()
        .w(px(1.0))
        .h(metrics.scaled(16.0))
        .bg(theme.border)
}
