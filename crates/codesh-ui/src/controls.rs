use crate::components::{ButtonInteraction, SymbolGlyph};
use crate::picker::Picker;
use crate::text_input::TextInput;
use crate::theme::{Metrics, Theme};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, Bounds, ClickEvent, Corner, Entity, Focusable, FontWeight, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels, Point, SharedString, Styled,
    Window, anchored, canvas, deferred, div, point, px,
};
use std::cell::Cell;
use std::rc::Rc;

pub const CONTROL_WIDTH: f32 = 210.0;
pub const SLIDER_WIDTH: f32 = 220.0;

#[derive(Clone, Copy, Debug)]
pub struct Style<'a> {
    pub theme: &'a Theme,
    pub metrics: &'a Metrics,
}

#[derive(Clone, Debug)]
pub struct Choice {
    pub value: String,
    pub label: String,
    pub enabled: bool,
}

impl Choice {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            enabled: true,
        }
    }
}

pub fn toggle(
    style: Style,
    id: &str,
    value: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Style { theme, metrics } = style;
    div()
        .id(SharedString::from(format!("settings-toggle-{id}")))
        .debug_selector({
            let selector = format!("settings-toggle-{id}");
            move || selector.clone()
        })
        .flex()
        .flex_row()
        .items_center()
        .flex_none()
        .w(metrics.scaled(38.0))
        .h(metrics.scaled(22.0))
        .rounded_full()
        .cursor_pointer()
        .border_1()
        .border_color(gpui::transparent_black())
        .child(SwitchVisual {
            value,
            accent: theme.accent,
            off: theme.fg_alpha(0.18),
            scale: f32::from(metrics.scaled(1.0)),
        })
        .focus(|focus| focus.border_color(theme.accent))
        .button_interaction(on_click)
        .into_any_element()
}

// State lives with the rendered element, so opening a page does not animate
// every switch. Only a value transition schedules frames, for at most 150ms.
struct SwitchVisual {
    value: bool,
    accent: gpui::Hsla,
    off: gpui::Hsla,
    scale: f32,
}

struct SwitchMotion {
    target: bool,
    from: f32,
    position: f32,
    started: std::time::Instant,
}

impl IntoElement for SwitchVisual {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl gpui::Element for SwitchVisual {
    type RequestLayoutState = ();
    type PrepaintState = f32;

