use codesh_app_core::settings::Appearance;
use codesh_ui::theme::Theme;
use gpui::Hsla;
#[cfg(all(target_os = "macos", not(test)))]
use gpui::Window;

use crate::model::AppModel;

impl AppModel {
    #[cfg(all(target_os = "macos", not(test)))]
    pub(crate) fn sync_sidebar_vibrancy(&mut self, window: &Window) {
        if enabled(&self.appearance) {
            // Codesh blurs the whole window background, not just the sidebar.
            let width = f32::from(window.viewport_size().width);
            if let Some(effect) = &mut self.sidebar_vibrancy {
                effect.set_width(width);
                effect.set_appearance(self.theme.bg);
            } else {
                self.sidebar_vibrancy =
                    codesh_ui::vibrancy::SidebarVibrancy::new(window, width, self.theme.bg);
            }
        } else {
            self.sidebar_vibrancy = None;
        }
    }

    pub(crate) fn sidebar_background(&self) -> Hsla {
        #[cfg(target_os = "macos")]
        let available = self.sidebar_vibrancy.is_some();
        #[cfg(not(target_os = "macos"))]
        let available = false;
        background(&self.appearance, &self.theme, available)
    }
}

fn enabled(appearance: &Appearance) -> bool {
    appearance.sidebar_vibrancy && appearance.sidebar_vibrancy_level > 0
}

fn background(appearance: &Appearance, theme: &Theme, available: bool) -> Hsla {
    // Codesh keeps the sidebar flat on the window colour unless the native
    // effect is on; the standard sidebar raises it instead.
    if !available || !enabled(appearance) {
        return crate::views::codesh::style::chrome(theme);
    }
    let mut color = crate::views::codesh::style::chrome(theme);
    color.a = 1.0 - f32::from(appearance.sidebar_vibrancy_level.min(100)) / 100.0;
    color
}
