mod rows;
mod state;
#[cfg(test)]
mod tests;

use codesh_protocol::{GitFileStatus, GitStatus, ProjectId};
use codesh_ui::components::{IconButton, IconGlyph};
use codesh_ui::controls::{self, Style};
use codesh_ui::icon::Icon;
use codesh_ui::text_input::{InputEvent, InputStyle, TextInput};
use codesh_ui::theme::{Metrics, Theme};
use gpui::prelude::FluentBuilder;
use gpui::{
    Animation, AnimationExt, App, AppContext, Context, Entity, EventEmitter, FocusHandle,
    Focusable, InteractiveElement, IntoElement, ParentElement, Render, Styled, Subscription,
    Transformation, Window, div, radians, svg, uniform_list,
};

use super::codesh::style::spacing;
use state::{Group, Groups, Row, commit_enabled};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GitPanelEvent {
    Close,
    Refresh,
    Stage(Vec<String>),
    Unstage(Vec<String>),
    Discard(Vec<String>),
    Commit { message: String, amend: bool },
    OpenDiff { path: String, staged: bool },
}

pub(crate) struct GitPanel {
    pub(crate) name: String,
    theme: Theme,
    metrics: Metrics,
    status: Option<GitStatus>,
    groups: Groups,
    rows: Vec<Row>,
    collapsed: [bool; 4],
    message: Entity<TextInput>,
    amend: bool,
    busy: bool,
    error: Option<String>,
    _input_subscription: Subscription,
}

impl EventEmitter<GitPanelEvent> for GitPanel {}

impl Focusable for GitPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.message.focus_handle(cx)
    }
}

impl GitPanel {
    pub(crate) fn new(
        _project: ProjectId,
        theme: &Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let message = cx.new(|cx| {
            TextInput::new(InputStyle::field(theme, &metrics), cx)
                .multiline()
                .with_placeholder("Message (⌘↩ to commit)")
        });
        let subscription = cx.subscribe(&message, |panel, _, event, cx| match event {
            InputEvent::Changed => cx.notify(),
            InputEvent::Submitted => panel.commit(cx),
            InputEvent::Cancelled => {}
        });
        Self {
            theme: theme.clone(),
            metrics,
            name: "Source Control".into(),
            status: None,
            groups: Groups::default(),
            rows: Vec::new(),
            collapsed: [false; 4],
            message,
            amend: false,
            busy: false,
            error: None,
            _input_subscription: subscription,
        }
    }

    pub(crate) fn set_status(&mut self, status: GitStatus, cx: &mut Context<Self>) {
        self.groups = Groups::from_status(&status);
        self.status = Some(status);
        self.error = None;
        self.rebuild_rows();
        cx.notify();
    }

    pub(crate) fn set_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        self.busy = busy;
        cx.notify();
    }

    pub(crate) fn set_error(&mut self, message: Option<String>, cx: &mut Context<Self>) {
        self.error = message.map(|error| crate::model::native::friendly_error(&error, true));
        cx.notify();
    }

    pub(crate) fn committed(&mut self, cx: &mut Context<Self>) {
        self.message.update(cx, |input, cx| input.set_text("", cx));
        self.amend = false;
        cx.notify();
    }

    pub(crate) fn set_theme(&mut self, theme: &Theme, cx: &mut Context<Self>) {
        self.theme = theme.clone();
        self.message.update(cx, |input, cx| {
            input.set_style(InputStyle::field(theme, &self.metrics), cx);
        });
        cx.notify();
    }

    fn header_action(
        &self,
        id: &'static str,
        icon: Icon,
        label: &'static str,
        event: GitPanelEvent,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        IconButton::new(
            id,
            icon,
            gpui::px(16.0),
            gpui::px(20.0),
            self.theme.fg_muted,
            self.theme.fg,
        )
        .tooltip(
            label,
            self.theme.raised(),
            self.theme.fg,
            self.theme.border,
            self.theme.bg,
        )
        .on_click(cx.listener(move |_, _, _, cx| cx.emit(event.clone())))
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        for group in Group::ALL {
            let indices = self.groups.get(group);
            if indices.is_empty() {
                continue;
            }
            self.rows.push(Row::Header(group));
            if !self.collapsed[group.index()] {
                self.rows
                    .extend(indices.iter().map(|&index| Row::File(group, index)));
            }
        }
    }

    fn toggle_amend(&mut self, cx: &mut Context<Self>) {
        if !self.busy {
            self.amend = !self.amend;
            cx.notify();
        }
    }

    fn can_commit(&self, cx: &App) -> bool {
        self.status.is_some()
            && commit_enabled(
                self.message.read(cx).text(),
                self.amend,
                self.busy,
                self.groups.get(Group::Staged).len(),
                self.groups.get(Group::Conflicts).len(),
            )
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        if self.can_commit(cx) {
            cx.emit(GitPanelEvent::Commit {
                message: self.message.read(cx).text().to_owned(),
                amend: self.amend,
            });
        }
    }

    fn file(&self, index: usize) -> Option<&GitFileStatus> {
        self.status.as_ref()?.files.get(index)
    }
}