    fn id(&self) -> Option<gpui::ElementId> {
        Some("switch-visual".into())
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, ()) {
        let style = gpui::Style {
            size: gpui::size(gpui::relative(1.0).into(), gpui::relative(1.0).into()),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        id: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _: Bounds<Pixels>,
        (): &mut (),
        window: &mut Window,
        _: &mut App,
    ) -> f32 {
        let Some(id) = id else {
            return if self.value { 1.0 } else { 0.0 };
        };
        window.with_element_state(id, |state: Option<SwitchMotion>, window| {
            let now = std::time::Instant::now();
            let target = if self.value { 1.0 } else { 0.0 };
            let mut state = state.unwrap_or(SwitchMotion {
                target: self.value,
                from: target,
                position: target,
                started: now,
            });
            if state.target != self.value {
                state.target = self.value;
                state.from = state.position;
                state.started = now;
            }
            let t = (now.duration_since(state.started).as_secs_f32() / 0.15).min(1.0);
            let eased = t * t * (3.0 - 2.0 * t);
            state.position = state.from + (target - state.from) * eased;
            if t < 1.0 && (state.position - target).abs() > f32::EPSILON {
                window.request_animation_frame();
            }
            (state.position, state)
        })
    }

    fn paint(
        &mut self,
        _: Option<&gpui::GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        position: &mut f32,
        window: &mut Window,
        _: &mut App,
    ) {
        let off: gpui::Rgba = self.off.into();
        let on: gpui::Rgba = self.accent.into();
        let mix = |a: f32, b: f32| a + (b - a) * *position;
        let color = gpui::Rgba {
            r: mix(off.r, on.r),
            g: mix(off.g, on.g),
            b: mix(off.b, on.b),
            a: mix(off.a, on.a),
        };
        window.paint_quad(gpui::fill(bounds, color).corner_radii(bounds.size.height / 2.0));
        let diameter = px(18.0 * self.scale);
        let inset = (bounds.size.height - diameter) / 2.0;
        let origin = point(
            bounds.left() + inset + (bounds.size.width - diameter - inset * 2.0) * *position,
            bounds.top() + inset,
        );
        let knob = Bounds::new(origin, gpui::size(diameter, diameter));
        let shadow = Bounds::new(origin + point(px(0.0), px(self.scale)), knob.size);
        window.paint_quad(gpui::fill(shadow, gpui::rgba(0x0000_0033)).corner_radii(diameter / 2.0));
        window.paint_quad(gpui::fill(knob, gpui::rgb(0x00ff_ffff)).corner_radii(diameter / 2.0));
    }
}

/// A box ticked when `checked`. `mixed` shows a dash instead, for a box that
/// stands for several where only some are ticked.
pub fn checkbox(
    style: Style,
    id: &str,
    checked: bool,
    mixed: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Style { theme, metrics } = style;
    let on = checked || mixed;
    div()
        .id(SharedString::from(format!("checkbox-{id}")))
        .debug_selector({
            let selector = format!("checkbox-{id}");
            move || selector.clone()
        })
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(metrics.scaled(14.0))
        .rounded(metrics.scaled(3.0))
        .cursor_pointer()
        .bg(if on { theme.accent } else { theme.surface })
        .border_1()
        .border_color(if on {
            theme.accent
        } else {
            theme.border_solid()
        })
        .when(checked, |checkbox| {
            checkbox.child(crate::components::IconGlyph::new(
                crate::icon::Icon::Check,
                metrics.scaled(12.0),
                gpui::rgb(0x00ff_ffff).into(),
            ))
        })
        .when(mixed && !checked, |checkbox| {
            checkbox.child(
                div()
                    .w(metrics.scaled(8.0))
                    .h(metrics.scaled(2.0))
                    .rounded_full()
                    .bg(gpui::rgb(0x00ff_ffff)),
            )
        })
        .focus(|focus| focus.border_color(theme.accent))
        // Like a macOS checkbox, a click ticks it without focusing it; Tab
        // still does.
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .button_interaction(on_click)
        .into_any_element()
}

pub fn button(
    style: Style,
    id: &str,
    label: &str,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let Style { theme, metrics } = style;
    div()
        .id(SharedString::from(format!("settings-button-{id}")))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(metrics.scaled(24.0))
        .px(metrics.spacing5())
        .rounded(metrics.scaled(6.0))
        .bg(theme.fg_alpha(0.16))
        .border_1()
        .border_color(theme.border)
        .text_size(metrics.scaled(13.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.fg)
        .when(!enabled, |element| element.opacity(0.4))
        .when(enabled, |element| {
            element
                .cursor_pointer()
                .hover(|hover| hover.bg(theme.hover))
                .focus(|focus| focus.border_color(theme.accent))
                .button_interaction(on_click)
        })
        .child(SharedString::from(label.to_owned()))
}

pub fn picker(
    style: Style,
    id: &str,
    choices: &[Choice],
    selected: &str,
    popover: Option<Entity<Picker>>,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let metrics = style.metrics;
    let label = choices
        .iter()
        .find(|choice| choice.value == selected)
        .map(|choice| choice.label.clone())
        .unwrap_or_default();
    let field = picker_trigger(
        style,
        id,
        &label,
        Some(CONTROL_WIDTH),
        popover.is_some(),
        on_toggle,
    );

    let Some(popover) = popover else {
        return div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .child(field)
            .into_any_element();
    };

    div()
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .child(field)
        .child(
            deferred(
                anchored()
                    .anchor(Corner::TopLeft)
                    .offset(point(
                        px(0.0),
                        metrics.control_medium() + metrics.spacing1(),
                    ))
                    .snap_to_window_with_margin(px(8.0))
                    .child(popover),
            )
            .with_priority(1),
        )
        .into_any_element()
}

pub fn picker_trigger(
    style: Style,
    id: &str,
    label: &str,
    width: Option<f32>,
    open: bool,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Style { theme, metrics } = style;
    div()
        .id(SharedString::from(format!("settings-picker-{id}")))
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .gap(metrics.spacing3())
        .map(|field| {
            if let Some(width) = width {
                field.w(metrics.scaled(width))
            } else {
                field.w_full()
            }
        })
        .h(metrics.scaled(24.0))
        .px(metrics.scaled(2.0))
        .rounded(metrics.scaled(6.0))
        .cursor_pointer()
        .when(open, |field| field.bg(theme.hover))
        .hover(|hover| hover.bg(theme.hover))
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .text_size(metrics.scaled(13.0))
                .text_color(theme.fg)
                .child(SharedString::from(label.to_owned())),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(metrics.scaled(20.0))
                .rounded(metrics.scaled(5.0))
                .bg(theme.surface)
                .child(crate::components::IconGlyph::new(
                    crate::icon::Icon::ChevronsUpDown,
                    metrics.scaled(12.0),
                    theme.fg,
                )),
        )
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .focus(|focus| focus.bg(theme.accent_soft))
        .button_interaction(on_toggle)
        .into_any_element()
}

pub fn segmented(
    style: Style,
    id: &str,
    choices: &[Choice],
    selected: &str,
    on_select: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Style { theme, metrics } = style;
    let handler = Rc::new(on_select);
    let mut group = div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .h(metrics.scaled(24.0))
        .gap(px(1.0))
        .p(px(1.0))
        .rounded(metrics.scaled(7.0))
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border);

