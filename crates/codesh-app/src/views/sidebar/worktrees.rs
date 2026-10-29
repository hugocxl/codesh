use codesh_app_core::{Project, ProjectId, ProjectStatus, Tab};
use codesh_core::shortcuts::ShortcutId;
use codesh_ui::{components::IconGlyph, icon::Icon, tr};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, px,
};

use crate::model::AppModel;
use crate::views::tab_sidebar::NavigationTarget;

impl AppModel {
    pub(crate) fn worktrees_visible(&self, id: ProjectId) -> bool {
        !self.appearance.hidden_worktrees.contains(&id)
    }

    pub(crate) fn toggle_worktree_visibility(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        if !self.appearance.hidden_worktrees.remove(&id) {
            self.appearance.hidden_worktrees.insert(id);
        }
        self.save_appearance(cx);
        if !self.worktrees_visible(id) {
            self.expanded_worktrees.remove(&id);
            if self.state.current_project().parent_id == Some(id) {
                self.select_project(id, cx);
            }
        }
        self.worktrees_toggled(id, cx);
        cx.notify();
    }

    pub(crate) fn has_worktrees(&self, project: &Project) -> bool {
        !project.home
            && project.parent_id.is_none()
            && self.worktrees_visible(project.id)
            && (self
                .state
                .projects()
                .iter()
                .any(|child| child.parent_id == Some(project.id))
                || self
                    .git
                    .projects
                    .get(&project.id)
                    .is_some_and(|repo| repo.summary.is_some()))
    }

    pub(crate) fn worktree_children(&self, parent: ProjectId) -> Vec<&Project> {
        self.state
            .projects()
            .iter()
            .filter(|project| project.parent_id == Some(parent))
            .collect()
    }

    pub(crate) fn preferred_worktree(&self, parent: ProjectId) -> ProjectId {
        if !self.worktrees_visible(parent) {
            return parent;
        }
        let active = self.state.current_project();
        if (active.id == parent || active.parent_id == Some(parent))
            && active.status() == ProjectStatus::Available
        {
            return active.id;
        }
        self.appearance
            .worktree_recent
            .iter()
            .copied()
            .find(|id| {
                self.state.project(*id).is_some_and(|project| {
                    (project.id == parent || project.parent_id == Some(parent))
                        && project.status() == ProjectStatus::Available
                })
            })
            .unwrap_or(parent)
    }

    pub(crate) fn new_worktree_target(&self) -> Option<ProjectId> {
        let current = self.state.current_project();
        let project = self
            .state
            .project(current.parent_id.unwrap_or(current.id))?;
        (!project.home
            && project.status() == ProjectStatus::Available
            && self.worktrees_visible(project.id)
            && !self.not_a_repository(project.id))
        .then_some(project.id)
    }
}

pub(super) fn group(
    project: &Project,
    index: usize,
    shortcut: Option<usize>,
    cards: &[NavigationTarget],
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let mut group = div()
        .flex()
        .flex_col()
        .child(super::project_row(project, index, shortcut, model, cx));
    if model.appearance.sidebar_expanded
        && model.has_worktrees(project)
        && project.status() == ProjectStatus::Available
    {
        let mut worktrees = div()
            .flex()
            .flex_col()
            .gap(model.metrics.scaled(2.0))
            .pb(model.metrics.spacing2())
            .child(row(project, true, cards, model, cx));
        for child in model.worktree_children(project.id) {
            worktrees = worktrees.child(row(child, false, cards, model, cx));
        }
        for (index, creation) in model.worktree_creations(project.id).enumerate() {
            worktrees = worktrees.child(creating_row(&creation.name, project.id, index, model));
        }
        group = group.child(worktrees);
    }
    if model.appearance.sidebar_expanded && !model.has_worktrees(project) {
        group = group.child(row(project, true, cards, model, cx));
    }
    group.into_any_element()
}

pub(in crate::views) fn terminal(
    project: &Project,
    tab: &Tab,
    cards: &[NavigationTarget],
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let number = card_number(cards, NavigationTarget::Tab(tab.id));
    activity_row(project, Some(tab), false, number, model, cx)
}

fn row(
    project: &Project,
    primary: bool,
    cards: &[NavigationTarget],
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let number = card_number(cards, NavigationTarget::Project(project.id));
    activity_row(project, None, primary, number, model, cx)
}

