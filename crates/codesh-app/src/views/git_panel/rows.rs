use codesh_protocol::GitFileStatus;
use codesh_ui::components::{ButtonInteraction, Tooltip};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div,
};

use super::{GitPanel, GitPanelEvent};

use super::state::{Group, Row};

fn path(entry: &GitFileStatus) -> String {
    String::from_utf8_lossy(&entry.file.path.0).into_owned()
}

impl GitPanel {
    pub(super) fn action(
        &self,
        id: &str,
        label: &str,
        hint: &'static str,
        event: GitPanelEvent,
        cx: &Context<Self>,
    ) -> AnyElement {
        self.action_with(id, label, hint, move |_| event.clone(), cx)
    }

    fn action_with(
        &self,
        id: &str,
        label: &str,
        hint: &'static str,
        event: impl Fn(&Self) -> GitPanelEvent + 'static,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme.clone();
        div()
            .id(SharedString::from(id.to_owned()))
            .debug_selector({
                let id = id.to_owned();
                move || id.clone()
            })
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .min_w(self.metrics.control_small())
            .h(self.metrics.control_small())
            .px(self.metrics.spacing1())
            .rounded(self.metrics.radius_sm())
            .text_color(theme.fg_muted)
            .tooltip(move |_, cx| {
                cx.new(|_| Tooltip::new(hint, theme.raised(), theme.fg, theme.border, theme.bg))
                    .into()
            })
            .when(self.busy, |view| view.opacity(0.4))
            .when(!self.busy, |view| {
                view.cursor_pointer()
                    .hover(|style| style.bg(self.theme.hover))
                    .button_interaction(cx.listener(move |panel, _, _, cx| {
                        cx.stop_propagation();
                        if !panel.busy {
                            cx.emit(event(panel));
                        }
                    }))
            })
            .child(label.to_owned())
            .into_any_element()
    }

    pub(super) fn render_row(&self, row: Row, cx: &Context<Self>) -> AnyElement {
        match row {
            Row::Header(group) => self.header(group, cx),
            Row::File(group, index) => self.file_row(group, index, cx),
        }
    }

    fn header(&self, group: Group, cx: &Context<Self>) -> AnyElement {
        let indices = self.groups.get(group);
        let label = if group == Group::Staged {
            "Unstage All"
        } else {
            "Stage All"
        };
        div()
            .id(("git-group", group.index()))
            .w_full()
            .min_w_0()
            .flex()
            .items_center()
            .h(self.metrics.scaled(28.0))
            .gap(self.metrics.spacing2())
            .text_color(self.theme.fg_muted)
            .text_size(self.metrics.font_body())
            .font_weight(FontWeight::SEMIBOLD)
            .child(
                div()
                    .id(("git-collapse", group.index()))
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .gap(self.metrics.spacing2())
                    .cursor_pointer()
                    .button_interaction(cx.listener(move |panel, _, _, cx| {
                        panel.collapsed[group.index()] = !panel.collapsed[group.index()];
                        panel.rebuild_rows();
                        cx.notify();
                    }))
                    .child(if self.collapsed[group.index()] {
                        "›"
                    } else {
                        "⌄"
                    })
                    .child(div().truncate().child(format!(
                        "{} ({})",
                        group.label(),
                        indices.len()
                    ))),
            )
            .child(self.action_with(
                &format!("git-all-{}", group.index()),
                label,
                label,
                move |panel| {
                    let paths = panel
                        .groups
                        .get(group)
                        .iter()
                        .filter_map(|&index| panel.file(index))
                        .map(path)
                        .collect();
                    if group == Group::Staged {
                        GitPanelEvent::Unstage(paths)
                    } else {
                        GitPanelEvent::Stage(paths)
                    }
                },
                cx,
            ))
            .into_any_element()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Declarative file row with contextual Git actions"
    )]
    fn file_row(&self, group: Group, index: usize, cx: &Context<Self>) -> AnyElement {
        let Some(entry) = self.file(index) else {
            return div().into_any_element();
        };
        let path = path(entry);
        let staged = group == Group::Staged;
        let stat = if staged {
            &entry.staged
        } else {
            &entry.unstaged
        };
        let code = match group {
            Group::Conflicts | Group::Untracked => b'U',
            Group::Staged => entry.file.index,
            Group::Changes => entry.file.worktree,
        };
        let color = match code {
            b'U' if group == Group::Conflicts => self.theme.danger,
            b'A' | b'?' | b'U' => self.theme.diff_add,
            b'D' => self.theme.diff_remove,
            b'R' | b'C' => self.theme.accent,
            _ => self.theme.warning,
        };
        let (directory, name) = path.rsplit_once('/').unwrap_or(("", &path));
        let directory = directory.to_owned();
        let name = name.to_owned();
        let row_id = SharedString::from(format!("git-file-{}-{path}", group.index()));
        let open = path.clone();
        let mut counts = div()
            .id(SharedString::from(format!(
                "git-counts-{}-{index}",
                group.index()
            )))
            .debug_selector(move || format!("git-counts-{}-{index}", group.index()))
            .flex()
            .flex_none()
            .whitespace_nowrap()
            .justify_end()
            .gap(self.metrics.spacing2());
        if stat.binary {
            counts = counts.child(div().text_color(self.theme.fg_muted).child("Binary"));
        } else {
            if let Some(added) = stat.additions {
                counts = counts.child(
                    div()
                        .text_color(self.theme.diff_add)
                        .child(format!("+{added}")),
                );
            }
            if let Some(removed) = stat.deletions {
                counts = counts.child(
                    div()
                        .text_color(self.theme.diff_remove)
                        .child(format!("−{removed}")),
                );
            }
        }
        let mut actions = div()
            .flex()
            .flex_none()
            .invisible()
            .group_hover("git-file-row", Styled::visible);
        if staged {
            actions = actions.child(self.action(
                &format!("git-unstage-{index}"),
                "−",
                "Unstage",
                GitPanelEvent::Unstage(vec![path]),
                cx,
            ));
        } else {
            actions = actions.child(self.action(
                &format!("git-stage-{index}"),
                "+",
                "Stage",
                GitPanelEvent::Stage(vec![path.clone()]),
                cx,
            ));
            if group != Group::Conflicts {
                actions = actions.child(self.action(
                    &format!("git-discard-{index}"),
                    "↺",
                    "Discard",
                    GitPanelEvent::Discard(vec![path]),
                    cx,
                ));
            }
        }
        div()
            .id(row_id)
            .debug_selector(move || format!("git-file-{}-{index}", group.index()))
            .group("git-file-row")
            .w_full()
            .flex()
            .items_center()
            .h(self.metrics.scaled(28.0))
            .gap(self.metrics.spacing2())
            .min_w_0()
            .rounded(self.metrics.radius_sm())
            .hover(|style| style.bg(self.theme.hover))
            .when(!self.busy, |view| {
                view.cursor_pointer()
                    .on_click(cx.listener(move |panel, _, _, cx| {
                        if !panel.busy {
                            cx.emit(GitPanelEvent::OpenDiff {
                                path: open.clone(),
                                staged,
                            });
                        }
                    }))
            })
            .child(
                div()
                    .flex_none()
                    .w(self.metrics.scaled(16.0))
                    .text_color(color)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(char::from(code).to_string()),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .gap(self.metrics.spacing2())
                    .child(div().flex_none().whitespace_nowrap().child(name))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(self.theme.fg_muted)
                            .child(directory),
                    ),
            )
            .child(counts)
            .child(actions)
            .into_any_element()
    }
}
