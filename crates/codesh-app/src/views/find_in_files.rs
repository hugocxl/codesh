//! Search UI and ripgrep JSON decoding; transport belongs to the host.

use std::{collections::HashSet, time::Duration};

use codesh_ui::{
    picker::{Confirm, Dismiss, SelectNext, SelectPrevious},
    text_input::{InputEvent, InputStyle, TextInput},
    theme::{Metrics, Theme},
};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, ListAlignment, ListState, Render,
    Subscription, Task, Window, div, prelude::*, px,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FindMatch {
    pub(crate) path: String,
    pub(crate) line: u32,
    /// One-based byte column, as reported by ripgrep.
    pub(crate) column: u32,
    pub(crate) preview: String,
}

#[derive(Clone, Debug)]
pub(crate) enum FindEvent {
    Search {
        query: String,
        case_sensitive: bool,
        regex: bool,
    },
    Open {
        path: String,
        line: u32,
    },
    Dismissed,
}

// Ripgrep encodes non-UTF-8 paths and lines in a base64 `bytes` field.
fn field_bytes(value: &serde_json::Value) -> Option<Vec<u8>> {
    if let Some(text) = value.get("text").and_then(serde_json::Value::as_str) {
        return Some(text.as_bytes().to_vec());
    }
    let encoded = value.get("bytes")?.as_str()?;
    let mut output = Vec::with_capacity(encoded.len() * 3 / 4);
    let mut bits = 0_u32;
    let mut count = 0;
    for byte in encoded.bytes().take_while(|byte| *byte != b'=') {
        let digit = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(digit);
        count += 6;
        if count >= 8 {
            count -= 8;
            output.push(u8::try_from((bits >> count) & 255).ok()?);
        }
    }
    Some(output)
}

pub(crate) fn parse_rg_json(output: &str) -> Vec<FindMatch> {
    let mut matches = Vec::new();
    for record in output.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(record) else {
            continue;
        };
        if value.get("type").and_then(serde_json::Value::as_str) != Some("match") {
            continue;
        }
        let Some(data) = value.get("data") else {
            continue;
        };
        let (Some(path), Some(lines), Some(line), Some(submatches)) = (
            data.get("path").and_then(field_bytes),
            data.get("lines").and_then(field_bytes),
            data.get("line_number")
                .and_then(serde_json::Value::as_u64)
                .and_then(|n| u32::try_from(n).ok()),
            data.get("submatches").and_then(serde_json::Value::as_array),
        ) else {
            continue;
        };
        let mut line_starts = vec![0];
        line_starts.extend(
            lines
                .iter()
                .enumerate()
                .filter_map(|(index, &byte)| (byte == b'\n').then_some(index + 1)),
        );
        for submatch in submatches {
            let Some(start) = submatch
                .get("start")
                .and_then(serde_json::Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
                .filter(|&n| n <= lines.len())
            else {
                continue;
            };
            let extra_lines = line_starts.partition_point(|&offset| offset <= start) - 1;
            let line_start = line_starts[extra_lines];
            let line_end = line_starts
                .get(extra_lines + 1)
                .map_or(lines.len(), |n| n - 1);
            let (Some(line), Some(column)) = (
                u32::try_from(extra_lines)
                    .ok()
                    .and_then(|n| line.checked_add(n)),
                u32::try_from(start - line_start)
                    .ok()
                    .and_then(|n| n.checked_add(1)),
            ) else {
                continue;
            };
            let preview_bytes = &lines[line_start..line_end];
            let preview_bytes = preview_bytes.strip_suffix(b"\r").unwrap_or(preview_bytes);
            matches.push(FindMatch {
                path: String::from_utf8_lossy(&path).into_owned(),
                line,
                column,
                preview: String::from_utf8_lossy(preview_bytes).into_owned(),
            });
        }
    }
    matches
}

#[derive(Clone, Debug)]
enum Row {
    File(String),
    Match(usize),
}

