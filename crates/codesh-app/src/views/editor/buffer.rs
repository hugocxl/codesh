use std::{
    ops::Range,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Position {
    pub row: usize,
    pub column: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Selection {
    anchor: Position,
    caret: Position,
}

#[derive(Debug)]
struct Edit {
    at: Position,
    removed: String,
    inserted: String,
    before: Selection,
    after: Selection,
    revision: u64,
    next_revision: u64,
}

#[derive(Debug)]
pub(super) struct Buffer {
    pub lines: Vec<String>,
    crlf: Vec<bool>,
    selection: Selection,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    revision: u64,
    serial: u64,
    saved: u64,
    typing: Option<Instant>,
    pub indent: usize,
    newline: &'static str,
    composition: Option<Edit>,
    pub changed_through: usize,
}

impl Buffer {
    pub(super) fn new(text: &str) -> Self {
        let (lines, crlf) = split_lines(text);
        let newline = if crlf[0] { "\r\n" } else { "\n" };
        let indent = lines
            .iter()
            .filter_map(|line| {
                let n = line.bytes().take_while(|c| *c == b' ').count();
                (n > 0).then_some(n)
            })
            .min()
            .unwrap_or(4)
            .clamp(1, 8);
        Self {
            lines,
            crlf,
            selection: Selection::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            serial: 0,
            saved: 0,
            typing: None,
            indent,
            newline,
            composition: None,
            changed_through: 0,
        }
    }

    pub(super) fn text(&self) -> String {
        self.slice(Position::default()..self.end())
    }
    pub(super) fn dirty(&self) -> bool {
        self.revision != self.saved
    }
    pub(super) fn mark_saved(&mut self) {
        self.saved = self.revision;
        self.break_typing();
    }
    pub(super) fn caret(&self) -> Position {
        self.selection.caret
    }
    pub(super) fn anchor(&self) -> Position {
        self.selection.anchor
    }
    pub(super) fn end(&self) -> Position {
        let row = self.lines.len() - 1;
        Position {
            row,
            column: self.lines[row].len(),
        }
    }
    pub(super) fn clamp(&self, mut p: Position) -> Position {
        p.row = p.row.min(self.lines.len() - 1);
        p.column = p.column.min(self.lines[p.row].len());
        while !self.lines[p.row].is_char_boundary(p.column) {
            p.column -= 1;
        }
        p
    }
    pub(super) fn select(&mut self, target: Position, extend: bool) {
        let target = self.clamp(target);
        if !extend {
            self.selection.anchor = target;
        }
        self.selection.caret = target;
        self.break_typing();
    }
    pub(super) fn range(&self) -> Range<Position> {
        self.anchor().min(self.caret())..self.anchor().max(self.caret())
    }
    pub(super) fn slice(&self, range: Range<Position>) -> String {
        if range.start.row == range.end.row {
            return self.lines[range.start.row][range.start.column..range.end.column].to_owned();
        }
        let mut result = self.lines[range.start.row][range.start.column..].to_owned();
        for row in range.start.row + 1..=range.end.row {
            result.push_str(if self.crlf[row - 1] { "\r\n" } else { "\n" });
            result.push_str(if row == range.end.row {
                &self.lines[row][..range.end.column]
            } else {
                &self.lines[row]
            });
        }
        result
    }
    pub(super) fn selected(&self) -> String {
        self.slice(self.range())
    }
    pub(super) fn break_typing(&mut self) {
        self.typing = None;
    }
    pub(super) fn after(at: Position, text: &str) -> Position {
        let count = text.bytes().filter(|b| *b == b'\n').count();
        if count == 0 {
            Position {
                column: at.column + text.len(),
                ..at
            }
        } else {
            Position {
                row: at.row + count,
                column: text.rsplit('\n').next().unwrap_or_default().len(),
            }
        }
    }
    fn replace_raw(&mut self, range: Range<Position>, text: &str) {
        let prefix = &self.lines[range.start.row][..range.start.column];
        let suffix = &self.lines[range.end.row][range.end.column..];
        let replacement = format!("{prefix}{text}{suffix}");
        self.changed_through = Self::after(range.start, text).row;
        let (lines, mut crlf) = split_lines(&replacement);
        if let Some(last) = crlf.last_mut() {
            *last = self.crlf[range.end.row];
        }
        self.lines.splice(range.start.row..=range.end.row, lines);
        self.crlf.splice(range.start.row..=range.end.row, crlf);
    }
    pub(super) fn replace(&mut self, range: Range<Position>, text: &str, typing: bool) -> usize {
        self.finish_composition();
        let text = text.to_owned();
        let removed = self.slice(range.clone());
        if text == removed {
            self.select(Self::after(range.start, &text), false);
            return range.start.row;
        }
        let before = self.selection;
        self.replace_raw(range.clone(), &text);
        let caret = Self::after(range.start, &text);
        self.selection = Selection {
            anchor: caret,
            caret,
        };
        self.serial += 1;
        let coalesce = typing
            && removed.is_empty()
            && !text.contains('\n')
            && self
                .typing
                .is_some_and(|last| last.elapsed() < Duration::from_millis(750));
        if let Some(last) = self
            .undo
            .last_mut()
            .filter(|last| coalesce && last.after == before && last.removed.is_empty())
        {
            last.inserted.push_str(&text);
            last.after = self.selection;
            last.next_revision = self.serial;
        } else {
            self.undo.push(Edit {
                at: range.start,
                removed,
                inserted: text,
                before,
                after: self.selection,
                revision: self.revision,
                next_revision: self.serial,
            });
        }
        self.revision = self.serial;
        self.redo.clear();
        self.typing = typing.then(Instant::now);
        range.start.row
    }
    pub(super) fn insert(&mut self, text: &str, typing: bool) -> usize {
        self.replace(self.range(), text, typing)
    }
    pub(super) fn compose(&mut self, range: Range<Position>, text: &str) -> usize {
        if self.composition.is_none() {
            self.composition = Some(Edit {
                at: range.start,
                removed: self.slice(range.clone()),
                inserted: String::new(),
                before: self.selection,
                after: self.selection,
                revision: self.revision,
                next_revision: self.revision,
            });
        }
        self.replace_raw(range.clone(), text);
        let caret = Self::after(range.start, text);
        self.selection = Selection {
            anchor: caret,
            caret,
        };
        self.serial += 1;
        self.revision = self.serial;
        if let Some(edit) = self.composition.as_mut() {
            text.clone_into(&mut edit.inserted);
            edit.after = self.selection;
            edit.next_revision = self.serial;
        }
        self.break_typing();
        range.start.row
    }
    pub(super) fn finish_composition(&mut self) {
        if let Some(edit) = self.composition.take() {
            if edit.removed == edit.inserted {
                self.revision = edit.revision;
            } else {
                self.undo.push(edit);
                self.redo.clear();
            }
        }
    }
    pub(super) fn undo(&mut self) -> Option<usize> {
        self.finish_composition();
        let edit = self.undo.pop()?;
        self.replace_raw(edit.at..Self::after(edit.at, &edit.inserted), &edit.removed);
        self.selection = edit.before;
        self.revision = edit.revision;
        let row = edit.at.row;
        self.redo.push(edit);
        self.break_typing();
        Some(row)
    }
    pub(super) fn redo(&mut self) -> Option<usize> {
        let edit = self.redo.pop()?;
        self.replace_raw(edit.at..Self::after(edit.at, &edit.removed), &edit.inserted);
        self.selection = edit.after;
        self.revision = edit.next_revision;
        let row = edit.at.row;
        self.undo.push(edit);
        self.break_typing();
        Some(row)
    }
    pub(super) fn previous(&self, p: Position) -> Position {
        if p.column > 0 {
            Position {
                column: self.lines[p.row][..p.column]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(i, _)| i),
                ..p
            }
        } else if p.row > 0 {
            Position {
                row: p.row - 1,
                column: self.lines[p.row - 1].len(),
            }
        } else {
            p
        }
    }
    pub(super) fn next(&self, p: Position) -> Position {
        if p.column < self.lines[p.row].len() {
            Position {
                column: p.column
                    + self.lines[p.row][p.column..]
                        .chars()
                        .next()
                        .map_or(0, char::len_utf8),
                ..p
            }
        } else if p.row + 1 < self.lines.len() {
            Position {
                row: p.row + 1,
                column: 0,
            }
        } else {
            p
        }
    }
    fn char_at(&self, p: Position) -> char {
        self.lines[p.row][p.column..].chars().next().unwrap_or('\n')
    }
    pub(super) fn word_left(&self, mut p: Position) -> Position {
        while p != Position::default() && self.char_at(self.previous(p)).is_whitespace() {
            p = self.previous(p);
        }
        let class = word_class(self.char_at(self.previous(p)));
        while p != Position::default() && word_class(self.char_at(self.previous(p))) == class {
            p = self.previous(p);
        }
        p
    }
    pub(super) fn word_right(&self, mut p: Position) -> Position {
        let class = word_class(self.char_at(p));
        while p != self.end() && word_class(self.char_at(p)) == class {
            p = self.next(p);
        }
        while p != self.end() && self.char_at(p).is_whitespace() {
            p = self.next(p);
        }
        p
    }
    pub(super) fn word_range(&self, p: Position) -> Range<Position> {
        let class = word_class(self.char_at(p));
        let mut start = p;
        let mut end = p;
        while start.column > 0 && word_class(self.char_at(self.previous(start))) == class {
            start = self.previous(start);
        }
        while end.column < self.lines[end.row].len() && word_class(self.char_at(end)) == class {
            end = self.next(end);
        }
        start..end
    }
    pub(super) fn newline(&self) -> String {
        let p = self.range().start;
        let prefix = &self.lines[p.row][..p.column];
        let indent: String = prefix
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        let extra = if prefix.trim_end().ends_with(['{', '[', '(']) {
            " ".repeat(self.indent)
        } else {
            String::new()
        };
        format!("{}{indent}{extra}", self.newline)
    }
    pub(super) fn utf16(&self, p: Position) -> usize {
        self.lines[..p.row]
            .iter()
            .map(|s| s.encode_utf16().count() + 1)
            .sum::<usize>()
            + self.lines[p.row][..p.column].encode_utf16().count()
    }
    pub(super) fn position_from_utf16(&self, mut offset: usize) -> Position {
        for (row, line) in self.lines.iter().enumerate() {
            for (column, ch) in line.char_indices() {
                if offset < ch.len_utf16() {
                    return Position { row, column };
                }
                offset -= ch.len_utf16();
            }
            if offset == 0 {
                return Position {
                    row,
                    column: line.len(),
                };
            }
            offset -= 1;
        }
        self.end()
    }
}
fn split_lines(text: &str) -> (Vec<String>, Vec<bool>) {
    let mut lines = Vec::new();
    let mut crlf = Vec::new();
    let mut parts = text.split('\n').peekable();
    while let Some(line) = parts.next() {
        let has_cr = parts.peek().is_some() && line.ends_with('\r');
        lines.push(if has_cr {
            line[..line.len() - 1].to_owned()
        } else {
            line.to_owned()
        });
        crlf.push(has_cr);
    }
    (lines, crlf)
}

fn word_class(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_mixed_line_endings_survive_edits_and_history() {
        let original = "one\r\ntwo\nthree\r\n";
        let mut b = Buffer::new(original);
        assert_eq!(b.text(), original);
        b.select(Position { row: 1, column: 3 }, false);
        b.insert("!", true);
        assert_eq!(b.text(), "one\r\ntwo!\nthree\r\n");
        b.insert(&b.newline(), false);
        assert_eq!(b.text(), "one\r\ntwo!\r\n\nthree\r\n");
        b.undo();
        b.undo();
        assert_eq!(b.text(), original);
        b.redo();
        b.redo();
        assert_eq!(b.text(), "one\r\ntwo!\r\n\nthree\r\n");
    }
    #[test]
    fn editor_ime_updates_are_one_undo_and_cancel_preserves_the_next_composition() {
        let mut b = Buffer::new("a😀b");
        let start = Position { row: 0, column: 1 };
        let end = Position { row: 0, column: 5 };
        b.select(start, false);
        b.select(end, true);
        b.compose(start..end, "に");
        b.compose(start..Buffer::after(start, "に"), "日本");
        b.finish_composition();
        assert_eq!(b.text(), "a日本b");
        b.undo();
        assert_eq!(b.text(), "a😀b");
        assert_eq!(b.range(), start..end);
        b.redo();
        assert_eq!(b.text(), "a日本b");
        let p = b.caret();
        b.compose(p..p, "x");
        b.compose(p..Buffer::after(p, "x"), "");
        b.finish_composition();
        b.compose(p..p, "Y");
        b.finish_composition();
        b.undo();
        assert_eq!(b.text(), "a日本b");
    }
    #[test]
    fn editor_enter_and_join_are_reversible_at_unicode_boundaries() {
        let mut b = Buffer::new("  if ready {");
        b.select(b.end(), false);
        b.insert(&b.newline(), false);
        assert_eq!(b.text(), "  if ready {\n    ");
        b.undo();
        assert_eq!(b.text(), "  if ready {");
        let mut b = Buffer::new("é\n😀");
        let next = Position { row: 1, column: 0 };
        b.replace(b.previous(next)..next, "", false);
        assert_eq!(b.text(), "é😀");
        b.undo();
        assert_eq!(b.text(), "é\n😀");
    }
    #[test]
    fn editor_multiline_unicode_edits_and_history_preserve_selection() {
        let mut b = Buffer::new("a😀b\nsecond\n");
        b.select(Position { row: 0, column: 1 }, false);
        b.select(Position { row: 1, column: 3 }, true);
        assert_eq!(b.selected(), "😀b\nsec");
        b.insert("x\ny", false);
        assert_eq!(b.text(), "ax\nyond\n");
        b.mark_saved();
        b.undo();
        assert_eq!(b.selected(), "😀b\nsec");
        assert!(b.dirty());
        b.redo();
        assert!(!b.dirty());
        assert_eq!(b.text(), "ax\nyond\n");
    }
    #[test]
    fn editor_typing_coalesces_but_save_and_navigation_break_groups() {
        let mut b = Buffer::new("");
        b.insert("h", true);
        b.insert("i", true);
        b.undo();
        assert_eq!(b.text(), "");
        b.redo();
        b.mark_saved();
        b.insert("!", true);
        b.undo();
        assert_eq!(b.text(), "hi");
        assert!(!b.dirty());
        b.insert("?", false);
        assert!(b.redo().is_none());
    }
    #[test]
    fn editor_words_indent_and_utf16() {
        let mut b = Buffer::new("  fn café() {\n    value\n}");
        let p = Position { row: 0, column: 10 };
        assert_eq!(b.word_left(p).column, 5);
        assert_eq!(b.word_right(Position { row: 0, column: 5 }).column, 10);
        b.select(Position { row: 0, column: 14 }, false);
        assert_eq!(b.newline(), "\n    ");
        let b = Buffer::new("a😀\n日本");
        for p in [
            Position { row: 0, column: 1 },
            Position { row: 0, column: 5 },
            Position { row: 1, column: 3 },
        ] {
            assert_eq!(b.position_from_utf16(b.utf16(p)), p);
        }
    }
    #[test]
    fn editor_reverse_selection_and_crlf_roundtrip() {
        let mut b = Buffer::new("one\r\ntwo\r\n");
        b.select(b.end(), false);
        b.select(Position::default(), true);
        assert_eq!(b.range(), Position::default()..b.end());
        b.insert("a\r\nb", false);
        assert_eq!(b.text(), "a\r\nb");
        b.undo();
        assert_eq!(b.text(), "one\r\ntwo\r\n");
    }
}
