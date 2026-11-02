use glidemark_app::assets::AppAssets;
use gpui::AssetSource;

/// Helper to extract stroke-width value from an SVG XML string
fn extract_stroke_width(svg_content: &str) -> f32 {
    let marker = "stroke-width=\"";
    let pos = svg_content
        .find(marker)
        .unwrap_or_else(|| panic!("stroke-width attribute not found in SVG content"));
    let rest = &svg_content[pos + marker.len()..];
    let end_quote = rest
        .find('"')
        .unwrap_or_else(|| panic!("Closing quote for stroke-width not found"));
    rest[..end_quote]
        .parse::<f32>()
        .unwrap_or_else(|_| panic!("Failed to parse stroke-width as f32: '{}'", &rest[..end_quote]))
}

#[test]
fn test_topbar_settings_button_active_state_is_false() {
    let ws_src = include_str!("../src/views/workspace.rs");

    // Locate the btn_settings button invocation in canvas header
    let btn_pos = ws_src
        .find("\"btn_settings\"")
        .expect("btn_settings invocation must exist in workspace.rs");

    let section_end = ws_src[btn_pos..]
        .find(")")
        .map(|idx| btn_pos + idx + 100)
        .unwrap_or(ws_src.len());
    let btn_section = &ws_src[btn_pos..section_end];

    // Assert that btn_settings passes false for active, not self.settings_window.is_some()
    assert!(
        btn_section.contains("false,"),
        "btn_settings must have active parameter set to false"
    );
    assert!(
        !btn_section.contains("self.settings_window.is_some()"),
        "btn_settings must NOT bind active state to self.settings_window.is_some()"
    );
}

#[test]
fn test_topbar_icons_optical_weight_balance_matrix() {
    // Expected optical balance matrix per ADR 0035 Decision 3:
    // - code.svg: viewBox stroke-width 2.25 -> 1.50px rendered stroke at 16px
    // - columns-2.svg: viewBox stroke-width 2.25 -> 1.50px rendered stroke at 16px
    // - panel-left.svg: viewBox stroke-width 2.25 -> 1.50px rendered stroke at 16px (sparse outer-box icon matching columns-2)
    // - download.svg: viewBox stroke-width 2.25 -> 1.50px rendered stroke at 16px
    // - book-open.svg: viewBox stroke-width 2.0 -> 1.33px rendered stroke at 16px
    // - save.svg: viewBox stroke-width 2.0 -> 1.33px rendered stroke at 16px
    // - settings.svg: viewBox stroke-width 1.8 -> 1.20px rendered stroke at 16px
    let icon_matrix: &[(&str, f32, f32)] = &[
        ("icons/code.svg", 2.25, 1.50),
        ("icons/columns-2.svg", 2.25, 1.50),
        ("icons/panel-left.svg", 2.25, 1.50),
        ("icons/download.svg", 2.25, 1.50),
        ("icons/book-open.svg", 2.0, 1.3333334),
        ("icons/save.svg", 2.0, 1.3333334),
        ("icons/settings.svg", 1.8, 1.20),
    ];

    let assets = AppAssets;
    let scale_factor: f32 = 16.0 / 24.0; // 2/3

    for &(icon_path, expected_viewbox_stroke, expected_rendered_stroke) in icon_matrix {
        let loaded = assets
            .load(icon_path)
            .unwrap_or_else(|_| panic!("Failed to load {}", icon_path))
            .unwrap_or_else(|| panic!("Asset not found: {}", icon_path));

        let svg_str = std::str::from_utf8(&loaded)
            .unwrap_or_else(|_| panic!("SVG is not valid utf-8: {}", icon_path));

        let stroke_w = extract_stroke_width(svg_str);
        assert_eq!(
            stroke_w, expected_viewbox_stroke,
            "{} must have viewBox stroke-width {}, got {}",
            icon_path, expected_viewbox_stroke, stroke_w
        );

        let rendered_stroke = stroke_w * scale_factor;
        assert!(
            (rendered_stroke - expected_rendered_stroke).abs() < 1e-4,
            "{} rendered stroke at 16px must be ~{}, got {}",
            icon_path, expected_rendered_stroke, rendered_stroke
        );
    }
}
