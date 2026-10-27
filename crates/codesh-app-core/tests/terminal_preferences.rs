use codesh_app_core::settings::{
    FontMap, PaddingColor, TerminalAction, TerminalColor, TerminalEdit, TerminalSettings,
};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn invalid_edits_and_corrupted_native_files_preserve_saved_preferences() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("terminal.toml");
    let mut settings = TerminalSettings::load_native(&path)?;
    settings.set_preference("background-opacity", "75")?;
    settings.save_native(&path)?;
    let original = settings.clone();
    let source = std::fs::read_to_string(&path)?;
    for (id, value) in [
        ("background-transparency", "NaN"),
        ("background-vibrancy", "101"),
        ("padding-right", "-1"),
        ("background-opacity", "NaN"),
        ("background-opacity", "101"),
        ("font-size", "0"),
        ("window-padding-x", "-1"),
        ("scroll-precision", "inf"),
        ("adjust-cursor-thickness", "-100%"),
        ("font-family", ""),
    ] {
        assert!(settings.set_preference(id, value).is_err());
        assert_eq!(settings, original);
    }
    settings.options.scroll_discrete = f32::NAN;
    assert!(settings.save_native(&path).is_err());
    assert_eq!(std::fs::read_to_string(&path)?, source);
    std::fs::write(&path, "font_size = 'broken'")?;
    assert!(TerminalSettings::load_native(&path).is_err());
    assert!(original.save_native(&path).is_err());
    assert_eq!(std::fs::read_to_string(path)?, "font_size = 'broken'");
    Ok(())
}

#[test]
#[allow(clippy::float_cmp)]
fn terminal_options_survive_editing_and_native_serialization() -> Result {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("terminal.toml");
    let mut settings = TerminalSettings::load_native(&path)?;
    settings.edit(TerminalEdit::Fallbacks(vec![
        "Fallback".into(),
        "Emoji".into(),
    ]))?;
    settings.set_preference("font-family-bold", "Bold")?;
    settings.set_preference("font-family-italic", "Italic")?;
    settings.set_preference("font-family-bold-italic", "Bold Italic")?;
    settings.set_preference("font-feature", "ss01, -calt, cv01=2,")?;
    settings.set_preference("font-thicken-strength", "128")?;
    settings.edit(TerminalEdit::CodepointMap {
        previous: None,
        map: Some(("U+E000-U+E0FF".into(), "Icons".into())),
    })?;
    settings.edit(TerminalEdit::CodepointMap {
        previous: None,
        map: Some(("U+F000".into(), "Other".into())),
    })?;
    settings.edit(TerminalEdit::CodepointMap {
        previous: Some(FontMap {
            start: 0xe000,
            end: 0xe0ff,
            family: "Icons".into(),
        }),
        map: Some(("U+E000-U+E1FF".into(), "\"Nerd Icons\"".into())),
    })?;
    settings.set_preference("background", "#101010")?;
    settings.set_preference("foreground", "white")?;
    settings.set_preference("cursor-color", "cell-foreground")?;
    settings.set_preference("cursor-text", "#202020")?;
    settings.set_preference("selection-foreground", "#303030")?;
    settings.set_preference("selection-background", "cell-background")?;
    settings.set_preference("palette-1", "#ff0000")?;
    settings.set_preference("palette-2", "#00ff00")?;
    settings.set_preference("palette-2", "")?;
    settings.set_preference("cursor-opacity", "40")?;
    settings.set_preference("window-padding-color", "extend-always")?;
    settings.set_preference("keybind-clear-defaults", "true")?;
    let chord = "ctrl-alt-x".parse()?;
    settings.edit(TerminalEdit::Binding {
        previous: None,
        binding: Some((chord, "text:echo \\e[A\\\\ \\x20".into())),
    })?;
    let renamed = "ctrl-alt-y".parse()?;
    settings.edit(TerminalEdit::Binding {
        previous: Some("ctrl-alt-x".parse()?),
        binding: Some((renamed, "increase_font_size:2".into())),
    })?;
    settings.edit(TerminalEdit::Binding {
        previous: None,
        binding: Some(("cmd-shift-k".parse()?, "text:git checkout ".into())),
    })?;
    settings.save_native(&path)?;

    let restored = TerminalSettings::load_native(&path)?;
    assert_eq!(restored, settings);
    let mut families = TerminalSettings::default().font_families;
    families.extend(["Fallback".into(), "Emoji".into()]);
    assert_eq!(restored.font_families, families);
    assert_eq!(restored.font.feature_list(), "ss01, -calt, cv01=2");
    assert_eq!(
        restored.font.codepoints,
        [
            FontMap {
                start: 0xe000,
                end: 0xe1ff,
                family: "Nerd Icons".into(),
            },
            FontMap {
                start: 0xf000,
                end: 0xf000,
                family: "Other".into(),
            },
        ]
    );
    assert_eq!(restored.options.background, Some(0x10_1010));
    assert_eq!(
        restored.options.cursor_color,
        Some(TerminalColor::CellForeground)
    );
    assert_eq!(restored.options.palette.len(), 1);
    assert_eq!(restored.options.cursor_opacity, 0.4);
    assert_eq!(restored.options.padding_color, PaddingColor::ExtendAlways);
    assert!(restored.keybindings.clear_defaults);
    assert_eq!(restored.keybindings.action(&"ctrl-alt-x".parse()?), None);
    assert_eq!(
        restored.keybindings.action(&"ctrl-alt-y".parse()?),
        Some(&TerminalAction::IncreaseFontSize(2.0))
    );
    assert_eq!(
        restored.keybindings.action(&"cmd-shift-k".parse()?),
        Some(&TerminalAction::Text(b"git checkout ".to_vec()))
    );
    Ok(())
}

