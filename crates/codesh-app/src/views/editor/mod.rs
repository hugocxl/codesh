mod buffer;
mod element;
mod highlight;

use buffer::{Buffer, Position};
use codesh_ui::theme::Theme;
use element::{EditorElement, Layout};
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollWheelEvent, Styled,
    UTF16Selection, Window, div, point, px,
};
use highlight::Highlights;
use std::ops::Range;

struct KeyBindingsInstalled;
impl gpui::Global for KeyBindingsInstalled {}

fn platform_key(key: &str) -> String {
    if cfg!(target_os = "macos") {
        key.to_owned()
    } else {
        key.replace("cmd-", "ctrl-").replace("alt-", "ctrl-")
    }
}

macro_rules! editor_keys {
    ($($action:ident => $key:literal),* $(,)?) => {
        gpui::actions!(codesh_editor, [$($action),*]);
        fn bind_keys(cx: &mut App) {
            if !cx.has_global::<KeyBindingsInstalled>() {
                cx.set_global(KeyBindingsInstalled);
                cx.bind_keys([$(gpui::KeyBinding::new(&platform_key($key), $action, Some("CodeshEditor"))),*]);
            }
        }
        impl EditorView {
            fn key_listeners(container: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
                container$(.on_action(cx.listener(|editor, _: &$action, window, cx| {
                    if let Ok(keystroke) = gpui::Keystroke::parse(&platform_key($key)) {
                        editor.key_down(&KeyDownEvent { keystroke, is_held: false }, window, cx);
                    }
                })))*
            }
        }
    };
}
editor_keys! {
    Save => "cmd-s", SelectAll => "cmd-a", Copy => "cmd-c", Cut => "cmd-x", Paste => "cmd-v",
    Undo => "cmd-z", Redo => "cmd-shift-z",
    Left => "left", Right => "right", Up => "up", Down => "down",
    SelectLeft => "shift-left", SelectRight => "shift-right", SelectUp => "shift-up", SelectDown => "shift-down",
    WordLeft => "alt-left", WordRight => "alt-right", SelectWordLeft => "alt-shift-left", SelectWordRight => "alt-shift-right",
    LineStart => "cmd-left", LineEnd => "cmd-right", SelectLineStart => "cmd-shift-left", SelectLineEnd => "cmd-shift-right",
    DocumentStart => "cmd-up", DocumentEnd => "cmd-down", SelectDocumentStart => "cmd-shift-up", SelectDocumentEnd => "cmd-shift-down",
    Home => "home", End => "end", SelectHome => "shift-home", SelectEnd => "shift-end",
    DocumentHome => "ctrl-home", DocumentEndKey => "ctrl-end", SelectDocumentHome => "ctrl-shift-home", SelectDocumentEndKey => "ctrl-shift-end",
    PageUp => "pageup", PageDown => "pagedown", SelectPageUp => "shift-pageup", SelectPageDown => "shift-pagedown",
    Backspace => "backspace", Delete => "delete", DeleteWord => "alt-backspace", DeleteNextWord => "alt-delete",
    DeleteToStart => "cmd-backspace", Newline => "enter", Indent => "tab",
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum EditorEvent {
    Changed,
    SaveRequested,
}

pub(crate) struct EditorView {
    buffer: Buffer,
    highlights: Highlights,
    theme: Theme,
    terminal: codesh_app_core::settings::TerminalSettings,
    focus: FocusHandle,
    marked: Option<Range<Position>>,
    layout: Option<Layout>,
    scroll: Point<Pixels>,
    follow_caret: bool,
    selecting: bool,
    word_selecting: bool,
    drag_anchor: Range<Position>,
    goal_column: Option<usize>,
}

impl std::fmt::Debug for EditorView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorView")
            .field("lines", &self.buffer.lines.len())
            .field("dirty", &self.is_dirty())
            .finish_non_exhaustive()
    }
}

