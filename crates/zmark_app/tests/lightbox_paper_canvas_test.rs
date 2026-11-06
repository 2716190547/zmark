use zmark_app::theme::ThemeColors;

/// Test 1: Lightbox source-level design system assertions
/// Verifies paper canvas styling and high-opacity deep black backdrop in lightbox.rs

/// Test 2: ThemeColors::hover_bg behavior and color contract (ADR 0036 Ticket G0)
#[test]
fn test_theme_hover_bg_properties() {
    let light_theme = ThemeColors::default_palette(false);
    let dark_theme = ThemeColors::default_palette(true);

    let light_hover = light_theme.hover_bg();
    let dark_hover = dark_theme.hover_bg();

    assert_eq!(
        light_hover,
        light_theme.text_primary.opacity(0.04),
        "light hover_bg must derive text_primary.opacity(0.04)"
    );
    assert_eq!(
        dark_hover,
        dark_theme.text_primary.opacity(0.04),
        "dark hover_bg must derive text_primary.opacity(0.04)"
    );

    // 1. Validate subtle alpha
    assert!(
        light_hover.a > 0.0 && light_hover.a <= 0.1,
        "light hover alpha must be subtle (0.0 < alpha <= 0.1), got {}",
        light_hover.a
    );
    assert!(
        dark_hover.a > 0.0 && dark_hover.a <= 0.1,
        "dark hover alpha must be subtle (0.0 < alpha <= 0.1), got {}",
        dark_hover.a
    );

    // 2. Assert light and dark hover colors are distinct
    assert_ne!(
        light_hover, dark_hover,
        "Light and dark hover colors must be distinct"
    );

    // 3. Test across all registered palettes
    for palette_id in &["default", "claude", "macos", "tokyo-night", "nord", "github", "solarized", "one-dark"] {
        let p_light = ThemeColors::resolve(palette_id, false);
        let p_dark = ThemeColors::resolve(palette_id, true);

        let h_light = p_light.hover_bg();
        let h_dark = p_dark.hover_bg();

        assert_eq!(h_light, p_light.text_primary.opacity(0.04));
        assert_eq!(h_dark, p_dark.text_primary.opacity(0.04));
        assert_ne!(h_light, h_dark, "Palette {} light and dark hover must be distinct", palette_id);
    }
}