#[test]
fn invalid_or_conflicting_terminal_edits_change_nothing() -> Result {
    let mut settings = TerminalSettings::default();
    settings.edit(TerminalEdit::Binding {
        previous: None,
        binding: Some(("ctrl-alt-x".parse()?, "copy_to_clipboard".into())),
    })?;
    settings.edit(TerminalEdit::CodepointMap {
        previous: None,
        map: Some(("U+E000".into(), "Icons".into())),
    })?;
    let original = settings.clone();
    for (id, value) in [
        ("background", "cell-foreground"),
        ("cursor-color", "#12345"),
        ("palette-256", "#ffffff"),
        ("palette-1", "not a color"),
        ("font-feature", "toolong"),
        ("font-family-bold", "Bad\"Name"),
        ("font-thicken-strength", "256"),
        ("cursor-opacity", "101"),
        ("window-padding-color", "sideways"),
        ("keybind-clear-defaults", "maybe"),
    ] {
        assert!(
            settings.set_preference(id, value).is_err(),
            "{id} = {value}"
        );
        assert_eq!(settings, original);
    }
    for edit in [
        TerminalEdit::Fallbacks(vec!["Bad\"Name".into()]),
        TerminalEdit::CodepointMap {
            previous: None,
            map: Some(("U+F000-U+E000".into(), "Icons".into())),
        },
        TerminalEdit::CodepointMap {
            previous: Some(FontMap {
                start: 1,
                end: 1,
                family: "Gone".into(),
            }),
            map: None,
        },
        TerminalEdit::Binding {
            previous: None,
            binding: Some(("ctrl-alt-z".parse()?, "new_window".into())),
        },
        TerminalEdit::Binding {
            previous: None,
            binding: Some(("ctrl-alt-x".parse()?, "paste_from_clipboard".into())),
        },
        TerminalEdit::Binding {
            previous: Some("ctrl-alt-x".parse()?),
            binding: Some(("ctrl-alt-z".parse()?, "text:\\q".into())),
        },
    ] {
        assert!(settings.edit(edit.clone()).is_err(), "{edit:?}");
        assert_eq!(settings, original);
    }
    Ok(())
}
