//! Palette geometry in logical points. Every height comes from row counts so the
//! panel never measures itself.

use gpui::Pixels;

use crate::text_input::InputStyle;
use crate::theme::{Metrics, Theme};

pub(super) const WIDTH: f32 = 740.0;
pub(super) const MAX_HEIGHT: f32 = 480.0;
pub(super) const TOP: f32 = 72.0;
pub(super) const PANEL_RADIUS: f32 = 14.0;
pub(super) const HEADER: f32 = 56.0;
pub(super) const SECTION: f32 = 28.0;
pub(super) const ROW: f32 = 40.0;
pub(super) const ROW_INSET: f32 = 8.0;
pub(super) const ROW_RADIUS: f32 = 8.0;
pub(super) const TILE: f32 = 24.0;
pub(super) const TILE_RADIUS: f32 = 6.0;
pub(super) const FOOTER: f32 = 40.0;
pub(super) const STATUS: f32 = 96.0;
pub(super) const LIST_INSET_Y: f32 = 6.0;
pub(super) const SEARCH_FONT: f32 = 18.0;
pub(super) const SEARCH_LINE: f32 = 24.0;
pub(super) const ACTIONS_WIDTH: f32 = 300.0;
pub(super) const ACTIONS_ROW: f32 = 32.0;
pub(super) const ACTIONS_MAX: f32 = 320.0;
pub(super) const KEYCAP: f32 = 20.0;
pub(super) const VIEWPORT_MARGIN: f32 = 8.0;

/// The panel's outer border, top and bottom, which is not scaled.
const BORDER: f32 = 2.0;

/// The height of the panel for a page with `sections` headers and `items`
/// rows, capped at `max_height`. `list` is false when a status message shows
/// instead of the rows.
#[allow(
    clippy::cast_precision_loss,
    reason = "Row counts map to GPUI f32 coordinates."
)]
pub(super) fn fitted_height(
    sections: usize,
    items: usize,
    list: bool,
    max_height: f32,
    metrics: Metrics,
) -> Pixels {
    let body = if list {
        sections as f32 * SECTION + items as f32 * ROW + LIST_INSET_Y * 2.0
    } else {
        STATUS
    };
    metrics.scaled((HEADER + body + FOOTER).min(max_height)) + gpui::px(BORDER)
}

/// The search field style: larger than [`InputStyle::field`], local to the palette.
pub(super) fn search_style(theme: &Theme, metrics: Metrics) -> InputStyle {
    InputStyle {
        font_size: metrics.scaled(SEARCH_FONT),
        line_height: metrics.scaled(SEARCH_LINE),
        ..InputStyle::field(theme, &metrics)
    }
}

/// The ⌘K filter field style.
pub(super) fn filter_style(theme: &Theme, metrics: Metrics) -> InputStyle {
    InputStyle::compact(theme, &metrics)
}

/// The height of the ⌘K panel for `rows` actions, capped at [`ACTIONS_MAX`].
#[allow(
    clippy::cast_precision_loss,
    reason = "Row counts map to GPUI f32 coordinates."
)]
pub(super) fn actions_height(rows: usize, message: bool, metrics: Metrics) -> Pixels {
    let list = (rows.max(1) as f32).mul_add(ACTIONS_ROW, LIST_INSET_Y * 2.0);
    let message = if message { ACTIONS_ROW } else { 0.0 };
    metrics.scaled((list + message + ACTIONS_ROW + LIST_INSET_Y).min(ACTIONS_MAX))
        + gpui::px(BORDER)
}
