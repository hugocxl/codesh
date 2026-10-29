use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use codesh_app_core::settings::Appearance;
use codesh_ui::theme::{ColorScheme, Theme};
use gpui::{Window, WindowAppearance};

use crate::views::terminal::colors::Palette;

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) scheme: ColorScheme,
}

impl Entry {
    pub(crate) fn swatches(&self) -> Vec<gpui::Hsla> {
        (0..16)
            .filter_map(|slot| self.scheme.palette_color(slot).map(Into::into))
            .collect()
    }
}

#[derive(Debug)]
pub(crate) struct Catalog {
    pub(crate) entries: Vec<Entry>,
    pub(crate) errors: Vec<String>,
}

impl Catalog {
    /// Themes in Codesh's directory replace the bundled ones.
    pub(crate) fn load(directory: &Path) -> Self {
        let mut entries: BTreeMap<_, _> = codesh_ui::assets::Assets::themes()
            .map(|(name, source)| (name.to_owned(), ColorScheme::parse(source)))
            .collect();
        let mut errors = Vec::new();
        if let Err(error) = fs::create_dir_all(directory)
            .and_then(|()| Self::read_directory(directory, &mut entries, &mut errors))
        {
            errors.push(format!(
                "Could not load themes from {}: {}",
                directory.display(),
                error
            ));
        }
        let mut entries: Vec<_> = entries
            .into_iter()
            .map(|(name, scheme)| Entry { name, scheme })
            .collect();
        entries.sort_by_key(|entry| {
            (
                !matches!(entry.name.as_str(), "Codesh" | "Codesh Light"),
                entry.name.to_lowercase(),
                entry.name.clone(),
            )
        });
        Self { entries, errors }
    }

    fn read_directory(
        directory: &Path,
        entries: &mut BTreeMap<String, ColorScheme>,
        errors: &mut Vec<String>,
    ) -> std::io::Result<()> {
        let mut paths = fs::read_dir(directory)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.sort();
        for path in paths {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name.starts_with('.') || !path.is_file() {
                continue;
            }
            match fs::read_to_string(&path) {
                Ok(source) => {
                    let scheme = ColorScheme::parse(&source);
                    if scheme.background.is_none() || scheme.foreground.is_none() {
                        errors.push(format!(
                            "Theme {name} needs valid background and foreground colors"
                        ));
                        continue;
                    }
                    entries.insert(theme_name(name).to_owned(), scheme);
                }
                Err(error) => {
                    errors.push(format!("Could not read theme {name}: {error}"));
                }
            }
        }
        Ok(())
    }

    fn selected(&self, appearance: &Appearance, dark: bool) -> Option<&Entry> {
        let name = if dark {
            &appearance.dark_theme
        } else {
            &appearance.light_theme
        };
        let fallback = if dark { "Codesh" } else { "Codesh Light" };
        self.find(name).or_else(|| self.find(fallback))
    }

    /// Accepts a theme's file name too, such as `Dracula.conf`.
    fn find(&self, name: &str) -> Option<&Entry> {
        [name, theme_name(name)]
            .into_iter()
            .find_map(|name| self.entries.iter().find(|entry| entry.name == name))
    }

    pub(crate) fn active_name(&self, appearance: &Appearance, dark: bool) -> String {
        self.selected(appearance, dark)
            .map_or_else(String::new, |entry| entry.name.clone())
    }

    pub(crate) fn resolve(&self, appearance: &Appearance, dark: bool) -> (Theme, Palette) {
        let scheme = self
            .selected(appearance, dark)
            .map_or_else(ColorScheme::default, |entry| entry.scheme.clone());
        (
            Theme::from_scheme(&scheme),
            Palette::from_scheme(&scheme, dark),
        )
    }
}

fn theme_name(file: &str) -> &str {
    file.strip_suffix(".conf")
        .or_else(|| file.strip_suffix(".theme"))
        .unwrap_or(file)
}

pub(crate) fn is_dark(window: &Window) -> bool {
    matches!(
        window.appearance(),
        WindowAppearance::Dark | WindowAppearance::VibrantDark
    )
}
