use codesh_ui::theme::{Theme, blend};
use gpui::Hsla;

/// The one spacing of Codesh's chrome: the window inset, the gap between
/// pane cards, the sidebar's and panels' side padding, and the insets of a
/// card's header and body. Unscaled, so every edge lines up at any UI zoom.
pub(crate) const GUTTER: f32 = 8.0;
pub(crate) const RADIUS: f32 = 10.0;
pub(crate) const HEADER: f32 = 36.0;
/// Height of the macOS window buttons, to centre them on the header row.
const TRAFFIC_LIGHT_HEIGHT: f32 = 14.0;
/// Where the window buttons sit: inside the top-left pane's header row, on
/// the same line as every card header and the sidebar's top row.
pub(crate) const TRAFFIC_LIGHTS: (f32, f32) =
    (2.0 * GUTTER, GUTTER + (HEADER - TRAFFIC_LIGHT_HEIGHT) / 2.0);
/// The window buttons' position as a point.
pub(crate) fn traffic_lights() -> gpui::Point<gpui::Pixels> {
    gpui::point(gpui::px(TRAFFIC_LIGHTS.0), gpui::px(TRAFFIC_LIGHTS.1))
}

/// Room the moved window buttons take beyond the standard measured width,
/// which assumes they start 9px from the edge.
pub(crate) const TRAFFIC_LIGHT_SHIFT: f32 = TRAFFIC_LIGHTS.0 - 9.0;
// Upstream terminal padding is absorbed by the card body inset.
pub(crate) const BASE_TERMINAL_PADDING: f32 = 2.0;
/// Codesh never shows the status bar; the setting remains available to shared components.
pub(crate) const STATUS_BAR: bool = false;
/// A collapsed sidebar disappears; its toggle moves into the top-left pane.
pub(crate) const HIDE_COLLAPSED_SIDEBAR: bool = true;

/// [`GUTTER`] as a length.
pub(crate) fn spacing() -> gpui::Pixels {
    gpui::px(GUTTER)
}

/// Pane cards stand out from the window chrome: lighter in dark themes, and in
/// light themes the chrome darkens so cards keep the theme's own background.
pub(crate) fn card(theme: &Theme) -> Hsla {
    if is_light(theme) {
        return theme.bg;
    }
    tint(theme)
}

/// The window background behind the pane cards, shared by the sidebar.
pub(crate) fn chrome(theme: &Theme) -> Hsla {
    if is_light(theme) {
        return tint(theme);
    }
    theme.bg
}

/// The right side panel floats above the pane cards: lighter still in dark
/// themes, and close to white in light ones.
pub(crate) fn panel(theme: &Theme) -> Hsla {
    if is_light(theme) {
        return blend(gpui::white(), theme.bg, 0.7);
    }
    blend(theme.fg, theme.bg, 0.06)
}

fn tint(theme: &Theme) -> Hsla {
    blend(theme.fg, theme.bg, 0.025)
}

fn is_light(theme: &Theme) -> bool {
    theme.bg.l > theme.fg.l
}

pub(crate) fn border(theme: &Theme) -> Hsla {
    blend(theme.border, theme.bg, 0.95)
}

/// First extent, gutter extent, second origin. Snap only the shared edge.
pub(crate) fn gutter_geometry(extent: f32, ratio: f32, scale: f32) -> [f32; 3] {
    let extent = extent.max(0.0);
    let gutter = extent.min(GUTTER);
    let available = extent - gutter;
    let first = ((available * ratio * scale).round() / scale).clamp(0.0, available);
    [first, gutter, first + gutter]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gutter_conserves_extent_at_fractional_scales_and_tiny_sizes() {
        for extent in [0.0, 5.0, 12.0, 101.0, 1001.0] {
            for scale in [1.0, 1.5, 2.0] {
                for ratio in [0.15, 0.5, 0.85] {
                    let [first, gutter, second] = gutter_geometry(extent, ratio, scale);
                    assert!(first >= 0.0 && second <= extent);
                    assert!((gutter - extent.min(GUTTER)).abs() < f32::EPSILON);
                    assert!((first + gutter + (extent - second) - extent).abs() < 0.001);
                    if extent > GUTTER {
                        assert!((first * scale - (first * scale).round()).abs() < 0.001);
                    }
                }
            }
        }
    }
}