impl Render for GitPanel {
    #[allow(clippy::too_many_lines, reason = "Declarative panel layout")]
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.theme;
        let m = self.metrics;
        let style = Style { theme, metrics: &m };
        // The hidden text measures wrapped lines without a layout/notify cycle.
        let mut measure = self.message.read(cx).text().to_owned();
        measure.push('\u{200b}');
        let mut branch_line = div()
            .flex()
            .items_center()
            .gap(spacing())
            .h(gpui::px(super::codesh::style::HEADER))
            .flex_none()
            .text_color(theme.fg_muted)
            .text_size(m.font_footnote())
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .min_w_0()
            .child(IconGlyph::new(Icon::GitBranch, m.icon_md(), theme.fg_muted))
            .child(div().flex_1().min_w_0().truncate().child(self.name.clone()));
        if self.busy {
            branch_line = branch_line.child(
                svg()
                    .path(Icon::Refresh.path())
                    .size(m.icon_md())
                    .text_color(theme.accent)
                    .with_animation(
                        "git-busy",
                        Animation::new(std::time::Duration::from_secs(1)).repeat(),
                        |svg, delta| {
                            svg.with_transformation(Transformation::rotate(radians(
                                delta * std::f32::consts::TAU,
                            )))
                        },
                    ),
            );
        } else {
            branch_line = branch_line.child(self.header_action(
                "git-refresh",
                Icon::Refresh,
                "Refresh",
                GitPanelEvent::Refresh,
                cx,
            ));
        }
        branch_line = branch_line.child(self.header_action(
            "git-close",
            Icon::X,
            "Close",
            GitPanelEvent::Close,
            cx,
        ));
        let mut root = div()
            .id("git-panel")
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .px(spacing())
            .pt(spacing())
            .gap(spacing())
            .bg(theme.bg)
            .text_color(theme.fg)
            .text_size(m.font_body())
            .child(branch_line)
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .min_w_0()
                    .min_h(m.line_height_field() * 3.0 + spacing() * 2.0)
                    .max_h(m.line_height_field() * 8.0 + spacing() * 2.0)
                    .overflow_hidden()
                    .rounded(m.radius_sm())
                    .bg(theme.surface)
                    .on_key_down(cx.listener(|panel, event: &gpui::KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter"
                            && event.keystroke.modifiers.platform
                            && !event.keystroke.modifiers.shift
                            && !event.keystroke.modifiers.alt
                            && !event.keystroke.modifiers.control
                        {
                            cx.stop_propagation();
                            panel.commit(cx);
                        }
                    }))
                    .child(
                        div()
                            .invisible()
                            .p(spacing())
                            .text_size(m.font_emphasis())
                            .line_height(m.line_height_field())
                            .child(measure),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .flex_col()
                            .p(spacing())
                            .child(self.message.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(spacing())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(spacing())
                            .when(self.busy, |view| view.opacity(0.4))
                            .child(controls::checkbox(
                                style,
                                "git-amend",
                                self.amend,
                                false,
                                cx.listener(|panel, _, _, cx| panel.toggle_amend(cx)),
                            ))
                            .child("Amend"),
                    )
                    .child(controls::button(
                        style,
                        "git-commit",
                        if self.amend { "Amend" } else { "Commit" },
                        self.can_commit(cx),
                        cx.listener(|panel, _, _, cx| panel.commit(cx)),
                    )),
            );
        if let Some(error) = &self.error {
            root = root.child(
                div()
                    .p(spacing())
                    .text_color(theme.fg_muted)
                    .child(error.clone())
                    .when(
                        error == codesh_ui::tr!("Not a Git repository").as_ref(),
                        |view| {
                            view.child(div().child(codesh_ui::tr!(
                                "Open a folder containing a Git repository to view changes."
                            )))
                        },
                    ),
            );
        }
        if self.rows.is_empty() && self.error.is_none() {
            root = root.child(div().p(spacing()).text_color(theme.fg_muted).child(
                if self.status.is_some() {
                    "No changes"
                } else {
                    "Loading changes…"
                },
            ));
        } else {
            root = root.child(
                uniform_list(
                    "git-panel-files",
                    self.rows.len(),
                    cx.processor(|panel, range: std::ops::Range<usize>, _, cx| {
                        range
                            .filter_map(|index| panel.rows.get(index).copied())
                            .map(|row| panel.render_row(row, cx))
                            .collect::<Vec<_>>()
                    }),
                )
                .flex_1()
                .min_h_0(),
            );
        }
        root
    }
}