#[derive(Debug)]
pub(crate) struct FindInFiles {
    input: Entity<TextInput>,
    results: Vec<FindMatch>,
    rows: Vec<Row>,
    collapsed: HashSet<String>,
    selected: usize,
    case_sensitive: bool,
    regex: bool,
    pending: Option<Task<()>>,
    scroll: ListState,
    theme: Theme,
    metrics: Metrics,
    _subscription: Subscription,
}

impl FindInFiles {
    pub(crate) fn new(theme: &Theme, metrics: Metrics, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(InputStyle::compact(theme, &metrics), cx)
                .with_placeholder("Find in files…")
                .with_key_context("Picker")
        });
        let subscription = cx.subscribe(&input, |this: &mut Self, _, event, cx| match event {
            InputEvent::Changed => this.search(cx),
            InputEvent::Submitted => this.open(cx),
            InputEvent::Cancelled => cx.emit(FindEvent::Dismissed),
        });
        Self {
            input,
            results: Vec::new(),
            rows: Vec::new(),
            collapsed: HashSet::new(),
            selected: 0,
            case_sensitive: false,
            regex: false,
            pending: None,
            scroll: ListState::new(0, ListAlignment::Top, px(32.0)),
            theme: theme.clone(),
            metrics,
            _subscription: subscription,
        }
    }

    pub(crate) fn set_results(&mut self, mut results: Vec<FindMatch>, cx: &mut Context<Self>) {
        results.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then(a.line.cmp(&b.line))
                .then(a.column.cmp(&b.column))
        });
        self.results = results;
        self.rebuild(cx);
    }

    pub(crate) fn set_theme(&mut self, theme: &Theme, cx: &mut Context<Self>) {
        self.theme = theme.clone();
        self.input.update(cx, |input, cx| {
            input.set_style(InputStyle::compact(theme, &self.metrics), cx);
        });
        cx.notify();
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        // Dropping the previous task cancels its timer and prevents stale queries.
        self.pending = Some(cx.spawn(async |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let _ = this.update(cx, |this, cx| {
                cx.emit(FindEvent::Search {
                    query: this.input.read(cx).text().to_owned(),
                    case_sensitive: this.case_sensitive,
                    regex: this.regex,
                });
            });
        }));
        self.results.clear();
        self.rebuild(cx);
    }

    fn rebuild(&mut self, cx: &mut Context<Self>) {
        self.rows.clear();
        let mut previous = None;
        for (index, result) in self.results.iter().enumerate() {
            if previous != Some(&result.path) {
                self.rows.push(Row::File(result.path.clone()));
                previous = Some(&result.path);
            }
            if !self.collapsed.contains(&result.path) {
                self.rows.push(Row::Match(index));
            }
        }
        self.selected = self
            .rows
            .iter()
            .position(|r| matches!(r, Row::Match(_)))
            .unwrap_or(0);
        self.scroll.reset(self.rows.len());
        cx.notify();
    }

    fn open(&mut self, cx: &mut Context<Self>) {
        match self.rows.get(self.selected) {
            Some(Row::Match(index)) => {
                let result = &self.results[*index];
                cx.emit(FindEvent::Open {
                    path: result.path.clone(),
                    line: result.line,
                });
            }
            Some(Row::File(path)) => {
                let path = path.clone();
                if !self.collapsed.remove(&path) {
                    self.collapsed.insert(path);
                }
                let selected = self.selected;
                self.rebuild(cx);
                self.selected = selected.min(self.rows.len().saturating_sub(1));
            }
            None => {}
        }
    }

    fn step(&mut self, next: bool, cx: &mut Context<Self>) {
        self.selected = if next {
            (self.selected + 1).min(self.rows.len().saturating_sub(1))
        } else {
            self.selected.saturating_sub(1)
        };
        self.scroll.scroll_to_reveal_item(self.selected);
        cx.notify();
    }
}

