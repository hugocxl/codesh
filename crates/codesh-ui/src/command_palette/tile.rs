//! The coloured rounded-square icon at the start of a palette row.

use gpui::{
    AnyElement, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled, div, hsla,
    linear_color_stop, linear_gradient, px,
};

use super::layout;
use crate::components::IconGlyph;
#[cfg(target_os = "macos")]
use crate::components::SymbolGlyph;
use crate::icon::Icon;
use crate::theme::{Metrics, Theme};

/// What a tile draws.
#[derive(Clone, Debug, PartialEq)]
pub enum TileGlyph {
    Icon(Icon),
    /// An SF Symbol name; other platforms draw the matching [`Icon`].
    Symbol(SharedString),
    /// An asset path. PNG images draw as they are, without a tile background;
    /// SVGs are tinted like icons.
    Asset(SharedString),
    /// One or two letters.
    Monogram(SharedString),
    /// Up to four colours in a 2×2 grid, such as a theme preview.
    Swatches(Vec<Hsla>),
    None,
}

/// The tile background colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TileHue {
    Red,
    Orange,
    Amber,
    Green,
    Teal,
    Blue,
    Indigo,
    Purple,
    Pink,
    Gray,
    /// The theme accent, with its contrasting foreground.
    Accent,
    Custom(Hsla),
    /// No background: the glyph draws muted.
    None,
}

impl TileHue {
    fn color(self, theme: &Theme) -> Option<Hsla> {
        let (hue, saturation, lightness) = match self {
            Self::Red => (4.0, 0.72, 0.52),
            Self::Orange => (24.0, 0.85, 0.52),
            Self::Amber => (38.0, 0.90, 0.46),
            Self::Green => (142.0, 0.52, 0.42),
            Self::Teal => (176.0, 0.58, 0.40),
            Self::Blue => (214.0, 0.78, 0.54),
            Self::Indigo => (242.0, 0.58, 0.58),
            Self::Purple => (276.0, 0.58, 0.56),
            Self::Pink => (332.0, 0.68, 0.56),
            Self::Gray => (220.0, 0.08, 0.50),
            Self::Accent => return Some(theme.accent),
            Self::Custom(color) => return Some(color),
            Self::None => return None,
        };
        Some(hsla(hue / 360.0, saturation, lightness, 1.0))
    }

    fn foreground(self, theme: &Theme) -> Hsla {
        match self {
            Self::Accent => theme.accent_foreground,
            Self::None => theme.fg_muted,
            _ => gpui::white(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub glyph: TileGlyph,
    pub hue: TileHue,
}

impl Default for Tile {
    fn default() -> Self {
        Self::none()
    }
}

impl Tile {
    pub fn icon(icon: Icon, hue: TileHue) -> Self {
        Self {
            glyph: TileGlyph::Icon(icon),
            hue,
        }
    }

    pub fn symbol(symbol: impl Into<SharedString>, hue: TileHue) -> Self {
        Self {
            glyph: TileGlyph::Symbol(symbol.into()),
            hue,
        }
    }

    pub fn asset(path: impl Into<SharedString>, hue: TileHue) -> Self {
        Self {
            glyph: TileGlyph::Asset(path.into()),
            hue,
        }
    }

    pub fn monogram(text: impl Into<SharedString>, hue: TileHue) -> Self {
        Self {
            glyph: TileGlyph::Monogram(text.into()),
            hue,
        }
    }

    pub fn swatches(colors: Vec<Hsla>) -> Self {
        Self {
            glyph: TileGlyph::Swatches(colors),
            hue: TileHue::None,
        }
    }

    pub fn none() -> Self {
        Self {
            glyph: TileGlyph::None,
            hue: TileHue::None,
        }
    }
}

fn is_png(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("png"))
}

/// The tile element, [`layout::TILE`] points square.
pub(super) fn render(tile: &Tile, theme: &Theme, metrics: Metrics) -> AnyElement {
    let size = metrics.scaled(layout::TILE);
    let frame = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(size)
        .rounded(metrics.scaled(layout::TILE_RADIUS));
    match &tile.glyph {
        TileGlyph::None => return frame.into_any_element(),
        TileGlyph::Asset(path) if is_png(path) => {
            return frame
                .child(gpui::img(path.clone()).size(size))
                .into_any_element();
        }
        TileGlyph::Swatches(colors) => {
            return frame
                .bg(theme.raised())
                .border_1()
                .border_color(theme.border)
                .child(swatches(colors, metrics))
                .into_any_element();
        }
        _ => {}
    }
    let foreground = tile.hue.foreground(theme);
    let glyph_size = metrics.scaled(layout::TILE * 0.6);
    let glyph = match &tile.glyph {
        TileGlyph::Icon(icon) => IconGlyph::new(*icon, glyph_size, foreground)
            .fit()
            .into_any_element(),
        TileGlyph::Symbol(symbol) => symbol_glyph(symbol, glyph_size, foreground),
        TileGlyph::Asset(path) => gpui::svg()
            .path(path.clone())
            .size(glyph_size)
            .flex_none()
            .text_color(foreground)
            .into_any_element(),
        TileGlyph::Monogram(text) => div()
            .text_size(metrics.font_body())
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(foreground)
            .child(text.clone())
            .into_any_element(),
        TileGlyph::Swatches(_) | TileGlyph::None => div().into_any_element(),
    };
    let Some(color) = tile.hue.color(theme) else {
        return frame.child(glyph).into_any_element();
    };
    let mut top = color;
    top.l = (top.l + 0.06).min(1.0);
    frame
        .bg(linear_gradient(
            180.0,
            linear_color_stop(top, 0.0),
            linear_color_stop(color, 1.0),
        ))
        .border_1()
        .border_color(hsla(0.0, 0.0, 1.0, 0.18))
        .child(glyph)
        .into_any_element()
}

#[cfg(target_os = "macos")]
fn symbol_glyph(symbol: &SharedString, size: gpui::Pixels, color: Hsla) -> AnyElement {
    SymbolGlyph::new(symbol.clone(), size, color).into_any_element()
}

#[cfg(not(target_os = "macos"))]
fn symbol_glyph(symbol: &SharedString, size: gpui::Pixels, color: Hsla) -> AnyElement {
    IconGlyph::new(
        Icon::from_symbol(symbol).unwrap_or(Icon::Puzzle),
        size,
        color,
    )
    .fit()
    .into_any_element()
}

fn swatches(colors: &[Hsla], metrics: Metrics) -> gpui::Div {
    let cell = metrics.scaled((layout::TILE - 8.0) / 2.0);
    let gap = px(1.0);
    let cell_div = |color: Hsla| div().size(cell).rounded(px(2.0)).bg(color);
    let mut grid = div().flex().flex_col().gap(gap);
    for pair in colors.chunks(2).take(2) {
        let mut row = div().flex().gap(gap);
        for color in pair {
            row = row.child(cell_div(*color));
        }
        grid = grid.child(row);
    }
    grid
}
