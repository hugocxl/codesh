use codesh_protocol::{GitAction, GitMergeMethod, GitPullRequest, GitPullRequestAction, ProjectId};
use codesh_ui::components::{ButtonInteraction, SymbolGlyph};
use codesh_ui::controls::{self, Style};
use codesh_ui::theme::{Metrics, Theme};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, ClickEvent, Context, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, Styled, Window, div, px, relative,
};

use crate::model::AppModel;
use crate::views::overlays::Overlay;

pub(crate) struct PullRequestPopover {
    project: ProjectId,
    number: u64,
    merge_method: GitMergeMethod,
    /// A worktree's pull request can remove the worktree once merged.
    remove_worktree: bool,
}

fn can_merge(pr: &GitPullRequest) -> bool {
    pr.state == "OPEN"
        && !pr.draft
        && pr.mergeable != Some(false)
        && !matches!(
            pr.merge_state.as_str(),
            "DIRTY" | "BEHIND" | "BLOCKED" | "DRAFT"
        )
}

fn merge_action_label(method: GitMergeMethod) -> SharedString {
    match method {
        GitMergeMethod::Merge => "Merge Commit".into(),
        GitMergeMethod::Squash => "Squash and Merge".into(),
        GitMergeMethod::Rebase => "Rebase and Merge".into(),
    }
}

