use std::hash::{BuildHasher, RandomState};
use std::rc::Rc;

use codesh_ui::popover::PopoverAnchor;
use gpui::{Context, Window};

use super::AppModel;
use crate::views::overlays::Overlay;

/// Built-in sidebar tips, in the order the previous and next buttons follow.
/// Each one must describe current behavior and call customizable shortcuts defaults.
pub(crate) const TIPS: &[&str] = &[
    "Quitting Codesh never ends your terminals. To keep them when closing tabs or panes too, set Settings → General → Closing terminals → When closing tabs or panes to Detach.",
    "Right-click a terminal and choose Detach Terminal to put it away without ending it. By default, ⌘⌥T opens Existing Terminals to bring it back.",
    "By default, ⌘D splits right, ⌘⇧D splits down, ⌘⌥ plus an arrow moves between panes, and ⌘⇧↩ maximizes or restores the focused pane.",
    "By default, ⌘⇧P opens the command palette. Choose Switch Project… to jump to any project or worktree by name.",
    "Choose Switch Workspace… in the command palette to show one workspace in the sidebar. Click ⚙ next to a project → Workspaces to add it to one.",
    "In a Git project, click + next to the project name to create a worktree: a branch in its own folder with its own terminals.",
    "To pair a phone, turn on Settings → Mobile → Allow mobile devices and click Show Pairing Code, or run codesh mobile enable and then codesh mobile pair.",
    "Working over SSH? Run codesh on that machine for tabs and splits in the terminal, and press Ctrl-B then ? to see its keys.",
    "Codesh recognizes AI agents like Claude Code and Codex, marks panes, tabs, and projects when they're working or need you, and notifies you when they're in the background.",
    "In a Git project, click Commit in the status bar. Choose the AI CLI, whether to include unstaged changes, and whether to push. The CLI writes the message and Codesh commits.",
    "Click Server in the status bar to connect, restart, or stop the background server. Restarting or stopping ends every terminal on this Mac.",
    "By default, ⌘⌃← and ⌘⌃→ go back and forward through tabs you've visited, across projects.",
];

/// The tip on show. It starts at a random tip once per launch and never rotates by itself.
#[derive(Debug)]
pub(crate) struct Tips {
    index: usize,
    /// The collapsed sidebar's tip button, which the tip popover opens from.
    pub(crate) anchor: PopoverAnchor,
}

impl Default for Tips {
    fn default() -> Self {
        Self::starting_at(RandomState::new().hash_one(()))
    }
}

impl Tips {
    fn starting_at(seed: u64) -> Self {
        Self {
            index: usize::try_from(seed % TIPS.len() as u64).unwrap_or_default(),
            anchor: Rc::default(),
        }
    }

    pub(crate) fn current(&self) -> gpui::SharedString {
        TIPS[self.index].into()
    }

    /// The current tip's one-based position in [`TIPS`].
    pub(crate) fn position(&self) -> usize {
        self.index + 1
    }

    pub(crate) fn step(&mut self, forward: bool) {
        self.index = if forward {
            (self.index + 1) % TIPS.len()
        } else {
            (self.index + TIPS.len() - 1) % TIPS.len()
        };
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TipPlacement {
    /// A card at the bottom of the expanded sidebar.
    Card,
    /// A button at the bottom of the icon-only sidebar that opens the tip in a popover.
    Button,
}

impl AppModel {
    /// Where the built-in sidebar shows tips, or `None` when it shows none.
    pub(crate) fn tip_placement(&self) -> Option<TipPlacement> {
        if !self.appearance.tips_visible {
            None
        } else if self.appearance.sidebar_expanded {
            Some(TipPlacement::Card)
        } else if self.sidebar_width() > 0.0 {
            Some(TipPlacement::Button)
        } else {
            None
        }
    }

    pub(crate) fn step_tip(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.tips.step(forward);
        cx.notify();
    }

    pub(crate) fn toggle_tip_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        if matches!(self.overlay, Some(Overlay::Tip)) {
            self.dismiss_overlay(cx);
            return;
        }
        self.dismiss_overlay(cx);
        self.overlay = Some(Overlay::Tip);
        self.overlay_focus.focus(window);
        cx.notify();
    }

    pub(crate) fn confirm_hide_tips(&mut self, cx: &mut Context<Self>) {
        self.confirm(
            HIDE_TITLE,
            HIDE_MESSAGE.to_string(),
            HIDE_ACTION,
            cx,
            |model, cx| {
                model.appearance.tips_visible = false;
                model.save_appearance(cx);
            },
        );
    }
}

const HIDE_TITLE: &str = "Hide Tips?";
const HIDE_MESSAGE: &str =
    "You can show tips again in Settings → Appearance → Interface by turning on Show tips.";
pub(crate) const HIDE_ACTION: &str = "Hide Tips";
