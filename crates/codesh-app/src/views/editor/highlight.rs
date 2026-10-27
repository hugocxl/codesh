use codesh_ui::theme::Theme;
use gpui::{Hsla, Rgba};
use std::sync::OnceLock;
use syntect::{
    easy::HighlightLines,
    highlighting::{
        Color, HighlightState, StyleModifier, Theme as SyntaxTheme, ThemeItem, ThemeSettings,
    },
    parsing::{ParseState, SyntaxSet},
};

fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

#[derive(Clone, Debug)]
struct Line {
    tokens: Vec<(usize, Hsla)>,
    parser: ParseState,
    highlighter: HighlightState,
}

#[derive(Debug)]
pub(super) struct Highlights {
    syntax: usize,
    theme: SyntaxTheme,
    lines: Vec<Line>,
    next: usize,
    through: usize,
    count: usize,
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Theme channels are clamped to an eight-bit color."
)]
fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn color(value: Hsla) -> Color {
    let c: Rgba = value.into();
    Color {
        r: channel(c.r),
        g: channel(c.g),
        b: channel(c.b),
        a: channel(c.a),
    }
}

impl Highlights {
    pub(super) fn new(path: &str, theme: &Theme) -> Self {
        let set = syntaxes();
        let syntax = std::path::Path::new(path)
            .extension()
            .and_then(|s| s.to_str())
            .and_then(|ext| set.find_syntax_by_extension(ext))
            .unwrap_or_else(|| set.find_syntax_plain_text());
        let index = set
            .syntaxes()
            .iter()
            .position(|item| std::ptr::eq(item, syntax))
            .unwrap_or(0);
        Self {
            syntax: index,
            theme: syntax_theme(theme),
            lines: Vec::new(),
            next: 0,
            through: 0,
            count: 0,
        }
    }
    pub(super) fn set_theme(&mut self, theme: &Theme) {
        self.theme = syntax_theme(theme);
        self.lines.clear();
        self.next = 0;
        self.through = 0;
    }
    pub(super) fn invalidate(&mut self, first: usize, through: usize, count: usize) {
        // Changed line counts shift parser checkpoints; retain only the valid prefix.
        if count != self.count {
            self.lines.truncate(first);
        }
        self.count = count;
        self.next = self.next.min(first);
        self.through = self.through.max(through);
    }
    pub(super) fn prepare(&mut self, lines: &[String], end: usize, budget: usize) -> bool {
        let set = syntaxes();
        self.count = lines.len();
        let end = end.min(lines.len());
        let mut processed = 0;
        while self.next < end && processed < budget {
            let row = self.next;
            let mut highlighter = if row == 0 {
                HighlightLines::new(&set.syntaxes()[self.syntax], &self.theme)
            } else {
                let previous = &self.lines[row - 1];
                HighlightLines::from_state(
                    &self.theme,
                    previous.highlighter.clone(),
                    previous.parser.clone(),
                )
            };
            let input = format!("{}\n", lines[row]);
            let mut consumed = 0;
            let tokens = highlighter
                .highlight_line(&input, set)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(style, text)| {
                    let len = text.len().min(lines[row].len().saturating_sub(consumed));
                    consumed += text.len();
                    if len == 0 {
                        return None;
                    }
                    Some((
                        len,
                        gpui::rgba(u32::from_be_bytes([
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                            style.foreground.a,
                        ]))
                        .into(),
                    ))
                })
                .collect();
            let (highlighter, parser) = highlighter.state();
            let stable = row >= self.through
                && self
                    .lines
                    .get(row)
                    .is_some_and(|old| old.parser == parser && old.highlighter == highlighter);
            let entry = Line {
                tokens,
                parser,
                highlighter,
            };
            if row < self.lines.len() {
                self.lines[row] = entry;
            } else {
                self.lines.push(entry);
            }
            self.next += 1;
            processed += 1;
            if stable {
                self.next = self.lines.len();
                self.through = 0;
            }
        }
        if self.next == self.lines.len() {
            self.through = 0;
        }
        self.next < end
    }
    pub(super) fn tokens(&self, row: usize) -> Option<&[(usize, Hsla)]> {
        self.lines
            .get(row)
            .filter(|_| row < self.next)
            .map(|line| line.tokens.as_slice())
    }
}

fn syntax_theme(theme: &Theme) -> SyntaxTheme {
    let mut result = SyntaxTheme {
        settings: ThemeSettings {
            foreground: Some(color(theme.fg)),
            background: Some(color(theme.bg)),
            ..ThemeSettings::default()
        },
        ..SyntaxTheme::default()
    };
    for (scope, value) in [
        ("comment", theme.fg_muted),
        ("string", theme.diff_add),
        ("constant", theme.warning),
        ("keyword", theme.accent),
        ("storage", theme.accent),
        ("entity.name.function", theme.diff_hunk),
        ("entity.name.type", theme.warning),
        ("invalid", theme.danger),
    ] {
        if let Ok(scope) = scope.parse() {
            result.scopes.push(ThemeItem {
                scope,
                style: StyleModifier {
                    foreground: Some(color(value)),
                    ..StyleModifier::default()
                },
            });
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_large_file_highlighting_is_lazy_and_reuses_a_stable_suffix() {
        let theme = Theme::from_scheme(&codesh_ui::theme::ColorScheme::default());
        let mut cache = Highlights::new("test.rs", &theme);
        let mut lines = vec!["let value = 1;".to_owned(); 10_000];
        assert!(cache.prepare(&lines, 500, 128));
        assert_eq!(cache.lines.len(), 128);
        lines[0] = "let renamed = 2;".to_owned();
        cache.invalidate(0, 0, lines.len());
        cache.prepare(&lines, 128, 1);
        assert_eq!(cache.next, 128);
        lines.insert(0, "/*".to_owned());
        cache.invalidate(0, 0, lines.len());
        assert!(cache.lines.is_empty());
        cache.prepare(&lines, 20, 20);
        let expected: usize = lines[0].len();
        assert_eq!(
            cache
                .tokens(0)
                .map(|tokens| tokens.iter().map(|(len, _)| len).sum::<usize>()),
            Some(expected)
        );
    }
    #[test]
    fn editor_highlight_invalidation_propagates_multiline_state_with_a_budget() {
        let theme = Theme::from_scheme(&codesh_ui::theme::ColorScheme::default());
        let mut cache = Highlights::new("test.rs", &theme);
        let mut lines: Vec<String> = ["/*", "inside", "*/", "fn main() {}"]
            .map(str::to_owned)
            .into();
        assert!(cache.prepare(&lines, 4, 1));
        assert!(cache.tokens(1).is_none());
        assert!(!cache.prepare(&lines, 4, 10));
        let old = cache.lines[1].parser.clone();
        lines[0] = "//".to_owned();
        cache.invalidate(0, 0, lines.len());
        cache.prepare(&lines, 4, 10);
        assert_ne!(old, cache.lines[1].parser);
        assert_eq!(cache.next, 4);
    }
}