impl EventEmitter<EditorEvent> for EditorView {}
impl Focusable for EditorView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "The pane coordinator passes owned file content through the shared editor API."
)]
impl EditorView {
    pub(crate) fn new(
        text: String,
        path: &str,
        theme: &Theme,
        terminal: &codesh_app_core::settings::TerminalSettings,
        cx: &mut Context<Self>,
    ) -> Self {
        bind_keys(cx);
        Self {
            buffer: Buffer::new(&text),
            highlights: Highlights::new(path, theme),
            theme: theme.clone(),
            terminal: terminal.clone(),
            focus: cx.focus_handle(),
            marked: None,
            layout: None,
            scroll: point(px(0.0), px(0.0)),
            follow_caret: true,
            selecting: false,
            word_selecting: false,
            drag_anchor: Position::default()..Position::default(),
            goal_column: None,
        }
    }
    pub(crate) fn text(&self) -> String {
        self.buffer.text()
    }
    pub(crate) fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        let caret = self.buffer.caret();
        self.buffer = Buffer::new(&text);
        self.buffer.select(caret, false);
        self.marked = None;
        self.highlights.set_theme(&self.theme);
        self.follow_caret = true;
        cx.notify();
    }
    pub(crate) fn is_dirty(&self) -> bool {
        self.buffer.dirty()
    }
    pub(crate) fn mark_saved(&mut self, cx: &mut Context<Self>) {
        self.finish_composition();
        self.buffer.mark_saved();
        cx.notify();
    }
    pub(crate) fn set_terminal(
        &mut self,
        terminal: &codesh_app_core::settings::TerminalSettings,
        cx: &mut Context<Self>,
    ) {
        if self.terminal != *terminal {
            self.terminal = terminal.clone();
            self.layout = None;
            self.follow_caret = true;
            cx.notify();
        }
    }

    pub(crate) fn set_theme(&mut self, theme: &Theme, cx: &mut Context<Self>) {
        self.theme.clone_from(theme);
        self.highlights.set_theme(theme);
        cx.notify();
    }
    /// Puts the caret at one-based `line` and character `column`, scrolled
    /// into view.
    pub(crate) fn go_to_line(&mut self, line: u32, column: u32, cx: &mut Context<Self>) {
        self.finish_composition();
        let row = usize::try_from(line.saturating_sub(1)).unwrap_or(usize::MAX);
        let row = row.min(self.buffer.lines.len().saturating_sub(1));
        let skip = usize::try_from(column.saturating_sub(1)).unwrap_or(usize::MAX);
        let text = &self.buffer.lines[row];
        let column = text
            .char_indices()
            .nth(skip)
            .map_or(text.len(), |(index, _)| index);
        self.buffer.select(Position { row, column }, false);
        self.goal_column = None;
        self.follow_caret = true;
        cx.notify();
    }
    fn changed(&mut self, row: usize, cx: &mut Context<Self>) {
        self.highlights
            .invalidate(row, self.buffer.changed_through, self.buffer.lines.len());
        self.follow_caret = true;
        self.goal_column = None;
        cx.emit(EditorEvent::Changed);
        cx.notify();
    }
    fn finish_composition(&mut self) {
        self.buffer.finish_composition();
        self.marked = None;
    }
    fn insert(&mut self, text: &str, typing: bool, cx: &mut Context<Self>) {
        self.finish_composition();
        let row = self.buffer.insert(text, typing);
        self.changed(row, cx);
    }
    fn move_to(&mut self, target: Position, extend: bool, cx: &mut Context<Self>) {
        self.finish_composition();
        self.buffer.select(target, extend);
        self.follow_caret = true;
        self.goal_column = None;
        cx.notify();
    }
    fn vertical(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let p = self.buffer.caret();
        let goal = self
            .goal_column
            .unwrap_or_else(|| self.buffer.lines[p.row][..p.column].chars().count());
        let row = p
            .row
            .saturating_add_signed(delta)
            .min(self.buffer.lines.len() - 1);
        let column = self.buffer.lines[row]
            .char_indices()
            .nth(goal)
            .map_or(self.buffer.lines[row].len(), |(i, _)| i);
        self.move_to(Position { row, column }, extend, cx);
        self.goal_column = Some(goal);
    }
    fn copy(&self, cx: &mut Context<Self>) {
        if !self.buffer.range().is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.buffer.selected()));
        }
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Editor keyboard commands share selection and composition handling."
    )]
    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let m = key.modifiers;
        let primary = if cfg!(target_os = "macos") {
            m.platform
        } else {
            m.control
        };
        let word = m.alt || (!cfg!(target_os = "macos") && m.control);
        let p = self.buffer.caret();
        let range = self.buffer.range();
        let line_start = Position { column: 0, ..p };
        let line_end = Position {
            column: self.buffer.lines[p.row].len(),
            ..p
        };
        let mut handled = true;
        match key.key.as_str() {
            "s" if primary => {
                self.finish_composition();
                cx.emit(EditorEvent::SaveRequested);
            }
            "a" if primary => {
                self.move_to(Position::default(), false, cx);
                self.move_to(self.buffer.end(), true, cx);
            }
            "c" if primary => self.copy(cx),
            "x" if primary => {
                self.copy(cx);
                if !range.is_empty() {
                    self.insert("", false, cx);
                }
            }
            "v" if primary => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.insert(&text, false, cx);
                }
            }
            "z" if primary => {
                self.finish_composition();
                let row = if m.shift {
                    self.buffer.redo()
                } else {
                    self.buffer.undo()
                };
                if let Some(row) = row {
                    self.changed(row, cx);
                }
            }
            "y" if primary => {
                self.finish_composition();
                if let Some(row) = self.buffer.redo() {
                    self.changed(row, cx);
                }
            }
            "left" => {
                let target = if primary && cfg!(target_os = "macos") {
                    line_start
                } else if word {
                    self.buffer.word_left(p)
                } else if !m.shift && !range.is_empty() {
                    range.start
                } else {
                    self.buffer.previous(p)
                };
                self.move_to(target, m.shift, cx);
            }
            "right" => {
                let target = if primary && cfg!(target_os = "macos") {
                    line_end
                } else if word {
                    self.buffer.word_right(p)
                } else if !m.shift && !range.is_empty() {
                    range.end
                } else {
                    self.buffer.next(p)
                };
                self.move_to(target, m.shift, cx);
            }
            "home" => self.move_to(
                if primary || m.control {
                    Position::default()
                } else {
                    line_start
                },
                m.shift,
                cx,
            ),
            "end" => self.move_to(
                if primary || m.control {
                    self.buffer.end()
                } else {
                    line_end
                },
                m.shift,
                cx,
            ),
            "up" if primary => self.move_to(Position::default(), m.shift, cx),
            "down" if primary => self.move_to(self.buffer.end(), m.shift, cx),
            "up" => self.vertical(-1, m.shift, cx),
            "down" => self.vertical(1, m.shift, cx),
            "pageup" => self.vertical(-self.page_rows(), m.shift, cx),
            "pagedown" => self.vertical(self.page_rows(), m.shift, cx),
            "backspace" | "delete" => {
                self.finish_composition();
                if range.is_empty() {
                    let target = if key.key == "backspace" {
                        if primary && cfg!(target_os = "macos") {
                            line_start
                        } else if word {
                            self.buffer.word_left(p)
                        } else {
                            self.buffer.previous(p)
                        }
                    } else if word {
                        self.buffer.word_right(p)
                    } else {
                        self.buffer.next(p)
                    };
                    self.buffer.select(target, true);
                }
                if !self.buffer.range().is_empty() {
                    self.insert("", false, cx);
                }
            }
            "enter" => self.insert(&self.buffer.newline(), false, cx),
            "tab" => {
                let count = self.buffer.indent
                    - self.buffer.lines[p.row][..p.column].chars().count() % self.buffer.indent;
                self.insert(&" ".repeat(count), false, cx);
            }
            _ => handled = false,
        }
        if handled {
            cx.stop_propagation();
            window.prevent_default();
        }
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "A viewport height becomes a bounded row count."
    )]
    fn page_rows(&self) -> isize {
        self.layout.as_ref().map_or(1, |l| {
            (f32::from(l.bounds.size.height) / f32::from(l.line_height))
                .floor()
                .max(1.0) as isize
        })
    }
    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        let p = self.position(event.position);
        self.selecting = true;
        self.word_selecting = event.click_count == 2;
        if self.word_selecting {
            let range = self.buffer.word_range(p);
            self.move_to(range.start, false, cx);
            self.move_to(range.end, true, cx);
            self.drag_anchor = range;
        } else {
            self.move_to(p, event.modifiers.shift, cx);
            self.drag_anchor = self.buffer.anchor()..self.buffer.anchor();
        }
    }
    fn mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }
    fn drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let p = self.position(position);
        if self.word_selecting {
            let range = self.buffer.word_range(p);
            let (anchor, caret) = if range.start < self.drag_anchor.start {
                (self.drag_anchor.end, range.start)
            } else {
                (self.drag_anchor.start, range.end)
            };
            self.buffer.select(anchor, false);
            self.buffer.select(caret, true);
        } else {
            self.buffer.select(p, true);
        }
        self.follow_caret = true;
        cx.notify();
    }
    fn position(&self, point: Point<Pixels>) -> Position {
        self.layout.as_ref().map_or(self.buffer.caret(), |l| {
            self.buffer.clamp(l.position(point, self.scroll))
        })
    }
    fn scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(layout) = &self.layout {
            let delta = event.delta.pixel_delta(layout.line_height);
            self.scroll.x = (self.scroll.x - delta.x).clamp(px(0.0), layout.max_scroll_x);
            self.scroll.y = (self.scroll.y - delta.y).clamp(px(0.0), layout.max_scroll);
            self.follow_caret = false;
            cx.stop_propagation();
            cx.notify();
        }
    }
    fn utf16_range(&self, range: &Range<Position>) -> Range<usize> {
        self.buffer.utf16(range.start)..self.buffer.utf16(range.end)
    }
    fn position_range(&self, range: &Range<usize>) -> Range<Position> {
        self.buffer.position_from_utf16(range.start)..self.buffer.position_from_utf16(range.end)
    }
}