impl AppModel {
    pub(crate) fn open_pull_request(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::PullRequest(PullRequestPopover { project: current, .. })) if current == project)
        {
            self.dismiss_overlay(cx);
            return;
        }
        let Some(pr) = self
            .git
            .projects
            .get(&project)
            .and_then(|repo| repo.pull_request.as_ref())
        else {
            return;
        };
        self.overlay = Some(Overlay::PullRequest(PullRequestPopover {
            project,
            number: pr.number,
            merge_method: GitMergeMethod::Squash,
            remove_worktree: false,
        }));
        self.queue_git_refresh(
            project,
            vec![GitAction::PullRequest(GitPullRequestAction::Info)],
            cx,
        );
        cx.notify();
    }

    pub(crate) fn refresh_pull_request(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.queue_git_refresh(
            project,
            vec![GitAction::PullRequest(GitPullRequestAction::Info)],
            cx,
        );
    }

    pub(crate) fn set_pull_request_merge_method(
        &mut self,
        project: ProjectId,
        method: GitMergeMethod,
        cx: &mut Context<Self>,
    ) {
        if let Some(Overlay::PullRequest(popover)) = &mut self.overlay
            && popover.project == project
        {
            popover.merge_method = method;
            cx.notify();
        }
    }

    fn toggle_merge_removal(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        if let Some(Overlay::PullRequest(popover)) = &mut self.overlay
            && popover.project == project
        {
            popover.remove_worktree = !popover.remove_worktree;
            cx.notify();
        }
    }

    pub(crate) fn open_pull_request_url(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let url = self
            .git
            .projects
            .get(&project)
            .and_then(|repo| repo.pull_request.as_ref())
            .map(|pr| pr.url.clone());
        if let Some(url) = url.filter(|url| url.starts_with("https://")) {
            cx.open_url(&url);
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Keep confirmation and PR identity checks together"
    )]
    pub(crate) fn request_pull_request_action(
        &mut self,
        project: ProjectId,
        action: GitPullRequestAction,
        cx: &mut Context<Self>,
    ) {
        let Some(pr) = self
            .git
            .projects
            .get(&project)
            .and_then(|repo| repo.pull_request.as_ref())
        else {
            return;
        };
        let number = pr.number;
        let head_oid = pr.head_oid.clone();
        let matches_current = match &action {
            GitPullRequestAction::Merge { number: target, .. }
            | GitPullRequestAction::Close { number: target }
            | GitPullRequestAction::UpdateBranch { number: target, .. } => *target == number,
            _ => false,
        };
        let reviewed_head = match &action {
            GitPullRequestAction::UpdateBranch { expected_head, .. } => Some(expected_head),
            GitPullRequestAction::Merge { expected_head, .. } => expected_head.as_ref(),
            _ => None,
        };
        if !matches_current || reviewed_head.is_some_and(|head| *head != head_oid) {
            return;
        }
        let remove_worktree = matches!(action, GitPullRequestAction::Merge { .. })
            && matches!(
                &self.overlay,
                Some(Overlay::PullRequest(popover))
                    if popover.project == project && popover.remove_worktree
            );
        if self.ai.running(project)
            || self
                .git
                .projects
                .get(&project)
                .is_some_and(super::Repository::busy)
            || self.close_prompt.is_some()
            || !self.session_listing_ready()
        {
            return;
        }
        let (title, message, label) = match &action {
            GitPullRequestAction::Merge { method, .. } if can_merge(pr) && remove_worktree => (
                "Merge pull request?",
                format!(
                    "Apply {} to pull request #{} ({}) at its reviewed commit {}? Afterwards Codesh asks to remove this worktree and switches to the primary checkout.",
                    &merge_action_label(*method),
                    number,
                    &pr.title,
                    &pr.head_oid[..pr.head_oid.len().min(7)]
                ),
                merge_action_label(*method),
            ),
            GitPullRequestAction::Merge { method, .. } if can_merge(pr) => (
                "Merge pull request?",
                format!(
                    "Apply {} to pull request #{} ({}) at its reviewed commit {}? Afterwards Codesh switches this repository to {} and fast-forwards it, unless another worktree has it checked out.",
                    &merge_action_label(*method),
                    number,
                    &pr.title,
                    &pr.head_oid[..pr.head_oid.len().min(7)],
                    &pr.base_branch
                ),
                merge_action_label(*method),
            ),
            GitPullRequestAction::Close { .. } if pr.state == "OPEN" => (
                "Close pull request?",
                format!(
                    "Close pull request #{} ({}) without merging?",
                    number, &pr.title
                ),
                "Close".into(),
            ),
            GitPullRequestAction::UpdateBranch { .. }
                if pr.state == "OPEN" && pr.merge_state == "BEHIND" && !pr.cross_repository =>
            {
                (
                    "Update pull request branch?",
                    format!(
                        "Merge {} into {} and push the updated branch?",
                        &pr.base_branch, &pr.head_branch
                    ),
                    "Update".into(),
                )
            }
            _ => return,
        };
        self.dismiss_overlay(cx);
        let window = self.window;
        let context = self.git.interaction;
        self.close_prompt = Some(cx.spawn(async move |this, cx| {
            let (send, receive) = async_channel::bounded(1);
            let dialog = window.update(cx, |_, window, cx| {
                codesh_ui::dialog::confirm(
                    window,
                    cx,
                    title,
                    &message,
                    &label,
                    None,
                    move |answer| {
                        let _ = send.try_send(answer);
                    },
                )
            });
            let confirmed = if let Ok(Ok(_dialog)) = dialog {
                matches!(
                    receive.recv().await,
                    Ok(codesh_ui::dialog::ConfirmationResponse::Confirmed { .. })
                )
            } else {
                false
            };
            let _ = this.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                if !confirmed {
                    return;
                }
                let fresh = model
                    .git
                    .projects
                    .get(&project)
                    .and_then(|repo| repo.pull_request.as_ref())
                    .is_some_and(|pr| {
                        pr.number == number && pr.head_oid == head_oid && pr.state == "OPEN"
                    });
                if model.git.interaction != context || !fresh {
                    model.fail_detail(
                        "Pull request changed".into(),
                        "It changed after you confirmed. Refresh and try again.",
                        cx,
                    );
                    return;
                }
                if remove_worktree {
                    model.merge_and_remove_worktree(project, action, cx);
                } else {
                    model.git_request(project, GitAction::PullRequest(action), cx);
                }
            });
        }));
    }
}

