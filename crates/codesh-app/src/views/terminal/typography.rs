use std::sync::Arc;

use codesh_app_core::settings::TerminalSettings;
use gpui::{Font, FontFallbacks, FontFeatures, Pixels, Size, TextRun, Window, font, px, size};

/// The terminal and native code panes share font resolution and cell metrics.
pub(crate) fn resolve(
    settings: &TerminalSettings,
    window: &Window,
) -> (Font, Pixels, Size<Pixels>) {
    let font_size = px(settings.font_size);
    let mut face = font(
        settings
            .font_families
            .first()
            .map_or_else(|| "Menlo".into(), |family| codesh_ui::font::family(family)),
    );
    face.fallbacks = Some(FontFallbacks::from_fonts(
        settings
            .font_families
            .iter()
            .skip(1)
            .map(|family| codesh_ui::font::family(family).into())
            .collect(),
    ));
    face.features = FontFeatures(Arc::new(settings.font.features.clone()));
    let sample = window.text_system().shape_line(
        "M".into(),
        font_size,
        &[TextRun {
            len: 1,
            font: face.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }],
        None,
    );
    let cell = size(
        px(settings
            .font
            .cell_width
            .apply(f32::from(sample.width), window.scale_factor())),
        px(settings.cell_height.apply(
            f32::from(sample.ascent + sample.descent),
            window.scale_factor(),
        )),
    );
    (face, font_size, cell)
}
