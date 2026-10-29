use codesh_ui::components::{ButtonInteraction, IconGlyph, Tooltip};
use codesh_ui::icon::Icon;
use codesh_ui::{controls, navigation};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, list, px,
};

use super::{Category, SettingsEvent, SettingsView};
use crate::views::{codesh::style, titlebar};

impl Category {
    fn tile(self) -> AnyElement {
        let (icon, color) = match self {
            Self::General => (Icon::Settings, 0x0077_777c),
            Self::Appearance => (Icon::Palette, 0x0034_78f6),
            Self::Terminal => (Icon::Terminal, 0x0020_3b26),
            Self::Keyboard => (Icon::Keyboard, 0x0077_777c),
            Self::Commands => (Icon::Code, 0x00e9_8224),
            Self::Browser => (Icon::Globe, 0x0026_8dec),
            Self::Server => (Icon::Network, 0x0029_9fa3),
            Self::Mobile => (Icon::AppWindow, 0x0038_a455),
            Self::Ai => (Icon::Sparkles, 0x00a3_4de0),
            Self::Backup => (Icon::History, 0x0077_777c),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(20.0))
            .rounded(px(5.0))
            .bg(gpui::rgb(color))
            .child(IconGlyph::new(
                icon,
                px(14.0),
                gpui::rgb(if self == Self::Terminal {
                    0x0079_f58b
                } else {
                    0x00ff_ffff
                })
                .into(),
            ))
            .into_any_element()
    }

    fn sections(self) -> &'static [&'static str] {
        match self {
            Self::Browser => &["General", "Browsing", "Profiles"],
            Self::Ai => &["Commit", "Create Pull Request"],
            Self::General => &[
                "Closing terminals",
                "Files",
                "Window size",
                "Worktrees",
                "Voice",
            ],
            Self::Appearance => &["Themes", "Interface", "Agents Panel"],
            Self::Keyboard => &["Shortcuts"],
            Self::Terminal => super::terminal::SECTIONS,
            Self::Server => &["Connection", "Sessions", "Server control"],
            Self::Mobile => &["Access", "Pairing", "Devices"],
            Self::Commands => &[],
            Self::Backup => &["Backup & Restore", "Migration", "Configuration"],
        }
    }
}

impl SettingsView {
    fn select_category(
        &mut self,
        category: Category,
        section: Option<&'static str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_terminal_slider(cx);
        self.recording = None;
        if section.is_none()
            && self.category == category
            && self.query.is_empty()
            && !self.expanded.remove(&category)
        {
            self.expanded.insert(category);
        }
        self.category = category;
        self.section = section;
        self.query.clear();
        self.results.reset();
        self.search.update(cx, |search, cx| search.set_text("", cx));
        self.focus.focus(window);
        if category == Category::Server {
            cx.emit(SettingsEvent::ReadServer);
        }
        if category == Category::Mobile {
            cx.emit(SettingsEvent::ReadMobile);
        }
        cx.notify();
    }

    fn categories(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut categories = div().flex().gap(px(2.0)).map(|nav| {
            if self.compact {
                nav.flex_wrap()
            } else {
                nav.flex_col()
            }
        });
        for category in Category::ALL {
            let selected = self.query.is_empty() && self.category == category;
            let expanded = self.expanded.contains(&category);
            categories = categories.child(
                navigation::item(
                    self.layout_style(),
                    SharedString::from(format!("settings-category-{}", category.label())),
                    selected,
                    cx.listener(move |view, _, window, cx| {
                        view.select_category(category, None, window, cx);
                    }),
                )
                .debug_selector(move || format!("settings-category-{}", category.label()))
                .when(category == Category::General, |row| {
                    row.track_focus(&self.navbar_focus)
                })
                .h(px(28.0))
                .px(px(style::GUTTER))
                .rounded(px(6.0))
                .text_size(px(13.0))
                .when(
                    matches!(
                        category,
                        Category::Terminal | Category::Browser | Category::Backup
                    ),
                    |row| row.mt(px(style::GUTTER)),
                )
                .when(selected, |row| row.bg(self.theme.fg_alpha(0.10)))
                .child(category.tile())
                .child(category.label()),
            );
            if expanded && !self.compact {
                for &section in category.sections() {
                    if category == Category::Terminal
                        && section == "Migration"
                        && self.snapshot.terminal.diagnostics.is_empty()
                    {
                        continue;
                    }
                    categories = categories.child(
                        navigation::subitem(
                            self.layout_style(),
                            SharedString::from(format!("settings-subcategory-{section}")),
                            selected && self.section == Some(section),
                            cx.listener(move |view, _, window, cx| {
                                view.select_category(category, Some(section), window, cx);
                            }),
                        )
                        .debug_selector(move || format!("settings-subcategory-{section}"))
                        .child(section),
                    );
                }
            }
        }
        categories.into_any_element()
    }

