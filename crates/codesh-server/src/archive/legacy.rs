use codesh_protocol::{Color, ExitReason, Size};
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct Record {
    pub(super) version: u32,
    pub(super) screen: Screen,
    pub(super) history: Vec<Vec<Run>>,
}

#[derive(Deserialize)]
pub(super) struct Screen {
    size: Size,
    rows: Vec<Row>,
    cursor: Cursor,
    reason: Option<ExitReason>,
}

#[derive(Deserialize)]
struct Row {
    index: u16,
    runs: Vec<Run>,
}

#[derive(Deserialize)]
pub(super) struct Run {
    text: String,
    width: u16,
    style: Style,
}

#[derive(Deserialize)]
struct Cursor {
    row: u16,
    col: u16,
    visible: bool,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
struct Style {
    fg: Color,
    bg: Color,
    bold: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
    strikethrough: bool,
    faint: bool,
}

impl From<Screen> for codesh_protocol::SavedScreen {
    fn from(screen: Screen) -> Self {
        Self {
            graphics: codesh_protocol::Graphics::default(),
            size: screen.size,
            rows: screen
                .rows
                .into_iter()
                .map(|row| codesh_protocol::Row {
                    index: row.index,
                    runs: row.runs.into_iter().map(Into::into).collect(),
                })
                .collect(),
            cursor: codesh_protocol::Cursor {
                row: screen.cursor.row,
                col: screen.cursor.col,
                visible: screen.cursor.visible,
                shape: codesh_protocol::CursorShape::Block,
            },
            reason: screen.reason,
        }
    }
}

impl From<Run> for codesh_protocol::Run {
    fn from(run: Run) -> Self {
        let style = run.style;
        Self {
            text: run.text,
            width: run.width,
            style: codesh_protocol::Style {
                fg: style.fg,
                bg: style.bg,
                bold: style.bold,
                italic: style.italic,
                inverse: style.inverse,
                faint: style.faint,
                strikethrough: style.strikethrough,
                underline: if style.underline {
                    codesh_protocol::Underline::Single
                } else {
                    codesh_protocol::Underline::None
                },
                ..codesh_protocol::Style::default()
            },
        }
    }
}
