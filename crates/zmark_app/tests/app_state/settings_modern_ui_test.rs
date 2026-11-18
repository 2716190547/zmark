use zmark_app::state::AppState;
use zmark_core::config::Preferences;
use zmark_core::document::Document;
use zmark_core::i18n::{get_strings, SupportedLocale};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_theme_colors_segmented_derived_tokens() {
    for is_dark in [false, true] {
        for pal in ["default", "claude", "macos", "tokyo-night", "nord", "github", "solarized", "one-dark"] {
            let theme = zmark_app::theme::ThemeColors::resolve(pal, is_dark);
            let track_bg = theme.segmented_track_bg();
            let thumb_bg = theme.segmented_thumb_bg();
            let thumb_border = theme.segmented_thumb_border();
            let thumb_text = theme.segmented_thumb_text();
            let inactive_text = theme.segmented_inactive_text();

            if is_dark {
                // In dark mode: track_bg has surface lightness + 0.03
                assert!((track_bg.l - (theme.surface.l + 0.03)).abs() < 1e-4);
                // thumb_bg has surface lightness + 0.08
                assert!((thumb_bg.l - (theme.surface.l + 0.08)).abs() < 1e-4);
                // thumb_border opacity is 0.65
                assert_eq!(thumb_border.a, 0.65);
            } else {
                // In light mode: track_bg opacity is 0.12
                assert_eq!(track_bg.a, 0.12);
                // thumb_bg is surface
                assert_eq!(thumb_bg.l, theme.surface.l);
                // thumb_border opacity is 0.45
                assert_eq!(thumb_border.a, 0.45);
            }

            let expected = if pal == "solarized" { gpui::rgb(if is_dark { 0xeee8d5 } else { 0x073642 }).into() } else { theme.text_primary };
            assert_eq!(thumb_text, expected);
            assert_eq!(inactive_text, theme.text_muted);
        }
    }
}

#[test]
fn test_line_height_i18n_all_ten_locales() {
    for &(loc, _tag, _name) in SupportedLocale::all() {
        let strings = get_strings(loc);
        assert!(
            !strings.line_height_label.is_empty(),
            "line_height_label must not be empty for locale {:?}",
            loc
        );

        match loc {
            SupportedLocale::ZhCn => assert_eq!(strings.line_height_label, "行高比率"),
            SupportedLocale::ZhTw => assert_eq!(strings.line_height_label, "行高比率"),
            SupportedLocale::En => assert_eq!(strings.line_height_label, "Line Height"),
            SupportedLocale::Ja => assert_eq!(strings.line_height_label, "行の高さ"),
            SupportedLocale::Ko => assert_eq!(strings.line_height_label, "줄 높이"),
            SupportedLocale::Fr => assert_eq!(strings.line_height_label, "Hauteur de ligne"),
            SupportedLocale::De => assert_eq!(strings.line_height_label, "Zeilenhöhe"),
            SupportedLocale::Es => assert_eq!(strings.line_height_label, "Altura de línea"),
            SupportedLocale::Ru => assert_eq!(strings.line_height_label, "Высота строки"),
            SupportedLocale::Pt => assert_eq!(strings.line_height_label, "Altura da linha"),
            SupportedLocale::Auto => {}
        }
    }
}

#[test]
fn test_app_state_set_line_height_persistence() {
    let dir = tempdir().expect("tempdir");
    let config_path = dir.path().join("config.toml");
    let mut state = AppState::new_with_preferences_at(
        Document::new_scratch(),
        Preferences::default(),
        config_path.clone(),
    );

    // Initial default is 1.6
    assert!((state.preferences.line_height - 1.6).abs() < f32::EPSILON);

    // Set valid line height 1.4
    state.set_line_height(1.4);
    assert!((state.preferences.line_height - 1.4).abs() < f32::EPSILON);

    // Read config from path and verify persistence
    let loaded = Preferences::load_from_path(&config_path).expect("failed to load preferences");
    assert!((loaded.line_height - 1.4).abs() < f32::EPSILON);

    // Out-of-range value 99.0 clamped to 1.6
    state.set_line_height(99.0);
    assert!((state.preferences.line_height - 1.6).abs() < f32::EPSILON);
    let loaded_clamped =
        Preferences::load_from_path(&config_path).expect("failed to load preferences");
    assert!((loaded_clamped.line_height - 1.6).abs() < f32::EPSILON);

    // NaN clamped to 1.6
    state.set_line_height(f32::NAN);
    assert!((state.preferences.line_height - 1.6).abs() < f32::EPSILON);
    let loaded_nan =
        Preferences::load_from_path(&config_path).expect("failed to load preferences");
    assert!((loaded_nan.line_height - 1.6).abs() < f32::EPSILON);
}

#[test]
fn test_backwards_compatibility_without_line_height() {
    let dir = tempdir().expect("tempdir");
    let config_path = dir.path().join("old_config.toml");
    let toml_str = "font_family = 'Monaco'\nfont_size = 14.0\nmax_column_width = 800.0\ntheme = 'dark'\npalette = 'default'\nlocale = 'auto'\ntab_size = 4\n";
    fs::write(&config_path, toml_str).expect("write old config");
    let pref: Preferences =
        Preferences::load_from_path(&config_path).expect("should deserialize");
    assert!((pref.line_height - 1.6).abs() < f32::EPSILON);
}
