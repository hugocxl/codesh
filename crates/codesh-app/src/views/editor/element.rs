use super::{EditorView, buffer::Position};
use gpui::{
    App, Bounds, ContentMask, DispatchPhase, Element, ElementId, ElementInputHandler, Entity,
    GlobalElementId, IntoElement, LayoutId, MouseMoveEvent, PaintQuad, Pixels, Point, ShapedLine,
    Style, TextRun, UnderlineStyle, Window, fill, point, px, size,
};

#[derive(Debug)]
struct DisplayLine {
    shaped: ShapedLine,
    offsets: Vec<(usize, usize)>,
}

fn display_offset(offsets: &[(usize, usize)], source: usize) -> usize {
    offsets
        .get(
            offsets
                .partition_point(|(byte, _)| *byte <= source)
                .saturating_sub(1),
        )
        .map_or(source, |(byte, display)| display + source - byte)
}

fn expand_tabs(text: &str, indent: usize) -> (String, Vec<(usize, usize)>) {
    if !text.contains('\t') {
        return (text.to_owned(), Vec::new());
    }
    let mut expanded = String::with_capacity(text.len());
    let mut offsets = Vec::new();
    let mut column = 0;
    for (byte, ch) in text.char_indices() {
        if ch == '\t' {
            offsets.push((byte, expanded.len()));
            let spaces = indent - column % indent;
            expanded.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
            offsets.push((byte + 1, expanded.len()));
        } else {
            expanded.push(ch);
            column += 1;
        }
    }
    (expanded, offsets)
}

impl DisplayLine {
    fn x(&self, column: usize) -> Pixels {
        self.shaped
            .x_for_index(display_offset(&self.offsets, column))
    }
    fn column(&self, x: Pixels) -> usize {
        if x <= px(0.0) {
            return 0;
        }
        let display = self.shaped.index_for_x(x).unwrap_or(self.shaped.len());
        for pair in self.offsets.chunks_exact(2) {
            let [(source, start), (end_source, end)] = pair else {
                continue;
            };
            if display >= *start && display < *end {
                return if 2 * (display - start) < end - start {
                    *source
                } else {
                    *end_source
                };
            }
        }
        self.offsets
            .get(
                self.offsets
                    .partition_point(|(_, byte)| *byte <= display)
                    .saturating_sub(1),
            )
            .map_or(display, |(source, byte)| source + display - byte)
    }
}

#[derive(Debug)]
pub(super) struct Layout {
    pub bounds: Bounds<Pixels>,
    pub line_height: Pixels,
    pub gutter: Pixels,
    pub max_scroll: Pixels,
    pub max_scroll_x: Pixels,
    first: usize,
    lines: Vec<DisplayLine>,
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Visible row indices map to GPUI pixel coordinates."
)]
impl Layout {
    pub(super) fn position(&self, position: Point<Pixels>, scroll: Point<Pixels>) -> Position {
        let row = ((f32::from(position.y - self.bounds.top() + scroll.y)
            / f32::from(self.line_height))
        .floor()
        .max(0.0)) as usize;
        let x = position.x - self.bounds.left() - self.gutter + scroll.x;
        let column = self
            .lines
            .get(row.saturating_sub(self.first))
            .map_or(0, |line| line.column(x));
        Position { row, column }
    }
    pub(super) fn point(&self, p: Position, scroll: Point<Pixels>) -> Point<Pixels> {
        let x = self
            .lines
            .get(p.row.wrapping_sub(self.first))
            .map_or(px(0.0), |line| line.x(p.column));
        point(
            self.bounds.left() + self.gutter + x - scroll.x,
            self.bounds.top() + self.line_height * p.row as f32 - scroll.y,
        )
    }
}

pub(super) struct EditorElement {
    pub editor: Entity<EditorView>,
}
impl IntoElement for EditorElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