fn detail_row(m: Metrics, theme: &Theme, label: &str, value: String, color: Hsla) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(m.spacing3())
        .text_size(m.font_footnote())
        .child(
            div()
                .flex_none()
                .w(m.scaled(52.0))
                .text_color(theme.fg_muted)
                .child(label.to_owned()),
        )
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .font_family(codesh_ui::font::monospace())
                .font_weight(FontWeight::MEDIUM)
                .text_color(color)
                .child(value),
        )
        .into_any_element()
}

#[derive(Clone, Copy)]
enum ActionTone {
    Regular,
    Primary,
    Danger,
}

fn action_button(
    style: Style<'_>,
    id: &'static str,
    symbol: &'static str,
    label: String,
    tone: ActionTone,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Style { theme, metrics: m } = style;
    let (foreground, background) = match tone {
        _ if !enabled => (theme.fg_dim, theme.surface),
        ActionTone::Regular => (theme.fg, theme.surface),
        ActionTone::Primary => (theme.accent_foreground, theme.accent),
        ActionTone::Danger => (theme.danger, theme.surface),
    };
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
        .px(m.spacing4())
        .py(m.spacing3())
        .rounded(m.radius_sm())
        .bg(background)
        .text_size(m.font_footnote())
        .font_weight(FontWeight::MEDIUM)
        .text_color(foreground)
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .when(!matches!(tone, ActionTone::Primary), |button| {
                    button.hover(|button| button.bg(theme.hover))
                })
                .button_interaction(on_click)
        })
        .child(SymbolGlyph::new(symbol, m.font_footnote(), foreground))
        .child(div().min_w(px(0.0)).truncate().child(label))
        .into_any_element()
}

const MERGE_METHODS: [(GitMergeMethod, &str, &str); 3] = [
    (GitMergeMethod::Squash, "pr-method-squash", "Squash"),
    (GitMergeMethod::Merge, "pr-method-merge", "Merge"),
    (GitMergeMethod::Rebase, "pr-method-rebase", "Rebase"),
];

fn merge_selector(
    popover: &PullRequestPopover,
    style: Style<'_>,
    enabled: bool,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let Style { theme, metrics: m } = style;
    let project = popover.project;
    let mut selector = div()
        .flex()
        .flex_none()
        .items_center()
        .p(m.spacing1())
        .rounded(m.radius_md())
        .bg(theme.hover)
        .text_size(m.font_footnote())
        .when(!enabled, |selector| selector.opacity(0.4));
    let mut previous_selected = None;
    for (method, id, label) in MERGE_METHODS {
        let selected = popover.merge_method == method;
        if previous_selected == Some(false) && !selected {
            selector = selector.child(
                div()
                    .flex_none()
                    .w(px(1.0))
                    .h(m.scaled(14.0))
                    .bg(theme.border),
            );
        }
        previous_selected = Some(selected);
        selector = selector.child(
            div()
                .id(id)
                .flex()
                .flex_1()
                .justify_center()
                .py(m.scaled(5.0))
                .rounded(m.radius_sm())
                .map(|segment| {
                    if selected {
                        segment
                            .bg(theme.surface)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.fg)
                    } else {
                        segment.text_color(theme.fg_muted)
                    }
                })
                .when(enabled, |segment| {
                    segment
                        .cursor_pointer()
                        .hover(|segment| segment.text_color(theme.fg))
                        .button_interaction(cx.listener(move |model, _, _, cx| {
                            model.set_pull_request_merge_method(project, method, cx);
                        }))
                })
                .child(label),
        );
    }
    selector.into_any_element()
}

fn merge_removal(
    popover: &PullRequestPopover,
    style: Style<'_>,
    enabled: bool,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let Style { theme, metrics: m } = style;
    let project = popover.project;
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
        .text_size(m.font_footnote())
        .text_color(theme.fg_muted)
        .when(!enabled, |row| row.opacity(0.4))
        .child(controls::toggle(
            style,
            "pr-remove-worktree",
            popover.remove_worktree,
            cx.listener(move |model, _, _, cx| {
                if enabled {
                    model.toggle_merge_removal(project, cx);
                }
            }),
        ))
        .child("Remove worktree after merge")
        .into_any_element()
}