    fn titlebar(&self, id: &'static str) -> gpui::Stateful<gpui::Div> {
        let focus = self.focus.clone();
        titlebar::background(id).on_mouse_down(gpui::MouseButton::Left, move |_, window, _| {
            focus.focus(window);
        })
    }

    pub(super) fn navigation(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        div()
            .debug_selector(|| "settings-navigation".into())
            .flex_none()
            .map(|nav| {
                if self.compact {
                    nav.w_full()
                } else {
                    nav.w(px(220.0)).h_full()
                }
            })
            .flex()
            .flex_col()
            .bg(self.theme.raised())
            .map(|nav| titlebar::top_corners(nav, window, true, self.compact))
            .border_color(self.theme.border)
            .child(
                self.titlebar("settings-sidebar-titlebar")
                    .h(px(if cfg!(target_os = "linux") && !self.compact {
                        12.0
                    } else {
                        40.0
                    }))
                    .flex_none(),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_h(px(0.0))
                    .flex_1()
                    .px(px(style::GUTTER))
                    .gap(px(style::GUTTER))
                    .child(
                        controls::search_field(
                            self.layout_style(),
                            "settings-search",
                            &self.search,
                        )
                        .debug_selector(|| "settings-search".into()),
                    )
                    .child(self.server_status(cx))
                    .child(
                        div()
                            .id("settings-categories")
                            .min_h(px(0.0))
                            .overflow_y_scroll()
                            .child(self.categories(cx)),
                    )
                    .when(!self.compact, |nav| nav.child(div().flex_1()))
                    .when(self.compact, |nav| nav.pb(px(12.0))),
            )
            .into_any_element()
    }