impl Render for EditorView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let container = div()
            .bg(super::codesh::style::card(&self.theme))
            .size_full()
            .min_h(px(0.0))
            .overflow_hidden()
            .key_context("CodeshEditor")
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::mouse_up))
            .on_scroll_wheel(cx.listener(Self::scroll))
            .child(EditorElement {
                editor: cx.entity(),
            });
        Self::key_listeners(container, cx)
    }
}

impl EntityInputHandler for EditorView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.position_range(&range);
        actual.replace(self.utf16_range(&range));
        Some(self.buffer.slice(range).replace("\r\n", "\n"))
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.utf16_range(&self.buffer.range()),
            reversed: self.buffer.caret() < self.buffer.anchor(),
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|range| self.utf16_range(range))
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.finish_composition();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .as_ref()
            .map(|r| self.position_range(r))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.buffer.range());
        let row = if self.marked.is_some() {
            let row = self.buffer.compose(range, text);
            self.finish_composition();
            row
        } else {
            self.buffer.replace(range, text, true)
        };
        self.changed(row, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range
            .as_ref()
            .map(|r| self.position_range(r))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.buffer.range());
        let start = range.start;
        let row = self.buffer.compose(range, text);
        self.marked = Some(start..Buffer::after(start, text));
        if let Some(selected) = selected {
            let base = self.buffer.utf16(start);
            let anchor = self.buffer.position_from_utf16(base + selected.start);
            let caret = self.buffer.position_from_utf16(base + selected.end);
            self.buffer.select(anchor, false);
            self.buffer.select(caret, true);
        }
        if text.is_empty() {
            self.finish_composition();
        }
        self.changed(row, cx);
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let range = self.position_range(&range);
        let start = layout.point(range.start, self.scroll);
        let end = layout.point(range.end, self.scroll);
        Some(Bounds::from_corners(
            start,
            point(end.x.max(start.x + px(1.0)), start.y + layout.line_height),
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.buffer.utf16(self.position(point)))
    }
}