fn card_number(cards: &[NavigationTarget], card: NavigationTarget) -> Option<usize> {
    cards.iter().take(9).position(|target| *target == card)
}

fn activity_row(
    project: &Project,
    tab: Option<&Tab>,
    primary: bool,
    number: Option<usize>,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = project.id;
    let tab_id = tab.map(|tab| tab.id);
    let key = tab_id.map_or_else(|| format!("worktree-{id}"), |tab| format!("terminal-{tab}"));
    let m = model.metrics;
    let theme = &model.theme;
    let active = model.state.current_project().id == id
        && tab_id.is_none_or(|tab| model.active_tab() == Some(tab));
    let missing = project.status() == ProjectStatus::Missing;
    let unavailable = missing || model.worktree_removing(id);
    let activity = tab.map_or_else(
        || super::activity_card::Data::new(project, model),
        |tab| super::activity_card::Data::for_tab(project, tab, model),
    );
    div()
        .id(SharedString::from(key.clone()))
        .debug_selector(move || key.clone())
        .flex()
        .flex_col()
        .px(m.scaled(10.0))
        .py(m.scaled(6.0))
        .rounded(m.scaled(8.0))
        .border_1()
        .border_color(gpui::transparent_black())
        .text_size(m.font_body())
        .text_color(theme.fg_muted)
        .when(active, |row| {
            row.bg(theme.surface)
                .border_color(super::super::codesh::style::border(theme))
        })
        .when(unavailable, |row| row.opacity(0.5))
        .hover(|style| style.bg(theme.hover))
        .when(!unavailable, |row| {
            row.cursor_pointer()
                .on_click(cx.listener(move |model, _, window, cx| {
                    if let Some(tab) = tab_id {
                        model.select_tab(tab, cx);
                    } else {
                        model.select_project(id, cx);
                    }
                    model.focus_active(window, cx);
                }))
        })
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                if let Some(project) = model.state.project(id) {
                    model.open_menu(
                        if let Some(tab) = tab_id {
                            vec![super::super::menu::Item::action(
                                tr!("Close Terminal"),
                                super::super::menu::Command::Tab(
                                    tab,
                                    super::super::tab_menu::Action::Close,
                                ),
                            )]
                        } else if project.parent_id.is_none() && !primary {
                            model.project_menu(project)
                        } else {
                            super::super::project_menu::worktree_items(project, primary)
                        },
                        event.position,
                        window,
                        cx,
                    );
                }
            }),
        )
        .child(card_content(project, tab, primary, number, activity, model))
        .into_any_element()
}

fn card_content(
    project: &Project,
    tab: Option<&Tab>,
    primary: bool,
    number: Option<usize>,
    activity: super::activity_card::Data,
    model: &AppModel,
) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .flex()
        .flex_col()
        .gap(m.scaled(3.0))
        .min_w(px(0.0))
        .child(headline(
            activity.headline,
            activity.provider,
            number,
            model,
        ))
        .child(
            div()
                .flex()
                .items_center()
                .gap(m.scaled(6.0))
                .text_size(m.scaled(11.0))
                .text_color(theme.fg_muted)
                .child(
                    div()
                        .flex_shrink()
                        .min_w(px(0.0))
                        .truncate()
                        .child(if tab.is_some() {
                            activity
                                .directory
                                .unwrap_or_else(|| tr!("Terminal").to_string())
                        } else {
                            project.name.clone()
                        }),
                )
                .children(activity.branch.map(|branch| {
                    div()
                        .flex_shrink()
                        .min_w(px(0.0))
                        .truncate()
                        .text_color(theme.fg_dim)
                        .child(branch)
                }))
                .when(primary, |line| {
                    line.child(
                        div()
                            .flex_none()
                            .px(m.scaled(6.0))
                            .rounded_full()
                            .bg(theme.surface)
                            .text_size(m.font_micro())
                            .font_weight(FontWeight::BOLD)
                            .child(tr!("PRIMARY")),
                    )
                })
                .child(div().flex_1())
                .when_some(activity.pr, |line, number| {
                    line.child(
                        div()
                            .flex_none()
                            .text_color(theme.accent)
                            .child(format!("#{number}")),
                    )
                })
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(m.icon_sm())
                        .child(super::super::tab_activity::worktree_dot(
                            project.id,
                            tab,
                            activity.working,
                            model,
                        )),
                ),
        )
}

