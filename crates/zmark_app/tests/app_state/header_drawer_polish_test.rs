use zmark_app::state::{AppState, ViewMode};
use zmark_app::theme::ThemeColors;
use zmark_app::views::workspace::{adaptive_header_padding, mode_tab_colors};
use zmark_core::document::Document;
use zmark_core::i18n::{get_strings, SupportedLocale};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_header_view_mode_states_and_warm_pill_palette() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

    // Initial default view mode is Split
    assert_eq!(state.view_mode, ViewMode::Split);

    // Transition to Source mode
    state.set_view_mode(ViewMode::Source);
    assert_eq!(state.view_mode, ViewMode::Source);

    // Transition to Preview mode
    state.set_view_mode(ViewMode::Preview);
    assert_eq!(state.view_mode, ViewMode::Preview);

    // Transition back to Split mode
    state.set_view_mode(ViewMode::Split);
    assert_eq!(state.view_mode, ViewMode::Split);

    // ThemeColors selection token verification (ADR 0036 Ticket G4)
    let light_theme = ThemeColors::resolve("default", false);
    let light_bg = light_theme.selection_bg();
    let light_text = light_theme.selection_text();

    assert!(light_bg.h >= 25.0 / 360.0 && light_bg.h <= 45.0 / 360.0, "Default light selection_bg must have warm hue");
    assert!(light_bg.s >= 0.30, "Default light selection_bg must have warm saturation");
    assert!(light_bg.a >= 0.10 && light_bg.a <= 0.20, "Default light selection_bg must have subtle warm opacity");
    assert_eq!(light_text, light_theme.text_primary);

    let dark_theme = ThemeColors::resolve("default", true);
    let dark_bg = dark_theme.selection_bg();
    let dark_text = dark_theme.selection_text();

    assert!(dark_bg.h >= 25.0 / 360.0 && dark_bg.h <= 45.0 / 360.0, "Default dark selection_bg must have warm hue");
    assert!(dark_bg.s >= 0.30, "Default dark selection_bg must have warm saturation");
    assert!(dark_bg.a >= 0.10 && dark_bg.a <= 0.20, "Default dark selection_bg must have subtle warm opacity");
    assert_eq!(dark_text, dark_theme.text_primary);

    // Cross-mode distinctness
    assert_ne!(light_bg, dark_bg, "Light and dark selection backgrounds must differ");
    assert_ne!(light_text, dark_text, "Light and dark selection texts must differ");

    // Verify consistency across registered palettes per Phase H
    for palette in zmark_app::theme::ALL_PALETTES {
        let l_theme = ThemeColors::resolve(palette.id, false);
        let d_theme = ThemeColors::resolve(palette.id, true);

        if palette.id == "default" || palette.id == "claude" {
            let l_bg = l_theme.selection_bg();
            let d_bg = d_theme.selection_bg();
            assert!(l_bg.h >= 25.0 / 360.0 && l_bg.h <= 45.0 / 360.0);
            assert!(l_bg.s >= 0.30);
            assert!(d_bg.h >= 25.0 / 360.0 && d_bg.h <= 45.0 / 360.0);
            assert!(d_bg.s >= 0.30);
        } else {
            assert_eq!(l_theme.selection_bg(), l_theme.text_primary.opacity(0.07));
            assert_eq!(d_theme.selection_bg(), d_theme.text_primary.opacity(0.07));
        }
        assert_eq!(l_theme.selection_text(), l_theme.text_primary);
        assert_eq!(d_theme.selection_text(), d_theme.text_primary);
    }

    // Direct verification of mode_tab_colors production function (P1 requirement)
    for is_dark in [false, true] {
        let theme = ThemeColors::default_palette(is_dark);

        let (active_bg, active_text) = mode_tab_colors(true, &theme);
        let (_inactive_bg, inactive_text) = mode_tab_colors(false, &theme);

        // Active background MUST be selection_bg, NOT theme.surface
        assert_eq!(
            active_bg,
            theme.selection_bg(),
            "Active mode tab background must equal selection_bg"
        );
        assert_ne!(
            active_bg,
            theme.surface,
            "Active mode tab background must NOT be theme.surface"
        );
        assert_eq!(
            active_text,
            theme.selection_text(),
            "Active mode tab text must equal selection_text"
        );
        assert_eq!(
            inactive_text,
            theme.text_muted,
            "Inactive mode tab text must equal text_muted"
        );
    }
}

