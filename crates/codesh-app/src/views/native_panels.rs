//! The native Files and Source Control side panels, their sidebar toggles,
//! and the bodies of editor and diff panes.

use codesh_app_core::settings::SidePanel;
use codesh_app_core::{PaneContent, PaneId};
use codesh_core::shortcuts::ShortcutId;
use codesh_ui::icon::Icon;
use codesh_ui::panel::{
    PanelFrame, PanelId, PanelPosition, PanelSizeBounds, PanelSizing, PanelStyle,
};
use codesh_ui::tr;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};

use super::codesh::style;
use crate::model::AppModel;
use crate::model::native::{EditorContent, FILES_PANEL, GIT_PANEL};

/// Wraps the workspace `content` with the pinned Files or Source Control panel.
pub(crate) fn panel_content(
    content: AnyElement,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = match model.appearance.side_panel {
        SidePanel::Files => FILES_PANEL,
        SidePanel::SourceControl => GIT_PANEL,
        SidePanel::None => return content,
    };
    let Some(placement) = model.panels.placement(&PanelId::new(id)).cloned() else {
        return content;
    };
    let m = model.metrics;
    let scale = f32::from(m.scaled(1.0));
    let viewport = window.viewport_size();
    let sizing = PanelSizing::new(
        &placement,
        f32::from(m.scaled(model.appearance.side_panel_width.unwrap_or(300.0))),
        PanelSizeBounds::new(
            f32::from(m.scaled(200.0)),
            (f32::from(viewport.width) - model.sidebar_width() - 100.0)
                .min(f32::from(m.scaled(800.0))),
        ),
        model.native.resize.clone(),
    );
    let owner = cx.weak_entity();
    let frame = PanelFrame::new(
        placement,
        sizing,
        div(),
        body(model, cx),
        move |dimension, _, cx| {
            let _ = owner.update(cx, |model, cx| {
                let size = Some(dimension / scale);
                model.appearance.side_panel_width = size;
                if !model.native.resize.is_active()
                    && size != model.settings.appearance.side_panel_width
                {
                    model.save_appearance(cx);
                }
                cx.notify();
            });
        },
        PanelStyle::new(model.theme.clone(), m)
            .sidebar(px(0.0))
            .background(gpui::transparent_black()),
    )
    .grip(model.webview_grip(None, PanelPosition::Right.resize_cursor()));
    div()
        .debug_selector(|| "native-side-panel".into())
        .relative()
        .flex()
        .size_full()
        .min_w_0()
        .min_h_0()
        .child(
            div()
                .relative()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_hidden()
                .child(content),
        )
        .child(frame)
        .into_any_element()
}

fn body(model: &AppModel, _cx: &mut Context<AppModel>) -> AnyElement {
    let project = model.state.current_project().id;
    let view = match model.appearance.side_panel {
        SidePanel::Files => model
            .native
            .files
            .get(&project)
            .map(|(panel, _)| panel.clone().into_any_element()),
        SidePanel::SourceControl => model
            .native
            .git
            .get(&project)
            .map(|(panel, _)| panel.clone().into_any_element()),
        SidePanel::None => None,
    };
    let theme = &model.theme;
    // The pane grid's inset already separates the panel on its left.
    div()
        .flex()
        .flex_col()
        .flex_1()
        .size_full()
        .min_h_0()
        .min_w_0()
        .pt(style::spacing())
        .pr(style::spacing())
        .pb(style::spacing())
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_hidden()
                .rounded(model.metrics.scaled(style::RADIUS))
                .border_1()
                .border_color(style::border(theme))
                .bg(style::panel(theme))
                .children(view),
        )
        .into_any_element()
}