fn merge_status(pr: &GitPullRequest, theme: &Theme) -> Option<(SharedString, Hsla)> {
    if pr.draft {
        return Some(("Draft".into(), theme.fg_muted));
    }
    match pr.merge_state.as_str() {
        "DIRTY" => Some(("Conflicts".into(), theme.danger)),
        "BEHIND" => Some(("Behind base".into(), theme.danger)),
        "BLOCKED" => Some(("Blocked".into(), theme.danger)),
        "DRAFT" => Some(("Draft".into(), theme.fg_muted)),
        "UNSTABLE" if pr.checks.failing > 0 => Some(("Checks failing".into(), theme.warning)),
        "UNSTABLE" if pr.checks.pending > 0 => Some(("Checks running".into(), theme.warning)),
        "CLEAN" | "HAS_HOOKS" | "UNSTABLE" => Some(("Ready".into(), theme.diff_add)),
        _ if pr.mergeable == Some(true) => Some(("Ready".into(), theme.diff_add)),
        _ if pr.mergeable == Some(false) => Some(("Conflicts".into(), theme.danger)),
        _ => None,
    }
}

fn checks_status(pr: &GitPullRequest, theme: &Theme) -> Option<(String, Hsla)> {
    let checks = &pr.checks;
    if checks.failing > 0 {
        Some((format!("{} failing", checks.failing), theme.danger))
    } else if checks.pending > 0 {
        Some((format!("{} running", checks.pending), theme.warning))
    } else if checks.passing > 0 {
        Some((
            format!("{}/{} passing", checks.passing, checks.passing),
            theme.diff_add,
        ))
    } else {
        None
    }
}

fn header(
    pr: &GitPullRequest,
    project: ProjectId,
    busy: bool,
    style: Style<'_>,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let Style { theme, metrics: m } = style;
    let (symbol, state, state_color) = match (pr.state.as_str(), pr.draft) {
        ("OPEN", true) => ("pencil.circle", "Draft · Open", theme.fg_muted),
        ("OPEN", false) if pr.checks.failing > 0 => ("xmark.octagon.fill", "Open", theme.danger),
        ("OPEN", false) if pr.checks.pending > 0 => ("clock", "Open", theme.warning),
        ("OPEN", false) => ("arrow.triangle.pull", "Open", theme.diff_add),
        ("MERGED", _) => ("checkmark.circle.fill", "Merged", theme.accent),
        _ => ("xmark.circle", "Closed", theme.danger),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing4())
        .child(SymbolGlyph::new(symbol, m.font_headline(), state_color))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.0))
                .gap(m.spacing1())
                .child(
                    div()
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("Pull Request #{}", pr.number)),
                )
                .child(
                    div()
                        .text_size(m.font_caption())
                        .text_color(theme.fg_muted)
                        .child(state),
                ),
        )
        .child(
            div()
                .id("pr-refresh")
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(m.control_small())
                .rounded(m.radius_sm())
                .when(!busy, |button| {
                    button
                        .cursor_pointer()
                        .hover(|button| button.bg(theme.hover))
                        .button_interaction(cx.listener(move |model, _, _, cx| {
                            model.refresh_pull_request(project, cx);
                        }))
                })
                .when(busy, |button| button.opacity(0.4))
                .child(SymbolGlyph::new(
                    "arrow.clockwise",
                    m.font_footnote(),
                    theme.fg_muted,
                )),
        )
        .into_any_element()
}

fn details(pr: &GitPullRequest, has_local_changes: bool, style: Style<'_>) -> AnyElement {
    let Style { theme, metrics: m } = style;
    let mut details = div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(m.spacing3())
        .child(detail_row(
            *m,
            theme,
            "Base",
            pr.base_branch.clone(),
            theme.fg,
        ));
    if let Some((label, color)) = merge_status(pr, theme) {
        details = details.child(detail_row(*m, theme, "Merge", label.into(), color));
    }
    if let Some((label, color)) = checks_status(pr, theme) {
        details = details.child(detail_row(*m, theme, "Checks", label, color));
    }
    if has_local_changes {
        details = details.child(detail_row(
            *m,
            theme,
            "Local",
            "Uncommitted changes".into(),
            theme.warning,
        ));
    }
    details.into_any_element()
}