#[test]
fn test_document_title_display_name_and_i18n_fallback() {
    let dir = tempdir().unwrap();

    // 1. Unnamed scratch document: localized untitled_file fallback
    let scratch_doc = Document::new_scratch();

    // Chinese (Simplified)
    let strings_zh_cn = get_strings(SupportedLocale::ZhCn);
    assert_eq!(strings_zh_cn.untitled_file, "未命名");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_zh_cn.untitled_file),
        "未命名",
        "Chinese locale must return '未命名' for scratch document"
    );

    // Chinese (Traditional)
    let strings_zh_tw = get_strings(SupportedLocale::ZhTw);
    assert_eq!(strings_zh_tw.untitled_file, "未命名");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_zh_tw.untitled_file),
        "未命名"
    );

    // English
    let strings_en = get_strings(SupportedLocale::En);
    assert_eq!(strings_en.untitled_file, "Untitled");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_en.untitled_file),
        "Untitled",
        "English locale must return 'Untitled' for scratch document"
    );

    // Japanese
    let strings_ja = get_strings(SupportedLocale::Ja);
    assert_eq!(strings_ja.untitled_file, "名称未設定");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_ja.untitled_file),
        "名称未設定"
    );

    // German
    let strings_de = get_strings(SupportedLocale::De);
    assert_eq!(strings_de.untitled_file, "Ohne Titel");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_de.untitled_file),
        "Ohne Titel"
    );

    // French
    let strings_fr = get_strings(SupportedLocale::Fr);
    assert_eq!(strings_fr.untitled_file, "Sans titre");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_fr.untitled_file),
        "Sans titre"
    );

    // Spanish
    let strings_es = get_strings(SupportedLocale::Es);
    assert_eq!(strings_es.untitled_file, "Sin título");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_es.untitled_file),
        "Sin título"
    );

    // 2. Document with concrete file path: returns file_name and NOT fallback
    let doc_file = dir.path().join("Architecture_Spec.md");
    fs::write(&doc_file, "# Architecture Spec").unwrap();

    let path_doc = Document::from_file(doc_file).unwrap();
    assert_eq!(
        path_doc.display_name_with_fallback(strings_zh_cn.untitled_file),
        "Architecture_Spec.md",
        "Concrete file path must return file_name, never masked by Chinese fallback"
    );
    assert_eq!(
        path_doc.display_name_with_fallback(strings_en.untitled_file),
        "Architecture_Spec.md",
        "Concrete file path must return file_name, never masked by English fallback"
    );
    assert_eq!(
        path_doc.display_name_with_fallback(strings_ja.untitled_file),
        "Architecture_Spec.md",
        "Concrete file path must return file_name, never masked by Japanese fallback"
    );

    // Deep nested file path
    let nested_file = dir.path().join("nested").join("subfolder").join("custom_design.markdown");
    fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
    fs::write(&nested_file, "# Design").unwrap();

    let nested_doc = Document::from_file(nested_file).unwrap();
    assert_eq!(
        nested_doc.display_name_with_fallback(strings_en.untitled_file),
        "custom_design.markdown",
        "Deeply nested file path must preserve its exact file_name"
    );
}

#[test]
fn mode_tabs_use_fill_only_without_accent_underline() {
    let src = include_str!("../../src/views/workspace.rs");
    let start = src
        .find("fn render_mode_icon_tab")
        .expect("render_mode_icon_tab must exist");
    let rest = &src[start..];
    let end = rest[1..]
        .find("\nfn ")
        .map(|i| i + 1)
        .unwrap_or(rest.len());
    let body = &rest[..end];

    assert!(
        !body.contains("mode_indicator_"),
        "active view-mode tabs must not render an underline indicator"
    );
    assert!(
        !body.contains("theme.accent"),
        "active view-mode tabs must communicate selection with fill, not an accent dot"
    );
}

#[test]
fn test_fullscreen_padding_adaptive_calculation() {
    // Direct assertion on production function: adaptive_header_padding(is_macos, is_fullscreen, drawer_open)

    // 1. Drawer Header (always at leading edge of window, drawer_open = false):
    // macOS non-fullscreen normal window requires 76px to leave safe room for native traffic lights
    assert_eq!(
        adaptive_header_padding(true, false, false),
        76.0,
        "macOS non-fullscreen window leading edge must have 76px left padding"
    );
    // macOS fullscreen window smoothly adapts to 12px
    assert_eq!(
        adaptive_header_padding(true, true, false),
        12.0,
        "macOS fullscreen window leading edge must adapt to 12px left padding"
    );
    // Non-macOS window leading edge always 8px
    assert_eq!(
        adaptive_header_padding(false, false, false),
        8.0,
        "Non-macOS window leading edge must have 8px left padding in normal mode"
    );
    assert_eq!(
        adaptive_header_padding(false, true, false),
        8.0,
        "Non-macOS window leading edge must have 8px left padding in fullscreen mode"
    );

    // 2. Canvas Header (contextual based on drawer open/collapsed state):
    // When drawer is open on macOS, canvas header receives standard 8px padding
    assert_eq!(
        adaptive_header_padding(true, false, true),
        8.0,
        "When drawer is open on macOS, canvas header needs only standard 8px padding"
    );
    assert_eq!(
        adaptive_header_padding(true, true, true),
        8.0,
        "When drawer is open in fullscreen on macOS, canvas header needs only standard 8px padding"
    );
    // When drawer is collapsed on macOS, canvas header absorbs the traffic light safe padding
    assert_eq!(
        adaptive_header_padding(true, false, false),
        76.0,
        "When drawer is collapsed on macOS, canvas header needs 76px padding"
    );
    assert_eq!(
        adaptive_header_padding(true, true, false),
        12.0,
        "When drawer is collapsed in fullscreen on macOS, canvas header needs 12px padding"
    );

    // On non-macOS (Windows / Linux), drawer open or collapsed always yields 8px for canvas header
    assert_eq!(adaptive_header_padding(false, false, false), 8.0);
    assert_eq!(adaptive_header_padding(false, true, false), 8.0);
    assert_eq!(adaptive_header_padding(false, false, true), 8.0);
    assert_eq!(adaptive_header_padding(false, true, true), 8.0);
}
