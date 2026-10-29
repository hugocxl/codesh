//! Keyboard shortcut chords such as `cmd-shift-p` drawn as keycaps.

use gpui::{FontWeight, ParentElement, SharedString, Styled, div, prelude::FluentBuilder};

use super::layout;
use crate::theme::{Metrics, Theme};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Platform {
    Mac,
    Other,
}

const CURRENT: Platform = if cfg!(target_os = "macos") {
    Platform::Mac
} else {
    Platform::Other
};

#[derive(Clone, Copy)]
enum Modifier {
    Control,
    Alt,
    Shift,
    Platform,
    Function,
}

impl Modifier {
    const ORDER: [Self; 5] = [
        Self::Function,
        Self::Control,
        Self::Alt,
        Self::Shift,
        Self::Platform,
    ];

    fn parse(token: &str) -> Option<Self> {
        Some(match token {
            "ctrl" | "control" => Self::Control,
            "alt" | "option" | "opt" => Self::Alt,
            "shift" => Self::Shift,
            "cmd" | "super" | "win" | "platform" | "secondary" => Self::Platform,
            "fn" => Self::Function,
            _ => return None,
        })
    }

    fn label(self, platform: Platform) -> &'static str {
        match (self, platform) {
            (Self::Control, Platform::Mac) => "⌃",
            (Self::Alt, Platform::Mac) => "⌥",
            (Self::Shift, Platform::Mac) => "⇧",
            (Self::Platform, Platform::Mac) => "⌘",
            (Self::Control, Platform::Other) => "Ctrl",
            (Self::Alt, Platform::Other) => "Alt",
            (Self::Shift, Platform::Other) => "Shift",
            (Self::Platform, Platform::Other) => "Super",
            (Self::Function, _) => "fn",
        }
    }
}

/// The keycaps of a chord written the way key bindings are (`cmd-shift-p`,
/// `ctrl-k ctrl-s`), with modifiers in the platform's conventional order.
pub fn parse(chord: &str) -> Vec<SharedString> {
    parse_for(chord, CURRENT)
}

fn parse_for(chord: &str, platform: Platform) -> Vec<SharedString> {
    chord
        .split_whitespace()
        .flat_map(|keystroke| keystroke_caps(keystroke, platform))
        .collect()
}

fn keystroke_caps(keystroke: &str, platform: Platform) -> Vec<SharedString> {
    let (modifiers, key) = match keystroke.strip_suffix("--") {
        Some(modifiers) => (modifiers, "-"),
        None => keystroke
            .rsplit_once('-')
            .filter(|(_, key)| !key.is_empty())
            .unwrap_or(("", keystroke)),
    };
    let mut held = [false; Modifier::ORDER.len()];
    for token in modifiers.split('-').filter(|token| !token.is_empty()) {
        let Some(modifier) = Modifier::parse(&token.to_ascii_lowercase()) else {
            return vec![SharedString::from(keystroke.to_owned())];
        };
        held[modifier as usize] = true;
    }
    Modifier::ORDER
        .iter()
        .filter(|modifier| held[**modifier as usize])
        .map(|modifier| SharedString::from(modifier.label(platform)))
        .chain(std::iter::once(key_label(key)))
        .collect()
}

fn key_label(key: &str) -> SharedString {
    let named = match key.to_ascii_lowercase().as_str() {
        "enter" | "return" => "↩",
        "tab" => "⇥",
        "escape" | "esc" => "⎋",
        "backspace" => "⌫",
        "delete" => "⌦",
        "left" => "←",
        "right" => "→",
        "up" => "↑",
        "down" => "↓",
        "space" => "Space",
        "pageup" => "PgUp",
        "pagedown" => "PgDn",
        _ => "",
    };
    if !named.is_empty() {
        return named.into();
    }
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return SharedString::default();
    };
    first.to_uppercase().chain(chars).collect::<String>().into()
}

/// One keycap per entry of `caps`, muted, sized for list rows and the footer.
pub(super) fn keycaps(caps: &[SharedString], theme: &Theme, metrics: Metrics) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(metrics.spacing1())
        .children(caps.iter().map(|cap| keycap(cap.clone(), theme, metrics)))
}

pub(super) fn keycap(cap: SharedString, theme: &Theme, metrics: Metrics) -> gpui::Div {
    let wide = cap.chars().count() > 1;
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(metrics.scaled(layout::KEYCAP))
        .min_w(metrics.scaled(layout::KEYCAP))
        .when(wide, |cap| cap.px(metrics.spacing3()))
        .rounded(metrics.radius_sm())
        .bg(theme.fg_alpha(0.08))
        .text_size(metrics.font_footnote())
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.fg_muted)
        .child(cap)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(chord: &str, platform: Platform) -> Vec<String> {
        parse_for(chord, platform)
            .into_iter()
            .map(|cap| cap.to_string())
            .collect()
    }

    #[test]
    fn modifiers_follow_platform_order_and_names() {
        assert_eq!(caps("cmd-shift-p", Platform::Mac), ["⇧", "⌘", "P"]);
        assert_eq!(
            caps("cmd-shift-p", Platform::Other),
            ["Shift", "Super", "P"]
        );
        assert_eq!(caps("alt-ctrl-x", Platform::Mac), ["⌃", "⌥", "X"]);
        assert_eq!(
            caps("ctrl-shift-enter", Platform::Other),
            ["Ctrl", "Shift", "↩"]
        );
    }

    #[test]
    fn named_and_unknown_keys() {
        assert_eq!(caps("enter", Platform::Mac), ["↩"]);
        assert_eq!(caps("tab", Platform::Mac), ["⇥"]);
        assert_eq!(caps("escape", Platform::Mac), ["⎋"]);
        assert_eq!(caps("cmd-backspace", Platform::Mac), ["⌘", "⌫"]);
        assert_eq!(caps("alt-left", Platform::Mac), ["⌥", "←"]);
        assert_eq!(caps("f5", Platform::Mac), ["F5"]);
        assert_eq!(caps("cmd--", Platform::Mac), ["⌘", "-"]);
        assert_eq!(caps("cmd-k cmd-s", Platform::Mac), ["⌘", "K", "⌘", "S"]);
    }
}
