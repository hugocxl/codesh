//! Native file picker. The host supplies project-relative paths and handles events.

use std::ops::Range;

use codesh_ui::{
    components::IconGlyph,
    icon::Icon,
    picker::{Confirm, Dismiss, SelectNext, SelectPrevious},
    text_input::{InputEvent, InputStyle, TextInput},
    theme::{Metrics, Theme},
};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, HighlightStyle, ListAlignment,
    ListState, Render, StyledText, Subscription, Window, div, prelude::*, px,
};

#[derive(Clone, Debug)]
pub(crate) enum QuickOpenEvent {
    Open { path: String },
    Dismissed,
}

#[derive(Debug)]
pub(crate) struct QuickOpen {
    input: Entity<TextInput>,
    files: Vec<String>,
    matches: Vec<FileMatch>,
    selected: usize,
    scroll: ListState,
    theme: Theme,
    metrics: Metrics,
    _subscription: Subscription,
}

#[derive(Debug)]
struct FileMatch {
    index: usize,
    score: i64,
    ranges: Vec<Range<usize>>,
}

// Try each possible first character so an early directory match cannot hide a
// stronger consecutive match in the basename. Offsets always refer to UTF-8 bytes.
fn fuzzy(path: &str, query: &[char]) -> Option<(i64, Vec<Range<usize>>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    let chars: Vec<_> = path.char_indices().collect();
    let basename = path.rfind('/').map_or(0, |index| index + 1);
    let mut best = None;
    for start in 0..chars.len() {
        let mut matched = 0;
        let mut score = 0;
        let mut ranges = Vec::new();
        let mut previous = None;
        for (index, &(offset, ch)) in chars.iter().enumerate().skip(start) {
            if ch.to_lowercase().next() == Some(query[matched]) {
                score += 10 + if offset >= basename { 20 } else { 0 };
                if index == 0
                    || !chars[index - 1].1.is_alphanumeric()
                    || (chars[index - 1].1.is_lowercase() && ch.is_uppercase())
                {
                    score += 15;
                }
                if previous == Some(index.saturating_sub(1)) {
                    score += 25;
                }
                previous = Some(index);
                ranges.push(offset..offset + ch.len_utf8());
                matched += 1;
                if matched == query.len() {
                    if best.as_ref().is_none_or(|(old, _)| score > *old) {
                        best = Some((score, ranges));
                    }
                    break;
                }
            } else if index == start {
                break;
            }
        }
    }
    best
}

fn rank(files: &[String], query: &str) -> Vec<FileMatch> {
    let query: Vec<_> = query
        .chars()
        .map(|ch| ch.to_lowercase().next().unwrap_or(ch))
        .collect();
    let mut matches: Vec<FileMatch> = Vec::with_capacity(101);
    for (index, path) in files.iter().enumerate() {
        let Some((score, ranges)) = fuzzy(path, &query) else {
            continue;
        };
        let candidate = FileMatch {
            index,
            score,
            ranges,
        };
        let position = matches.partition_point(|existing| {
            existing.score > candidate.score
                || (existing.score == candidate.score
                    && (files[existing.index].len(), &files[existing.index]) <= (path.len(), path))
        });
        if position < 100 {
            matches.insert(position, candidate);
            matches.truncate(100);
        }
    }
    matches
}

pub(crate) fn highlighted(text: String, ranges: Vec<Range<usize>>, theme: &Theme) -> StyledText {
    StyledText::new(text).with_highlights(ranges.into_iter().map(|range| {
        (
            range,
            HighlightStyle {
                color: Some(theme.accent),
                font_weight: Some(gpui::FontWeight::BOLD),
                ..Default::default()
            },
        )
    }))
}

impl QuickOpen {
    pub(crate) fn new(theme: &Theme, metrics: Metrics, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(InputStyle::compact(theme, &metrics), cx)
                .with_placeholder("Go to file…")
                .with_key_context("Picker")
        });
        let subscription = cx.subscribe(&input, |this: &mut Self, _, event, cx| match event {
            InputEvent::Changed => this.refresh(cx),
            InputEvent::Submitted => this.open(cx),
            InputEvent::Cancelled => cx.emit(QuickOpenEvent::Dismissed),
        });
        Self {
            input,
            files: Vec::new(),
            matches: Vec::new(),
            selected: 0,
            scroll: ListState::new(0, ListAlignment::Top, px(32.0)),
            theme: theme.clone(),
            metrics,
            _subscription: subscription,
        }
    }

    pub(crate) fn set_files(&mut self, mut files: Vec<String>, cx: &mut Context<Self>) {
        files.sort_unstable();
        files.dedup();
        self.files = files;
        self.refresh(cx);
    }

    pub(crate) fn set_theme(&mut self, theme: &Theme, cx: &mut Context<Self>) {
        self.theme = theme.clone();
        self.input.update(cx, |input, cx| {
            input.set_style(InputStyle::compact(theme, &self.metrics), cx);
        });
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.matches = rank(&self.files, self.input.read(cx).text());
        self.selected = 0;
        self.scroll.reset(self.matches.len());
        self.scroll.scroll_to_reveal_item(0);
        cx.notify();
    }

    fn open(&self, cx: &mut Context<Self>) {
        if let Some(found) = self.matches.get(self.selected) {
            cx.emit(QuickOpenEvent::Open {
                path: self.files[found.index].clone(),
            });
        }
    }

    fn step(&mut self, next: bool, cx: &mut Context<Self>) {
        self.selected = if next {
            (self.selected + 1).min(self.matches.len().saturating_sub(1))
        } else {
            self.selected.saturating_sub(1)
        };
        self.scroll.scroll_to_reveal_item(self.selected);
        cx.notify();
    }
}