#[allow(
    clippy::too_many_lines,
    reason = "Render the PR status and its actions together"
)]
pub(crate) fn render_pr(
    popover: &PullRequestPopover,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let m = model.metrics;
    let style = Style { theme, metrics: &m };
    let repository = model.git.projects.get(&popover.project);
    let pr = repository
        .and_then(|repo| repo.pull_request.as_ref())
        .filter(|pr| pr.number == popover.number);
    let Some(pr) = pr else {
        return codesh_ui::popover::surface(theme, m)
            .w(m.scaled(280.0))
            .child(codesh_ui::popover::body(m).child("Pull request changed. Reopen its status."))
            .into_any_element();
    };
    let project = popover.project;
    let number = pr.number;
    let merge_head = pr.head_oid.clone();
    let merge_method = popover.merge_method;
    let busy = model.ai.running(project) || repository.is_some_and(super::Repository::busy);
    let has_local_changes = repository
        .and_then(|repo| repo.summary.as_ref())
        .is_some_and(|summary| summary.changed > summary.untracked);
    codesh_ui::popover::surface(theme, m)
        .w(m.scaled(280.0))
        .p(m.spacing6())
        .gap(m.spacing5())
        .line_height(relative(1.2))
        .child(header(pr, project, busy, style, cx))
        .child(details(pr, has_local_changes, style))
        .child(div().flex_none().h(px(1.0)).bg(theme.border))
        .child(action_button(
            style,
            "pr-open",
            "arrow.up.right.square",
            "Open on GitHub".into(),
            ActionTone::Regular,
            true,
            cx.listener(move |model, _, _, cx| model.open_pull_request_url(project, cx)),
        ))
        .when(pr.state == "OPEN", |surface| {
            surface
                .when(
                    pr.merge_state == "BEHIND" && !pr.cross_repository,
                    |surface| {
                        let head_oid = pr.head_oid.clone();
                        surface.child(action_button(
                            style,
                            "pr-update-branch",
                            "arrow.down.circle",
                            format!("Update from {}", &pr.base_branch),
                            ActionTone::Regular,
                            !busy && !has_local_changes,
                            cx.listener(move |model, _, _, cx| {
                                model.request_pull_request_action(
                                    project,
                                    GitPullRequestAction::UpdateBranch {
                                        number,
                                        expected_head: head_oid.clone(),
                                    },
                                    cx,
                                );
                            }),
                        ))
                    },
                )
                .child(merge_selector(popover, style, !busy, cx))
                .when(
                    model
                        .state
                        .project(project)
                        .is_some_and(|project| project.parent_id.is_some()),
                    |surface| surface.child(merge_removal(popover, style, !busy, cx)),
                )
                .child(action_button(
                    style,
                    "pr-merge",
                    "arrow.triangle.merge",
                    merge_action_label(merge_method).into(),
                    ActionTone::Primary,
                    can_merge(pr) && !busy,
                    cx.listener(move |model, _, _, cx| {
                        model.request_pull_request_action(
                            project,
                            GitPullRequestAction::Merge {
                                number,
                                method: merge_method,
                                delete_branch: false,
                                expected_head: Some(merge_head.clone()),
                            },
                            cx,
                        );
                    }),
                ))
                .child(action_button(
                    style,
                    "pr-close",
                    "xmark.circle",
                    "Close PR".into(),
                    ActionTone::Danger,
                    !busy,
                    cx.listener(move |model, _, _, cx| {
                        model.request_pull_request_action(
                            project,
                            GitPullRequestAction::Close { number },
                            cx,
                        );
                    }),
                ))
        })
        .into_any_element()
}
