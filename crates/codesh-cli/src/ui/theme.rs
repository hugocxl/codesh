//! Colors come from the host terminal's palette, so the interface follows its
//! theme, light or dark.

use codesh_app_core::activity::ActivityIndicator;
use ratatui::style::{Color, Modifier, Style};

pub(crate) const ACCENT: Color = Color::Blue;
/// Text drawn on an accent background.
const CONTRAST: Color = Color::Black;
/// Separators and the borders of unfocused panes.
const LINE: Color = Color::DarkGray;

pub(crate) fn text() -> Style {
    Style::new()
}

pub(crate) fn strong() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

pub(crate) fn muted() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

pub(crate) fn accent() -> Style {
    Style::new().fg(ACCENT)
}

pub(crate) fn key() -> Style {
    accent().add_modifier(Modifier::BOLD)
}

pub(crate) fn line() -> Style {
    Style::new().fg(LINE)
}

/// A highlighted label, such as the active tab or a mode name.
pub(crate) fn pill() -> Style {
    Style::new()
        .fg(CONTRAST)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn danger() -> Style {
    Style::new().fg(Color::Red)
}

pub(crate) fn danger_pill() -> Style {
    pill().bg(Color::Red)
}

pub(crate) fn resize_pill() -> Style {
    pill().bg(Color::Magenta)
}

pub(crate) fn button() -> Style {
    Style::new()
        .add_modifier(Modifier::REVERSED)
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn warning() -> Style {
    Style::new().fg(Color::Yellow)
}

pub(crate) fn connected(online: bool) -> Style {
    Style::new().fg(if online { Color::Green } else { Color::Yellow })
}

/// The dot that shows what the agents behind a pane, tab, or project are doing.
pub(crate) fn activity(indicator: ActivityIndicator) -> Option<(&'static str, Style)> {
    let (symbol, color) = match indicator {
        ActivityIndicator::None => return None,
        ActivityIndicator::Idle => ("○", Color::Green),
        ActivityIndicator::Completed => ("●", Color::Cyan),
        ActivityIndicator::Working => ("●", Color::Yellow),
        ActivityIndicator::Blocked => ("●", Color::Red),
    };
    Some((symbol, Style::new().fg(color)))
}

pub(crate) fn activity_label(indicator: ActivityIndicator) -> &'static str {
    match indicator {
        ActivityIndicator::None | ActivityIndicator::Idle => "idle",
        ActivityIndicator::Completed => "done",
        ActivityIndicator::Working => "working",
        ActivityIndicator::Blocked => "needs you",
    }
}