/// The sidebar's Files and Source Control toggles.
pub(crate) fn sidebar_buttons(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let button = |id: &'static str,
                  panel: SidePanel,
                  icon: Icon,
                  tooltip: SharedString,
                  shortcut: ShortcutId| {
        let active = model.appearance.side_panel == panel;
        let theme = &model.theme;
        crate::views::titlebar::action_button(id, tooltip, Some(shortcut), model)
            .debug_selector(move || id.into())
            .when(active, |button| button.bg(theme.surface))
            .on_click(cx.listener(move |model, _, window, cx| {
                cx.stop_propagation();
                model.toggle_side_panel(panel, window, cx);
            }))
            .child(crate::views::titlebar::chrome_glyph(
                icon,
                if active { theme.fg } else { theme.fg_muted },
                model,
            ))
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .child(
            crate::views::titlebar::icon_button(
                "sidebar-new-terminal",
                Icon::Plus,
                tr!("New Terminal"),
                Some(ShortcutId::NewTab),
                model,
            )
            .debug_selector(|| "sidebar-new-terminal".into())
            .on_click(cx.listener(|model, _, window, cx| {
                cx.stop_propagation();
                model.new_tab(cx);
                model.focus_active(window, cx);
            })),
        )
        .child(button(
            "sidebar-files",
            SidePanel::Files,
            Icon::Folder,
            tr!("Files"),
            ShortcutId::ToggleFiles,
        ))
        .child(button(
            "sidebar-source-control",
            SidePanel::SourceControl,
            Icon::GitBranch,
            tr!("Source Control"),
            ShortcutId::ToggleSourceControl,
        ))
        .into_any_element()
}

fn message(text: impl Into<SharedString>, model: &AppModel) -> AnyElement {
    div()
        .flex()
        .size_full()
        .items_center()
        .justify_center()
        .p(style::spacing())
        .text_center()
        .text_color(model.theme.fg_muted)
        .text_size(model.metrics.font_body())
        .child({
            let text = text.into();
            div()
                .child(text.clone())
                .when(text == tr!("Not a Git repository"), |view| {
                    view.child(div().text_size(model.metrics.font_footnote()).child(tr!(
                        "Open a folder containing a Git repository to view changes."
                    )))
                })
        })
        .into_any_element()
}

fn banner(id: PaneId, model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let owner = cx.weak_entity();
    let action = move |label: SharedString, reload: bool| {
        let owner = owner.clone();
        div()
            .id(SharedString::from(format!("disk-change-{reload}-{id}")))
            .debug_selector(move || format!("disk-change-{reload}"))
            .px(m.spacing2())
            .rounded(m.radius_sm())
            .cursor_pointer()
            .text_color(theme.accent)
            .hover(|style| style.bg(theme.hover))
            .on_click(move |_, _, cx| {
                let _ = owner.update(cx, |model, cx| model.resolve_disk_change(id, reload, cx));
            })
            .child(label)
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(style::spacing())
        .px(style::spacing())
        .py(m.spacing1())
        .bg(theme.surface)
        .text_size(m.font_footnote())
        .text_color(theme.fg)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(tr!("Changed on disk")),
        )
        .child(action(tr!("Reload"), true))
        .child(action(tr!("Keep mine"), false))
        .into_any_element()
}

/// An editor or diff pane's body.
pub(crate) fn pane_body(
    id: PaneId,
    content: &PaneContent,
    model: &AppModel,
    cx: &Context<AppModel>,
) -> AnyElement {
    let inner = match content {
        PaneContent::Editor { .. } => match model.editors.get(&id) {
            Some(editor) => match &editor.content {
                EditorContent::Ready { view, .. } => div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .min_h_0()
                    .when(editor.disk.is_some(), |body| {
                        body.child(banner(id, model, cx))
                    })
                    .child(div().flex_1().min_h_0().child(view.clone()))
                    .into_any_element(),
                EditorContent::Message(text) => message(text.clone(), model),
                EditorContent::Loading => message(tr!("Loading…"), model),
            },
            None => message(tr!("Loading…"), model),
        },
        PaneContent::Diff { .. } => match model.diffs.get(&id) {
            Some(diff) => match (&diff.view, &diff.error) {
                (Some(view), _) => view.clone().into_any_element(),
                (None, Some(error)) => message(error.clone(), model),
                (None, None) => message(tr!("Loading…"), model),
            },
            None => message(tr!("Loading…"), model),
        },
        _ => return div().into_any_element(),
    };
    div()
        .size_full()
        .bg(style::card(&model.theme))
        .id(SharedString::from(format!("native-pane-{id}")))
        .debug_selector(|| "native-pane".into())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |model, _, window, cx| {
                model.focus_pane(id, cx);
                model.focus_active(window, cx);
            }),
        )
        .child(inner)
        .into_any_element()
}

/// The pane's title, marked while an editor has unsaved changes.
pub(crate) fn pane_title(id: PaneId, title: String, model: &AppModel) -> String {
    if model.editor_dirty(id) {
        format!("• {title}")
    } else {
        title
    }
}
