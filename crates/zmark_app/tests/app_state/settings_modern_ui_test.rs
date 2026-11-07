use zmark_app::state::AppState;
use zmark_core::config::Preferences;
use zmark_core::document::Document;
use zmark_core::i18n::{get_strings, SupportedLocale};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn load_settings_source() -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| "crates/zmark_app".to_string());
    let path = Path::new(&manifest_dir).join("src/views/settings.rs");
    if path.exists() {
        fs::read_to_string(&path).expect("failed to read settings.rs from manifest dir")
    } else {
        fs::read_to_string("crates/zmark_app/src/views/settings.rs")
            .or_else(|_| fs::read_to_string("src/views/settings.rs"))
            .expect("failed to locate settings.rs")
    }
}

#[test]
fn test_settings_source_has_no_hard_borders_on_nav_mode_and_palettes() {
    let src = load_settings_source();

    // 0. Verify zero occurrences of .border_1() in entire settings.rs
    assert!(
        !src.contains(".border_1()"),
        "settings.rs must not contain any .border_1() hard borders"
    );

    // 1. Verify render_nav_item has no .border_1() and uses selection tokens
    let nav_pos = src
        .find("fn render_nav_item")
        .expect("render_nav_item definition must exist");
    let nav_end = src[nav_pos + 1..]
        .find("\nfn ")
        .map(|idx| nav_pos + 1 + idx)
        .unwrap_or(src.len());
    let nav_section = &src[nav_pos..nav_end];

    assert!(
        !nav_section.contains(".border_1()"),
        "render_nav_item must not contain .border_1() hard borders"
    );
    assert!(
        nav_section.contains("selection_bg"),
        "render_nav_item must use selection_bg"
    );
    assert!(
        nav_section.contains("selection_text"),
        "render_nav_item must use selection_text"
    );

    // 2. Verify render_mode_pill has no .border_1() and uses segmented derived tokens (Ticket F2)
    let mode_pos = src
        .find("fn render_mode_pill")
        .expect("render_mode_pill definition must exist");
    let mode_end = src[mode_pos + 1..]
        .find("\nfn ")
        .map(|idx| mode_pos + 1 + idx)
        .unwrap_or(src.len());
    let mode_section = &src[mode_pos..mode_end];

    assert!(
        !mode_section.contains(".border_1()"),
        "render_mode_pill must not contain .border_1() hard borders"
    );
    assert!(
        mode_section.contains("segmented_thumb_bg"),
        "render_mode_pill must use segmented_thumb_bg"
    );
    assert!(
        mode_section.contains("segmented_thumb_text"),
        "render_mode_pill must use segmented_thumb_text"
    );
    assert!(
        mode_section.contains("segmented_thumb_border"),
        "render_mode_pill must use segmented_thumb_border"
    );
    assert!(
        mode_section.contains("transparent_black"),
        "render_mode_pill must use transparent_black for inactive items"
    );

    // 3. Verify palette list loop in render_appearance_tab has no .border_1() on palette cards, uses sample cards with 2px accent border for active
    let app_tab_pos = src
        .find("fn render_appearance_tab")
        .expect("render_appearance_tab definition must exist");
    let app_tab_end = src[app_tab_pos + 1..]
        .find("\nfn ")
        .map(|idx| app_tab_pos + 1 + idx)
        .unwrap_or(src.len());
    let app_tab_section = &src[app_tab_pos..app_tab_end];

    let palette_loop_pos = app_tab_section
        .find("ALL_PALETTES")
        .expect("ALL_PALETTES loop must exist in render_appearance_tab");
    let palette_loop_section = &app_tab_section[palette_loop_pos..];

    assert!(
        !palette_loop_section.contains(".border_1()"),
        "Palette cards in render_appearance_tab must not contain .border_1() hard borders"
    );
    assert!(
        palette_loop_section.contains("border_2().border_color(theme.accent)"),
        "Palette cards in render_appearance_tab must use border_2().border_color(theme.accent) for active card"
    );
    assert!(
        palette_loop_section.contains("chunks(2)"),
        "Palette cards in render_appearance_tab must use chunks(2) for 2-column layout"
    );

    // 4. Verify language cards in render_language_tab use selection tokens
    let lang_tab_pos = src
        .find("fn render_language_tab")
        .expect("render_language_tab definition must exist");
    let lang_tab_end = src[lang_tab_pos + 1..]
        .find("\nfn ")
        .map(|idx| lang_tab_pos + 1 + idx)
        .unwrap_or(src.len());
    let lang_tab_section = &src[lang_tab_pos..lang_tab_end];

    assert!(
        lang_tab_section.contains("selection_bg"),
        "Language cards in render_language_tab must use selection_bg"
    );
    assert!(
        lang_tab_section.contains("selection_text"),
        "Language cards in render_language_tab must use selection_text"
    );

    // 5. Verify auto_save and tab_size toggles in render_editor_save_tab use selection tokens and segmented tokens
    let editor_tab_pos = src
        .find("fn render_editor_save_tab")
        .expect("render_editor_save_tab definition must exist");
    let editor_tab_end = src[editor_tab_pos + 1..]
        .find("\nfn ")
        .map(|idx| editor_tab_pos + 1 + idx)
        .unwrap_or(src.len());
    let editor_tab_section = &src[editor_tab_pos..editor_tab_end];

    let auto_save_pos = editor_tab_section
        .find("auto_save_label")
        .expect("auto_save_label must exist");
    let tab_size_pos = editor_tab_section
        .find("tab_size_label")
        .expect("tab_size_label must exist");
    let line_height_pos = editor_tab_section
        .find("line_height_label")
        .expect("line_height_label must exist");

    let auto_save_section = &editor_tab_section[auto_save_pos..tab_size_pos];
    assert!(
        auto_save_section.contains("render_ios_toggle"),
        "auto_save toggle must use render_ios_toggle"
    );

    let tab_size_section = &editor_tab_section[tab_size_pos..line_height_pos];
    assert!(
        tab_size_section.contains("segmented_thumb_bg"),
        "tab_size toggle must use segmented_thumb_bg"
    );
    assert!(
        tab_size_section.contains("segmented_thumb_text"),
        "tab_size toggle must use segmented_thumb_text"
    );
    assert!(
        tab_size_section.contains("segmented_track_bg"),
        "tab_size toggle container must use segmented_track_bg"
    );

    // 6. Verify zero warm_pill usages in entire settings.rs
    assert!(
        !src.contains("warm_pill_bg"),
        "settings.rs must not contain warm_pill_bg"
    );
    assert!(
        !src.contains("warm_pill_text"),
        "settings.rs must not contain warm_pill_text"
    );
    assert!(
        !src.contains("warm_pill_hover"),
        "settings.rs must not contain warm_pill_hover"
    );
}

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
                // In light mode: track_bg opacity is 0.06
                assert_eq!(track_bg.a, 0.06);
                // thumb_bg is surface
                assert_eq!(thumb_bg.l, theme.surface.l);
                // thumb_border opacity is 0.45
                assert_eq!(thumb_border.a, 0.45);
            }

            assert_eq!(thumb_text, theme.text_primary);
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

#[test]
fn test_settings_layout_two_columns_without_top_bar() {
    let src = load_settings_source();

    // Verify 44px top horizontal bar is thoroughly removed
    assert!(
        !src.contains("h(px(44.))"),
        "settings.rs must not contain h(px(44.)) top titlebar"
    );
    assert!(
        !src.contains("pl(px(80.))"),
        "settings.rs must not contain pl(px(80.))"
    );

    // Verify root layout has direct flex_row
    assert!(
        src.contains("flex_row()"),
        "settings.rs must use flex_row() for direct two-column layout"
    );

    // Verify sidebar has 36px top padding for native macOS traffic lights
    assert!(
        src.contains("pt(px(36.))"),
        "settings.rs left sidebar must have pt(px(36.))"
    );
}