impl FindInFiles {
    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let row = div()
            .id(index)
            .h(self.metrics.scaled(32.0))
            .px(super::codesh::style::spacing())
            .flex()
            .items_center()
            .gap_2()
            .overflow_hidden()
            .cursor_pointer()
            .bg(if self.selected == index {
                self.theme.hover
            } else {
                self.theme.bg
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected = index;
                this.open(cx);
            }));
        match &self.rows[index] {
            Row::File(path) => row
                .child(format!(
                    "{} {path}",
                    if self.collapsed.contains(path) {
                        "▸"
                    } else {
                        "▾"
                    }
                ))
                .into_any_element(),
            Row::Match(i) => {
                let result = &self.results[*i];
                let start = usize::try_from(result.column.saturating_sub(1)).unwrap_or(0);
                let ranges =
                    if result.preview.is_char_boundary(start) && start < result.preview.len() {
                        let length = if self.regex {
                            1
                        } else {
                            self.input.read(cx).text().chars().count().max(1)
                        };
                        let end = result.preview[start..]
                            .char_indices()
                            .nth(length)
                            .map_or(result.preview.len(), |(n, _)| start + n);
                        std::iter::once(start..end).collect()
                    } else {
                        Vec::new()
                    };
                row.child(
                    div()
                        .text_color(self.theme.fg_muted)
                        .child(format!("{}:{}", result.line, result.column)),
                )
                .child(super::quick_open::highlighted(
                    result.preview.clone(),
                    ranges,
                    &self.theme,
                ))
                .into_any_element()
            }
        }
    }
}

impl FindInFiles {
    fn render_toolbar(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_grow().child(self.input.clone()))
            .child(
                div()
                    .id("find-case")
                    .cursor_pointer()
                    .p_1()
                    .rounded_md()
                    .bg(if self.case_sensitive {
                        self.theme.hover
                    } else {
                        self.theme.bg
                    })
                    .child("Aa")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.case_sensitive = !this.case_sensitive;
                        this.search(cx);
                    })),
            )
            .child(
                div()
                    .id("find-regex")
                    .cursor_pointer()
                    .p_1()
                    .rounded_md()
                    .bg(if self.regex {
                        self.theme.hover
                    } else {
                        self.theme.bg
                    })
                    .child(".*")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.regex = !this.regex;
                        this.search(cx);
                    })),
            )
    }
}

impl EventEmitter<FindEvent> for FindInFiles {}
impl Focusable for FindInFiles {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for FindInFiles {
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
                cx.listener(|_, _, _, cx| cx.emit(FindEvent::Dismissed)),
            )
            .child(
                div()
                    .id("find-in-files")
                    .occlude()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .key_context("Picker")
                    .w(self.metrics.scaled(720.0))
                    .max_w_full()
                    .h(self.metrics.scaled(480.0))
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
                    .on_action(cx.listener(|_, _: &Dismiss, _, cx| cx.emit(FindEvent::Dismissed)))
                    .child(self.render_toolbar(cx))
                    .child(list)
                    .when(self.results.is_empty(), |el| {
                        el.child(div().text_color(self.theme.fg_muted).child("No results"))
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_in_files_parses_multiple_submatches_and_multiline_offsets() {
        let output = concat!(
            "{\"type\":\"begin\"}\n",
            "garbage\n",
            r#"{"type":"match","data":{"path":{"text":"src/a.rs"},"lines":{"text":"foo foo\nbar foo\r\n"},"line_number":7,"submatches":[{"start":0},{"start":4},{"start":12},{"start":900}]}}"#
        );
        let found = parse_rg_json(output);
        assert_eq!(
            found
                .iter()
                .map(|m| (m.line, m.column, m.preview.as_str()))
                .collect::<Vec<_>>(),
            [(7, 1, "foo foo"), (7, 5, "foo foo"), (8, 5, "bar foo")]
        );
        assert!(found.iter().all(|m| m.path == "src/a.rs"));
    }

    #[test]
    fn find_in_files_tolerates_binary_fields_and_incomplete_records() {
        let found = parse_rg_json(
            r#"{"type":"match","data":{"path":{"bytes":"Yf8="},"lines":{"bytes":"/2ZvbyA="},"line_number":1,"submatches":[{"start":1}]}}
{"type":"match","data":{}}
{"type":"summary"}"#,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, "a�");
        assert_eq!(found[0].preview, "�foo ");
    }
}