impl QuickOpen {
    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let found = &self.matches[index];
        let path = &self.files[found.index];
        let base = path.rfind('/').map_or(0, |i| i + 1);
        let ranges = found
            .ranges
            .iter()
            .filter(|r| r.start >= base)
            .map(|r| r.start - base..r.end - base)
            .collect();
        div()
            .id(index)
            .h(self.metrics.scaled(32.0))
            .px(super::codesh::style::spacing())
            .flex()
            .items_center()
            .gap_2()
            .rounded_md()
            .bg(if self.selected == index {
                self.theme.hover
            } else {
                self.theme.bg
            })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected = index;
                this.open(cx);
            }))
            .child(IconGlyph::new(
                Icon::File,
                self.metrics.icon_md(),
                self.theme.fg_muted,
            ))
            .child(highlighted(path[base..].to_owned(), ranges, &self.theme))
            .child(
                div()
                    .text_color(self.theme.fg_muted)
                    .overflow_hidden()
                    .child(highlighted(
                        path[..base].to_owned(),
                        found
                            .ranges
                            .iter()
                            .filter(|range| range.end <= base)
                            .cloned()
                            .collect(),
                        &self.theme,
                    )),
            )
            .into_any_element()
    }
}

impl EventEmitter<QuickOpenEvent> for QuickOpen {}
impl Focusable for QuickOpen {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for QuickOpen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let list = gpui::list(
            self.scroll.clone(),
            cx.processor(|this, index, _, cx| this.render_row(index, cx)),
        )
        .flex_grow()
        .min_h(px(0.0));
        div()
            .occlude()
            .absolute()
            .size_full()
            .top_0()
            .left_0()
            .flex()
            .justify_center()
            .pt(px(48.0))
            .bg(gpui::hsla(0.0, 0.0, 0.0, 0.3))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.emit(QuickOpenEvent::Dismissed)),
            )
            .child(
                div()
                    .id("quick-open")
                    .occlude()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .key_context("Picker")
                    .w(self.metrics.scaled(560.0))
                    .max_w_full()
                    .h(self.metrics.scaled(400.0))
                    .p(super::codesh::style::spacing())
                    .rounded(px(super::codesh::style::RADIUS))
                    .bg(self.theme.bg)
                    .text_color(self.theme.fg)
                    .text_size(self.metrics.font_body())
                    .flex()
                    .flex_col()
                    .gap_2()
                    .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.step(true, cx)))
                    .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.step(false, cx)))
                    .on_action(cx.listener(|this, _: &Confirm, _, cx| this.open(cx)))
                    .on_action(
                        cx.listener(|_, _: &Dismiss, _, cx| cx.emit(QuickOpenEvent::Dismissed)),
                    )
                    .child(self.input.clone())
                    .child(list)
                    .when(self.matches.is_empty(), |el| {
                        el.child(
                            div()
                                .text_color(self.theme.fg_muted)
                                .child("No matching files"),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_open_ranks_basename_boundaries_and_consecutive_matches() {
        let files = [
            "src/main.rs",
            "src/map_index.rs",
            "main/resources/icon.svg",
            "src/domain.rs",
            "README.md",
        ]
        .map(str::to_owned);
        let ranked = rank(&files, "mai");
        assert_eq!(
            ranked
                .iter()
                .map(|m| files[m.index].as_str())
                .collect::<Vec<_>>(),
            [
                "src/main.rs",
                "src/map_index.rs",
                "src/domain.rs",
                "main/resources/icon.svg"
            ]
        );
        assert_eq!(rank(&files, "MAIN")[0].index, 0);
        let (_, ranges) = fuzzy("src/Éclair.rs", &['é', 'c']).unwrap();
        assert_eq!(ranges, [4..6, 6..7]);
        assert!(rank(&files, "xyz").is_empty());
        let many: Vec<_> = (0..1000).map(|i| format!("src/file_{i:04}.rs")).collect();
        let limited = rank(&many, "file");
        assert_eq!(limited.len(), 100);
        assert_eq!(limited.first().map(|m| m.index), Some(0));
        assert_eq!(limited.last().map(|m| m.index), Some(99));
    }
}