    fn server_status(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("settings-server-status")
            .flex()
            .items_center()
            .gap(px(style::GUTTER))
            .px(px(style::GUTTER))
            .py(px(style::GUTTER))
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(|row| row.bg(self.theme.hover))
            .button_interaction(cx.listener(|view, _, window, cx| {
                view.select_category(Category::Server, None, window, cx);
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(32.0))
                    .rounded_full()
                    .bg(self.theme.accent)
                    .child(IconGlyph::new(
                        Icon::Terminal,
                        px(20.0),
                        gpui::rgb(0x00ff_ffff).into(),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .text_size(px(13.0))
                    .child(Category::Server.label())
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(self.theme.fg_muted)
                            .child(match self.snapshot.connection {
                                crate::model::ConnectionState::Connecting => "Connecting…",
                                crate::model::ConnectionState::Ready => "Running",
                                crate::model::ConnectionState::Disconnected => "Stopped",
                            }),
                    ),
            )
            .into_any_element()
    }

    fn category_navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let index = Category::ALL
            .iter()
            .position(|category| *category == self.category)
            .unwrap_or(0);
        div()
            .flex()
            .items_center()
            .rounded_full()
            .bg(self.theme.surface)
            .children(
                [(false, Icon::ChevronLeft), (true, Icon::ChevronRight)]
                    .into_iter()
                    .map(|(forward, icon)| {
                        let target = if forward {
                            index
                                .checked_add(1)
                                .filter(|index| *index < Category::ALL.len())
                        } else {
                            index.checked_sub(1)
                        };
                        div()
                            .id(if forward {
                                "settings-forward"
                            } else {
                                "settings-back"
                            })
                            .debug_selector(move || {
                                if forward {
                                    "settings-forward".into()
                                } else {
                                    "settings-back".into()
                                }
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(28.0))
                            .h(px(26.0))
                            .rounded_full()
                            .when(forward, |button| {
                                button.border_l_1().border_color(self.theme.border)
                            })
                            .when(target.is_none(), |button| button.opacity(0.3))
                            .when_some(target, |button, index| {
                                button
                                    .cursor_pointer()
                                    .hover(|button| button.bg(self.theme.hover))
                                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation();
                                    })
                                    .button_interaction(cx.listener(move |view, _, window, cx| {
                                        view.select_category(
                                            Category::ALL[index],
                                            None,
                                            window,
                                            cx,
                                        );
                                    }))
                            })
                            .child(IconGlyph::new(icon, px(16.0), self.theme.fg))
                    }),
            )
            .into_any_element()
    }

    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        self.titlebar("settings-content-titlebar")
            .h(px(if cfg!(target_os = "linux") {
                72.0
            } else {
                44.0
            }))
            .when(cfg!(target_os = "linux"), |header| header.pt(px(28.0)))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(style::GUTTER))
            .child(self.category_navigation(cx))
            .child(
                div()
                    .text_size(px(17.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(self.category.label()),
            )
            .child(div().flex_1())
            .when_some(self.configuration_file(), |header, file| {
                header.child(
                    div()
                        .id("settings-open-configuration")
                        .debug_selector(|| "settings-open-configuration".into())
                        .px(px(style::GUTTER))
                        .py(px(4.0))
                        .rounded(px(6.0))
                        .text_size(px(11.0))
                        .text_color(self.theme.fg_muted)
                        .cursor_pointer()
                        .hover(|button| button.bg(self.theme.hover).text_color(self.theme.fg))
                        .focus(|button| button.bg(self.theme.hover))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .button_interaction(cx.listener(move |_, _, _, cx| {
                            cx.emit(SettingsEvent::OpenConfiguration(file));
                        }))
                        .child(format!("Edit in {file}")),
                )
            })
            .into_any_element()
    }

    fn dismissible_error(
        &self,
        id: &'static str,
        error: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tooltip_theme = self.theme.clone();
        div()
            .debug_selector(move || format!("settings-{id}-error"))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(self.note(error, true).flex_1().min_w(px(0.0)))
            .child(
                div()
                    .id(SharedString::from(format!("settings-dismiss-{id}-error")))
                    .debug_selector(move || format!("settings-dismiss-{id}-error"))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(self.metrics.control_small())
                    .rounded(self.metrics.radius_sm())
                    .cursor_pointer()
                    .hover(|button| button.bg(self.theme.hover))
                    .focus(|button| button.bg(self.theme.hover))
                    .tooltip(move |_, cx| {
                        cx.new(|_| {
                            Tooltip::new(
                                "Dismiss",
                                tooltip_theme.raised(),
                                tooltip_theme.fg,
                                tooltip_theme.border,
                                tooltip_theme.bg,
                            )
                        })
                        .into()
                    })
                    .button_interaction(cx.listener(move |view, _, _, cx| {
                        view.dismiss_error(id, cx);
                    }))
                    .child(IconGlyph::new(
                        Icon::X,
                        self.metrics.icon_sm(),
                        self.theme.fg_muted,
                    )),
            )
            .into_any_element()
    }

    pub(super) fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        for anchor in self.picker_anchors.values() {
            anchor.set(None);
        }

        let view = cx.entity();
        div()
            .relative()
            .w_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .h_full()
            .flex()
            .flex_col()
            .px(px(style::GUTTER * 2.5))
            .child(self.header(cx))
            .children(
                ["application", "configuration"]
                    .into_iter()
                    .filter_map(|id| {
                        self.errors
                            .get(id)
                            .map(|error| self.dismissible_error(id, error, cx))
                    }),
            )
            .child(
                div()
                    .id("settings-sections")
                    .w_full()
                    .max_w(px(640.0))
                    .mx_auto()
                    .debug_selector(|| "settings-sections".into())
                    .flex_1()
                    .min_w(px(0.0))
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(
                        list(self.results.state.clone(), move |index, window, cx| {
                            view.update(cx, |view, cx| view.result(index, window, cx))
                        })
                        .size_full(),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "settings-scrollbar".into())
                    .absolute()
                    .right(px(3.0))
                    .top(px(44.0))
                    .bottom(px(8.0))
                    .child(
                        self.scrollbar
                            .element(&self.results.state, self.theme.fg_alpha(0.18)),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn layout_content(
        &mut self,
        size: gpui::Size<gpui::Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.compact = size.width < px(660.0);
        self.refresh_results(size.height, window, cx);
        div()
            .debug_selector(|| "settings-container".into())
            .size_full()
            .flex()
            .when(self.compact, Styled::flex_col)
            .child(if self.compact {
                self.navigation(window, cx)
            } else {
                gpui::AnyView::from(self.navigation_view.clone())
                    .cached(div().w(px(220.0)).h_full().flex_none().style().clone())
                    .into_any_element()
            })
            .child(
                gpui::AnyView::from(self.content_view.clone()).cached(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .min_h(px(0.0))
                        .h_full()
                        .style()
                        .clone(),
                ),
            )
            .into_any_element()
    }
}