pub(super) struct Prepaint {
    layout: Layout,
    numbers: Vec<ShapedLine>,
    selections: Vec<PaintQuad>,
    cursor: PaintQuad,
    scroll: Point<Pixels>,
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }
    #[allow(
        clippy::too_many_lines,
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Virtualized text layout converts bounded visible rows to pixel positions."
    )]
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Prepaint {
        self.editor.update(cx, |editor, cx| {
            let (face, font_size, cell) =
                crate::views::terminal::typography::resolve(&editor.terminal, window);
            let line_height = cell.height;
            let base = TextRun {
                len: 0,
                font: face,
                color: editor.theme.fg,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let number_width = editor.buffer.lines.len().to_string();
            let measure = window.text_system().shape_line(
                number_width.clone().into(),
                font_size,
                &[TextRun {
                    len: number_width.len(),
                    ..base.clone()
                }],
                None,
            );
            let gutter = measure.width + px(24.0);
            let count = editor.buffer.lines.len();
            let max_scroll = (line_height * count as f32 - bounds.size.height).max(px(0.0));
            let mut scroll = editor.scroll;
            let caret = editor.buffer.caret();
            if editor.follow_caret {
                let top = line_height * caret.row as f32;
                if top < scroll.y {
                    scroll.y = top;
                }
                if top + line_height > scroll.y + bounds.size.height {
                    scroll.y = top + line_height - bounds.size.height;
                }
                let text = &editor.buffer.lines[caret.row];
                let (expanded, offsets) = expand_tabs(text, editor.buffer.indent);
                let caret_line = window.text_system().shape_line(
                    expanded.clone().into(),
                    font_size,
                    &[TextRun {
                        len: expanded.len(),
                        ..base.clone()
                    }],
                    None,
                );
                let x = caret_line.x_for_index(display_offset(&offsets, caret.column));
                let width = (bounds.size.width - gutter - px(4.0)).max(px(1.0));
                if x < scroll.x {
                    scroll.x = x;
                }
                if x + px(2.0) > scroll.x + width {
                    scroll.x = x + px(2.0) - width;
                }
            }
            scroll.y = scroll.y.clamp(px(0.0), max_scroll);
            let first = (f32::from(scroll.y) / f32::from(line_height)).floor() as usize;
            let end = (first
                + (f32::from(bounds.size.height) / f32::from(line_height)).ceil() as usize
                + 1)
            .min(count);
            if editor.highlights.prepare(&editor.buffer.lines, end, 128) {
                cx.notify();
            }
            let mut lines = Vec::with_capacity(end - first);
            let mut numbers = Vec::with_capacity(end - first);
            for row in first..end {
                let text = &editor.buffer.lines[row];
                let (expanded, offsets) = expand_tabs(text, editor.buffer.indent);
                let mut runs = Vec::new();
                let tokens = editor.highlights.tokens(row);
                let fallback = [(text.len(), editor.theme.fg)];
                let mut offset = 0;
                for &(len, color) in tokens.unwrap_or(&fallback) {
                    let mut boundaries = [offset, offset + len, offset + len, offset + len];
                    let marked = editor
                        .marked
                        .as_ref()
                        .filter(|r| row >= r.start.row && row <= r.end.row);
                    let marked_range = marked.map(|r| {
                        (if row == r.start.row {
                            r.start.column
                        } else {
                            0
                        })..(if row == r.end.row {
                            r.end.column
                        } else {
                            text.len()
                        })
                    });
                    if let Some(range) = &marked_range {
                        for (slot, boundary) in
                            boundaries[2..].iter_mut().zip([range.start, range.end])
                        {
                            if boundary > offset && boundary < offset + len {
                                *slot = boundary;
                            }
                        }
                        boundaries.sort_unstable();
                    }
                    for span in boundaries.windows(2) {
                        if span[1] > span[0] {
                            runs.push(TextRun {
                                len: display_offset(&offsets, span[1])
                                    - display_offset(&offsets, span[0]),
                                color,
                                underline: marked_range
                                    .as_ref()
                                    .filter(|r| r.contains(&span[0]))
                                    .map(|_| UnderlineStyle {
                                        color: Some(color),
                                        thickness: px(1.0),
                                        wavy: false,
                                    }),
                                ..base.clone()
                            });
                        }
                    }
                    offset += len;
                }
                if offset < text.len() {
                    runs.push(TextRun {
                        len: expanded.len() - display_offset(&offsets, offset),
                        ..base.clone()
                    });
                }
                if runs.is_empty() {
                    runs.push(base.clone());
                }
                lines.push(DisplayLine {
                    shaped: window.text_system().shape_line(
                        expanded.into(),
                        font_size,
                        &runs,
                        None,
                    ),
                    offsets,
                });
                let number = (row + 1).to_string();
                numbers.push(window.text_system().shape_line(
                    number.clone().into(),
                    font_size,
                    &[TextRun {
                        len: number.len(),
                        color: editor.theme.fg_muted,
                        ..base.clone()
                    }],
                    None,
                ));
            }
            let max_scroll_x = (lines
                .iter()
                .map(|line| line.shaped.width)
                .max()
                .unwrap_or(px(0.0))
                - (bounds.size.width - gutter).max(px(1.0))
                + px(4.0))
            .max(px(0.0));
            scroll.x = scroll.x.clamp(px(0.0), max_scroll_x);
            let layout = Layout {
                bounds,
                line_height,
                gutter,
                max_scroll,
                max_scroll_x,
                first,
                lines,
            };
            let range = editor.buffer.range();
            let mut selections = Vec::new();
            if !range.is_empty() {
                for row in first..end {
                    if row < range.start.row || row > range.end.row {
                        continue;
                    }
                    let start = Position {
                        row,
                        column: if row == range.start.row {
                            range.start.column
                        } else {
                            0
                        },
                    };
                    let end = Position {
                        row,
                        column: if row == range.end.row {
                            range.end.column
                        } else {
                            editor.buffer.lines[row].len()
                        },
                    };
                    let left = layout.point(start, scroll);
                    let mut right = layout.point(end, scroll);
                    if row < range.end.row {
                        right.x += px(8.0);
                    }
                    selections.push(fill(
                        Bounds::new(left, size((right.x - left.x).max(px(0.0)), line_height)),
                        editor.theme.accent_soft,
                    ));
                }
            }
            let cursor = fill(
                Bounds::new(layout.point(caret, scroll), size(px(1.5), line_height)),
                editor.theme.accent,
            );
            Prepaint {
                layout,
                numbers,
                selections,
                cursor,
                scroll,
            }
        })
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "Visible line indices become pixel coordinates."
    )]
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        prepaint: &mut Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.editor.read(cx).focus.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        let editor = self.editor.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble && editor.read(cx).selecting {
                editor.update(cx, |view, cx| view.drag(event.position, cx));
            }
        });
        let layout = &prepaint.layout;
        let text_bounds = Bounds::from_corners(
            point(
                (bounds.left() + layout.gutter).min(bounds.right()),
                bounds.top(),
            ),
            bounds.bottom_right(),
        );
        window.with_content_mask(
            Some(ContentMask {
                bounds: text_bounds,
            }),
            |window| {
                for selection in prepaint.selections.drain(..) {
                    window.paint_quad(selection);
                }
                for (index, line) in layout.lines.iter().enumerate() {
                    let origin = point(
                        bounds.left() + layout.gutter - prepaint.scroll.x,
                        bounds.top() + layout.line_height * (layout.first + index) as f32
                            - prepaint.scroll.y,
                    );
                    let _ = line.shaped.paint(origin, layout.line_height, window, cx);
                }
                if focus.is_focused(window)
                    && window.is_window_active()
                    && self.editor.read(cx).buffer.range().is_empty()
                {
                    window.paint_quad(prepaint.cursor.clone());
                }
            },
        );
        window.with_content_mask(
            Some(ContentMask {
                bounds: Bounds::new(
                    bounds.origin,
                    size(layout.gutter.min(bounds.size.width), bounds.size.height),
                ),
            }),
            |window| {
                for (index, number) in prepaint.numbers.iter().enumerate() {
                    let origin = point(
                        bounds.left() + layout.gutter - px(12.0) - number.width,
                        bounds.top() + layout.line_height * (layout.first + index) as f32
                            - prepaint.scroll.y,
                    );
                    let _ = number.paint(origin, layout.line_height, window, cx);
                }
            },
        );
        let empty = Layout {
            bounds,
            line_height: layout.line_height,
            gutter: layout.gutter,
            max_scroll: layout.max_scroll,
            max_scroll_x: layout.max_scroll_x,
            first: 0,
            lines: Vec::new(),
        };
        let layout = std::mem::replace(&mut prepaint.layout, empty);
        self.editor.update(cx, |editor, _| {
            editor.layout = Some(layout);
            editor.scroll = prepaint.scroll;
            editor.follow_caret = false;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_tab_expansion_preserves_utf8_cursor_and_token_offsets() {
        let (text, offsets) = expand_tabs("\té\tx", 4);
        assert_eq!(text, "    é   x");
        assert_eq!(display_offset(&offsets, 0), 0);
        assert_eq!(display_offset(&offsets, 1), 4);
        assert_eq!(display_offset(&offsets, 3), 6);
        assert_eq!(display_offset(&offsets, 4), 9);
        assert_eq!(display_offset(&offsets, 5), text.len());
    }
}
