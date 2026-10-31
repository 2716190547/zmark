use glidemark_app::theme::ThemeColors;

/// Test 1: Lightbox source-level design system assertions
/// Verifies paper canvas styling and high-opacity deep black backdrop in lightbox.rs
#[test]
fn test_lightbox_paper_canvas_source_styling() {
    let src = include_str!("../src/views/lightbox.rs");

    // 1. Backdrop background uses 0x000000e0 (88% black)
    assert!(
        src.contains("0x000000e0"),
        "lightbox.rs backdrop must use 0x000000e0 (88% deep black)"
    );
    assert!(
        !src.contains("0x00000073"),
        "lightbox.rs must not contain legacy 0x00000073 45% translucent backdrop"
    );

    // 2. lightbox_diagram_card contains rounded_xl, p_4, and shadow_lg
    let card_section_pos = src
        .find("lightbox_diagram_card")
        .expect("lightbox.rs must define lightbox_diagram_card");
    let card_section = &src[card_section_pos..card_section_pos + 400.min(src.len() - card_section_pos)];

    assert!(
        card_section.contains(".rounded_xl()"),
        "lightbox_diagram_card must have .rounded_xl()"
    );
    assert!(
        card_section.contains(".p_4()"),
        "lightbox_diagram_card must have .p_4()"
    );
    assert!(
        card_section.contains(".shadow_lg()"),
        "lightbox_diagram_card must have .shadow_lg()"
    );

    // 3. lightbox_diagram_card sets solid paper canvas background (theme.surface or solid base)
    assert!(
        card_section.contains("theme.surface") || card_section.contains("0xffffff"),
        "lightbox_diagram_card must set a solid paper canvas background (theme.surface or solid base)"
    );
}

/// Test 2: ThemeColors::warm_pill_hover behavior and color contract
#[test]
fn test_theme_warm_pill_hover_properties() {
    let light_theme = ThemeColors::default_palette(false);
    let dark_theme = ThemeColors::default_palette(true);

    let light_hover = light_theme.warm_pill_hover();
    let dark_hover = dark_theme.warm_pill_hover();

    // 1. Validate Hsla range bounds for light theme
    assert!(
        light_hover.h >= 0.0 && light_hover.h <= 1.0,
        "light hover hue must be in 0.0..=1.0, got {}",
        light_hover.h
    );
    assert!(
        light_hover.s >= 0.0 && light_hover.s <= 1.0,
        "light hover saturation must be in 0.0..=1.0, got {}",
        light_hover.s
    );
    assert!(
        light_hover.l >= 0.0 && light_hover.l <= 1.0,
        "light hover lightness must be in 0.0..=1.0, got {}",
        light_hover.l
    );
    assert!(
        light_hover.a > 0.0 && light_hover.a <= 0.1,
        "light hover alpha must be subtle (0.0 < alpha <= 0.1), got {}",
        light_hover.a
    );

    // 2. Validate Hsla range bounds for dark theme
    assert!(
        dark_hover.h >= 0.0 && dark_hover.h <= 1.0,
        "dark hover hue must be in 0.0..=1.0, got {}",
        dark_hover.h
    );
    assert!(
        dark_hover.s >= 0.0 && dark_hover.s <= 1.0,
        "dark hover saturation must be in 0.0..=1.0, got {}",
        dark_hover.s
    );
    assert!(
        dark_hover.l >= 0.0 && dark_hover.l <= 1.0,
        "dark hover lightness must be in 0.0..=1.0, got {}",
        dark_hover.l
    );
    assert!(
        dark_hover.a > 0.0 && dark_hover.a <= 0.1,
        "dark hover alpha must be subtle (0.0 < alpha <= 0.1), got {}",
        dark_hover.a
    );

    // 3. Assert light and dark hover colors are distinct
    assert_ne!(
        light_hover, dark_hover,
        "Light and dark hover colors must be distinct"
    );

    // 4. Test across multiple resolved palettes
    for palette_id in &["default", "claude", "macos", "tokyo-night", "nord", "github", "solarized", "one-dark"] {
        let p_light = ThemeColors::resolve(palette_id, false);
        let p_dark = ThemeColors::resolve(palette_id, true);

        let h_light = p_light.warm_pill_hover();
        let h_dark = p_dark.warm_pill_hover();

        assert!(h_light.a <= 0.1, "Palette {} light hover alpha > 0.1", palette_id);
        assert!(h_dark.a <= 0.1, "Palette {} dark hover alpha > 0.1", palette_id);
        assert_ne!(h_light, h_dark, "Palette {} light and dark hover must be distinct", palette_id);
    }
}