/// The card's top line: the running command, the agent and its task, or a
/// quiet shell chip when nothing runs.
fn headline(
    headline: super::activity_card::Headline,
    provider: Option<codesh_protocol::AgentProvider>,
    number: Option<usize>,
    model: &AppModel,
) -> AnyElement {
    use super::activity_card::Headline;
    let m = model.metrics;
    let theme = &model.theme;
    let line = div()
        .flex()
        .items_center()
        .gap(m.scaled(6.0))
        .min_w(px(0.0))
        .text_size(m.scaled(13.0));
    let hint = number.and_then(|index| {
        model
            .shortcut_hints
            .label(ShortcutId::TABS[index], &model.settings.keymap)
    });
    let lead = |icon: AnyElement| match hint.clone() {
        Some(label) => super::super::shortcut_hints::badge(label, m.scaled(18.0), model),
        None => icon,
    };
    let chip = |icon: Icon, color: gpui::Hsla| {
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(m.scaled(18.0))
            .rounded(m.scaled(5.0))
            .bg(theme.surface)
            .border_1()
            .border_color(super::super::codesh::style::border(theme))
            .child(IconGlyph::new(icon, m.scaled(11.0), color))
            .into_any_element()
    };
    let text = |content: String, color: gpui::Hsla, weight: FontWeight| {
        div()
            .flex_1()
            .min_w(px(0.0))
            .truncate()
            .text_color(color)
            .font_weight(weight)
            .child(content)
    };
    match headline {
        Headline::Command(command) => line
            .child(lead(chip(Icon::Terminal, theme.fg)))
            .child(text(command, theme.fg, FontWeight::SEMIBOLD))
            .into_any_element(),
        Headline::Editor(file) => line
            .child(lead(chip(Icon::File, theme.fg)))
            .child(text(file, theme.fg, FontWeight::SEMIBOLD))
            .into_any_element(),
        Headline::Diff(file) => line
            .child(lead(chip(Icon::GitBranch, theme.fg)))
            .child(text(file, theme.fg, FontWeight::SEMIBOLD))
            .into_any_element(),
        Headline::Browser(page) => line
            .child(lead(chip(Icon::Globe, theme.fg)))
            .child(text(page, theme.fg, FontWeight::SEMIBOLD))
            .into_any_element(),
        Headline::Agent { task } => line
            .child(lead(provider.map_or_else(
                || chip(Icon::Terminal, theme.fg),
                |provider| {
                    super::super::tab_activity::provider_icon(provider, m.scaled(18.0), model)
                },
            )))
            .child(match task {
                Some(task) => text(task, theme.fg, FontWeight::MEDIUM),
                None => text(
                    provider
                        .map_or("Agent", codesh_protocol::AgentProvider::name)
                        .to_owned(),
                    theme.fg_muted,
                    FontWeight::NORMAL,
                ),
            })
            .into_any_element(),
        Headline::Shell { shell, directory } => line
            .child(lead(chip(Icon::Terminal, theme.diff_add)))
            .child(div().flex_none().text_color(theme.fg_muted).child(shell))
            .when_some(directory, |line, directory| {
                line.child(text(directory, theme.fg_dim, FontWeight::NORMAL))
            })
            .into_any_element(),
        Headline::Empty => line
            .child(lead(chip(Icon::Terminal, theme.fg_dim)))
            .child(text(
                tr!("No terminal").to_string(),
                theme.fg_dim,
                FontWeight::NORMAL,
            ))
            .into_any_element(),
    }
}

/// A worktree the server is still creating.
fn creating_row(name: &str, parent: ProjectId, index: usize, model: &AppModel) -> AnyElement {
    let m = model.metrics;
    let key = format!("worktree-creating-{parent}-{index}");
    let selector = key.clone();
    div()
        .debug_selector(move || selector.clone())
        .flex()
        .items_center()
        .gap(m.spacing4())
        .px(m.spacing2())
        .h(m.scaled(28.0))
        .text_size(m.font_body())
        .text_color(model.theme.fg_muted)
        .child(
            div()
                .flex_none()
                .w(m.scaled(16.0))
                .flex()
                .items_center()
                .justify_center()
                .child(super::super::tab_activity::status_glyph(
                    key,
                    super::super::tab_activity::Status::Busy(super::super::tab_activity::CREATING),
                    m.icon_sm(),
                    model,
                )),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .child(name.to_owned()),
        )
        .into_any_element()
}
