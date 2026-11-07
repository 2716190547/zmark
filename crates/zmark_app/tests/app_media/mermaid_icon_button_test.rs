use zmark_app::assets::AppAssets;
use zmark_app::theme::ThemeColors;
use zmark_core::i18n::{get_strings, SupportedLocale};
use gpui::AssetSource;

/// Test 1: Verify preview.rs has 0 occurrences of emoji in mermaid cards/buttons.
#[test]
fn test_mermaid_card_no_emoji() {
    let preview_src = include_str!("../../src/views/preview.rs");

    // Assert that there are 0 occurrences of "🔍" (U+1F50D) in preview.rs
    let search_icon_count = preview_src.matches('🔍').count();
    assert_eq!(
        search_icon_count, 0,
        "preview.rs must contain 0 occurrences of '🔍' (U+1F50D), found {}",
        search_icon_count
    );

    // Assert that there are 0 occurrences of "zoom_hint" formatted with emoji
    for (idx, line) in preview_src.lines().enumerate() {
        if line.contains("mermaid_zoom_hint") {
            assert!(
                !line.contains('🔍'),
                "Line {idx} references mermaid_zoom_hint with emoji: {line}"
            );
            // Verify no high-codepoint emoji characters appear on zoom_hint lines
            for ch in line.chars() {
                assert!(
                    ch < '\u{1F300}' || ch > '\u{1F9FF}',
                    "Line {idx} formatting mermaid_zoom_hint contains emoji '{ch}': {line}"
                );
            }
        }
    }

    // Verify maximize SVG icon is referenced in preview.rs
    assert!(
        preview_src.contains("icons/maximize.svg"),
        "preview.rs must reference 'icons/maximize.svg' for the zoom button"
    );
}

/// Test 2: Verify maximize.svg asset is registered, loadable, and contains valid SVG tags.
#[test]
fn test_maximize_icon_asset_registration() {
    let assets = AppAssets;

    // List assets via AppAssets.list("")
    let list = assets.list("").expect("AppAssets::list should succeed");
    let has_maximize = list.iter().any(|p| p.as_ref() == "icons/maximize.svg");
    assert!(
        has_maximize,
        "AppAssets.list(\"\") must contain 'icons/maximize.svg'"
    );

    // Load maximize.svg
    let loaded = assets
        .load("icons/maximize.svg")
        .expect("AppAssets::load should succeed");
    assert!(
        loaded.is_some(),
        "AppAssets.load(\"icons/maximize.svg\") returned None"
    );

    let bytes = loaded.unwrap();
    assert!(
        !bytes.is_empty(),
        "icons/maximize.svg bytes must not be empty"
    );

    // Verify SVG content
    let svg_text = std::str::from_utf8(&bytes).expect("maximize.svg must be valid UTF-8");
    assert!(
        svg_text.contains("xmlns=\"http://www.w3.org/2000/svg\""),
        "maximize.svg must contain XML namespace attribute"
    );
    assert!(
        svg_text.contains("<svg") && svg_text.contains("<path"),
        "maximize.svg must contain valid <svg> and <path> elements"
    );
}

/// Test 3: Verify all 10 locales have non-empty, emoji-free mermaid_zoom_hint.
#[test]
fn test_mermaid_zoom_hint_i18n() {
    let locales = [
        SupportedLocale::ZhCn,
        SupportedLocale::ZhTw,
        SupportedLocale::En,
        SupportedLocale::Ja,
        SupportedLocale::Ko,
        SupportedLocale::Fr,
        SupportedLocale::De,
        SupportedLocale::Es,
        SupportedLocale::Ru,
        SupportedLocale::Pt,
    ];

    for locale in locales {
        let strings = get_strings(locale);
        let hint = strings.mermaid_zoom_hint;

        // 1. Assert non-empty
        assert!(
            !hint.trim().is_empty(),
            "mermaid_zoom_hint must not be empty for locale {:?}",
            locale
        );

        // 2. Assert contains no emoji
        assert!(
            !hint.contains('🔍'),
            "mermaid_zoom_hint for locale {:?} must not contain '🔍'",
            locale
        );
        for ch in hint.chars() {
            assert!(
                ch < '\u{1F300}' || ch > '\u{1F9FF}',
                "mermaid_zoom_hint for locale {:?} contains emoji '{}': {}",
                locale,
                ch,
                hint
            );
        }

        // 3. Specific assert for ZhCn
        if locale == SupportedLocale::ZhCn {
            assert_eq!(
                hint, "点击放大查看流程图",
                "SupportedLocale::ZhCn mermaid_zoom_hint must equal '点击放大查看流程图'"
            );
        }
    }
}

/// Test 4: Verify selection token styling and contrast for light & dark palettes (ADR 0036 Ticket G3).
#[test]
fn test_mermaid_zoom_button_styling_contrast() {
    // 1. Light Mode Palette
    let light_theme = ThemeColors::default_palette(false);
    let light_bg = light_theme.selection_bg();
    let light_hover = light_theme.hover_bg();
    let light_text = light_theme.selection_text();

    assert!(light_bg.h >= 25.0 / 360.0 && light_bg.h <= 45.0 / 360.0, "selection_bg must have warm hue");
    assert!(light_bg.s >= 0.30, "selection_bg must have warm saturation");
    assert!(light_bg.a >= 0.10 && light_bg.a <= 0.20, "selection_bg must have subtle warm opacity per Phase H");
    assert_eq!(light_hover, light_theme.text_primary.opacity(0.04));
    assert_eq!(light_text, light_theme.text_primary);
    assert_eq!(light_text.a, 1.0, "selection_text must be opaque");

    // 2. Dark Mode Palette
    let dark_theme = ThemeColors::default_palette(true);
    let dark_bg = dark_theme.selection_bg();
    let dark_hover = dark_theme.hover_bg();
    let dark_text = dark_theme.selection_text();

    assert!(dark_bg.h >= 25.0 / 360.0 && dark_bg.h <= 45.0 / 360.0, "selection_bg must have warm hue");
    assert!(dark_bg.s >= 0.30, "selection_bg must have warm saturation");
    assert!(dark_bg.a >= 0.10 && dark_bg.a <= 0.20, "selection_bg must have subtle warm opacity per Phase H");
    assert_eq!(dark_hover, dark_theme.text_primary.opacity(0.04));
    assert_eq!(dark_text, dark_theme.text_primary);
    assert_eq!(dark_text.a, 1.0, "selection_text must be opaque");

    // Cross-mode distinctness
    assert_ne!(
        light_bg, dark_bg,
        "Light and dark selection backgrounds must be distinct"
    );
    assert_ne!(
        light_text, dark_text,
        "Light and dark selection texts must be distinct"
    );
}