    for choice in choices {
        let is_selected = choice.value == selected;
        let handler = handler.clone();
        let value = SharedString::from(choice.value.clone());
        group = group.child(
            div()
                .id(SharedString::from(format!(
                    "settings-segment-{id}-{}",
                    choice.value
                )))
                .debug_selector({
                    let selector = format!("settings-segment-{id}-{}", choice.value);
                    move || selector.clone()
                })
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .h(metrics.scaled(20.0))
                .px(metrics.spacing4())
                .rounded(metrics.scaled(6.0))
                .text_size(metrics.scaled(13.0))
                .font_weight(if is_selected {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(theme.fg)
                .when(is_selected, |element| {
                    element.bg(theme.fg_alpha(0.18)).shadow_sm()
                })
                .when(!choice.enabled, |element| element.opacity(0.4))
                .when(choice.enabled && !is_selected, |element| {
                    element
                        .cursor_pointer()
                        .hover(|hover| hover.bg(theme.hover))
                })
                .when(choice.enabled, |element| {
                    element
                        .focus(|focus| {
                            focus.bg(if is_selected {
                                theme.fg_alpha(0.18)
                            } else {
                                theme.hover
                            })
                        })
                        .button_interaction(move |_, window, cx| handler(&value, window, cx))
                })
                .child(SharedString::from(choice.label.clone())),
        );
    }

    group.into_any_element()
}

#[derive(Debug)]
pub struct Grab {
    pub bounds: Bounds<Pixels>,
    pub position: Point<Pixels>,
}

pub fn fraction_at(bounds: Bounds<Pixels>, position: Point<Pixels>) -> f32 {
    let width = f32::from(bounds.size.width);
    if width <= 0.0 {
        return 0.0;
    }
    (f32::from(position.x - bounds.origin.x) / width).clamp(0.0, 1.0)
}

pub fn slider(
    style: Style,
    id: &str,
    value: f32,
    range: (f32, f32),
    on_grab: impl Fn(&Grab, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Style { theme, metrics } = style;
    let span = (range.1 - range.0).max(f32::EPSILON);
    let fraction = ((value - range.0) / span).clamp(0.0, 1.0);
    let track: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
    let recorder = track.clone();
    let width = metrics.scaled(SLIDER_WIDTH);
    let knob = metrics.scaled(18.0);
    let rail = metrics.scaled(4.0);

    div()
        .id(SharedString::from(format!("settings-slider-{id}")))
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .w(width)
        .h(metrics.control_medium())
        .cursor_pointer()
        .child(
            canvas(
                move |bounds, _, _| recorder.set(Some(bounds)),
                |_, (): (), _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .w_full()
                .h(rail)
                .rounded_full()
                .bg(theme.border),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .w(width * fraction)
                .h(rail)
                .rounded_full()
                .bg(theme.accent),
        )
        .child(
            div()
                .absolute()
                .left((width - knob) * fraction)
                .size(knob)
                .rounded_full()
                .bg(gpui::rgb(0x00ff_ffff))
                .shadow_sm()
                .border_1()
                .border_color(theme.border),
        )
        .on_mouse_down(
            MouseButton::Left,
            move |event: &MouseDownEvent, window, cx| {
                let Some(bounds) = track.get() else {
                    return;
                };
                on_grab(
                    &Grab {
                        bounds,
                        position: event.position,
                    },
                    window,
                    cx,
                );
            },
        )
        .into_any_element()
}

pub fn text_area(
    style: Style,
    id: &str,
    input: &Entity<TextInput>,
    content_height: Option<f32>,
) -> AnyElement {
    let Style { theme, metrics } = style;
    div()
        .id(SharedString::from(format!("settings-area-{id}")))
        .relative()
        .flex()
        .flex_col()
        .min_h(px(0.0))
        .p(metrics.spacing4())
        .rounded(metrics.scaled(6.0))
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .map(|element| match content_height {
            Some(height) => element
                .flex_none()
                .h(metrics.scaled(height) + metrics.spacing4() * 2.0 + px(2.0)),
            None => element.flex_grow(),
        })
        .child(input.clone())
        .child(input_focus_ring(style, input, 6.0))
        .into_any_element()
}

fn input_focus_ring(style: Style<'_>, input: &Entity<TextInput>, radius: f32) -> impl IntoElement {
    let input = input.clone();
    let accent = style.theme.accent;
    let radius = style.metrics.scaled(radius);
    let thickness = style.metrics.scaled(3.0);
    canvas(
        |_, _, _| (),
        move |bounds, (), window, cx| {
            if input.focus_handle(cx).contains_focused(window, cx) {
                let mut soft = accent;
                soft.a = 0.35;
                window.paint_quad(
                    gpui::outline(
                        bounds.inset(thickness / 2.0),
                        soft,
                        gpui::BorderStyle::Solid,
                    )
                    .corner_radii(radius)
                    .border_widths(thickness),
                );
                window.paint_quad(
                    gpui::outline(bounds.inset(px(0.5)), accent, gpui::BorderStyle::Solid)
                        .corner_radii(radius),
                );
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub fn text_field(
    style: Style,
    id: &str,
    input: &Entity<TextInput>,
    width: Option<f32>,
) -> AnyElement {
    let Style { theme, metrics } = style;
    div()
        .id(SharedString::from(format!("settings-field-{id}")))
        .debug_selector({
            let selector = format!("settings-field-{id}");
            move || selector.clone()
        })
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .h(metrics.scaled(24.0))
        .min_h(metrics.scaled(24.0))
        .max_h(metrics.scaled(24.0))
        .px(metrics.spacing4())
        .rounded(metrics.scaled(6.0))
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .map(|element| match width {
            Some(width) => element.flex_none().w(metrics.scaled(width)),
            None => element.flex_grow().min_w(px(0.0)),
        })
        .on_mouse_down_out(|_, window, _| window.blur())
        .child(crate::text_input::growing_input(input))
        .child(input_focus_ring(style, input, 6.0))
        .into_any_element()
}

pub fn search_field(
    style: Style,
    id: impl Into<gpui::ElementId>,
    input: &Entity<TextInput>,
) -> gpui::Stateful<gpui::Div> {
    let Style { theme, metrics } = style;
    div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(metrics.spacing4())
        .h(metrics.scaled(28.0))
        .px(metrics.scaled(9.0))
        .bg(theme.surface)
        .border_1()
        .border_color(gpui::transparent_black())
        .rounded(metrics.scaled(8.0))
        .child(SymbolGlyph::new(
            "magnifyingglass",
            metrics.icon_md(),
            theme.fg_muted,
        ))
        .child(crate::text_input::growing_input(input))
        .child(input_focus_ring(style, input, 8.0))
}
