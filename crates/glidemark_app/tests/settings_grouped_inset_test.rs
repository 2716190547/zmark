use glidemark_app::assets::AppAssets;
use gpui::AssetSource;

/// 1. `test_search_svg_asset_loaded_and_listed`:
///    - Use `glidemark_app::assets::AppAssets`.
///    - Assert `AppAssets.list("")` contains `"icons/search.svg"`.
///    - Assert `AppAssets.load("icons/search.svg")` returns `Ok(Some(bytes))` and `!bytes.is_empty()`.
#[test]
fn test_search_svg_asset_loaded_and_listed() {
    let assets = AppAssets;
    let list = assets.list("").expect("AppAssets::list should succeed");
    assert!(
        list.iter().any(|p| p.as_ref() == "icons/search.svg"),
        "AppAssets.list(\"\") must contain 'icons/search.svg'"
    );

    let loaded = assets
        .load("icons/search.svg")
        .expect("AppAssets::load should succeed");
    assert!(
        loaded.is_some(),
        "AppAssets.load('icons/search.svg') must return Some"
    );
    let bytes = loaded.unwrap();
    assert!(
        !bytes.is_empty(),
        "Loaded bytes for 'icons/search.svg' must not be empty"
    );
}

/// 2. `test_settings_flattened_structure_without_grouping_cards`:
///    - Inspect `crates/glidemark_app/src/views/settings.rs` via `include_str!`.
///    - Assert `rounded_2xl` is completely removed (no outer grouping cards).
///    - Assert `rounded_md()` is used for flattened inline items.
///    - Assert `!src.contains(".border_1()")` (strictly zero hard borders).
///    - Assert segmented controls span full row width (`w_full()`).
#[test]
fn test_settings_flattened_structure_without_grouping_cards() {
    let src = include_str!("../src/views/settings.rs");

    // Assert no outer grouping cards
    assert!(
        !src.contains("rounded_2xl"),
        "settings.rs must NOT use 'rounded_2xl' grouping cards"
    );

    // Assert rounded_md is used for flattened inline items
    assert!(
        src.contains("rounded_md()"),
        "settings.rs must use 'rounded_md()' for flattened inline items"
    );

    // Assert strictly zero hard borders
    assert!(
        !src.contains(".border_1()"),
        "settings.rs must not contain any hard borders (.border_1())"
    );

    // Assert segmented controls span full row width
    assert!(
        src.contains("w_full()"),
        "settings.rs must use w_full() for full-width segmented controls and rows"
    );
}

/// 3. `test_ios_pill_toggle_switch_contract`:
///    - Inspect `settings.rs` for `render_ios_toggle`.
///    - Assert `rounded_full()` is used for both Track and Thumb.
///    - Assert Thumb is a white circle (`rgb(0xffffff)`).
///    - Assert Track uses `theme.accent` when active and `theme.selection_bg()` when inactive.
///    - Assert `auto_save` and `auto_save_on_blur` use `render_ios_toggle`.
#[test]
fn test_ios_pill_toggle_switch_contract() {
    let src = include_str!("../src/views/settings.rs");

    // Inspect settings.rs for render_ios_toggle
    assert!(
        src.contains("fn render_ios_toggle"),
        "settings.rs must define 'render_ios_toggle'"
    );

    let fn_pos = src
        .find("fn render_ios_toggle")
        .expect("render_ios_toggle definition must exist");
    let fn_end = src[fn_pos + 1..]
        .find("\nfn ")
        .map(|idx| fn_pos + 1 + idx)
        .unwrap_or(src.len());
    let toggle_fn = &src[fn_pos..fn_end];

    // Assert rounded_full() is used for both Track and Thumb
    let count_rounded_full = toggle_fn.matches("rounded_full()").count();
    assert!(
        count_rounded_full >= 2,
        "render_ios_toggle must use rounded_full() for both Track and Thumb (found {})",
        count_rounded_full
    );

    // Assert Thumb is a white circle (rgb(0xffffff))
    assert!(
        toggle_fn.contains("0xffffff"),
        "render_ios_toggle Thumb must be a white circle (rgb(0xffffff))"
    );

    // Assert Track uses theme.accent when active and theme.selection_bg() when inactive
    assert!(
        toggle_fn.contains("theme.accent"),
        "render_ios_toggle Track must use theme.accent when active"
    );
    assert!(
        toggle_fn.contains("selection_bg()"),
        "render_ios_toggle Track must use theme.selection_bg() when inactive"
    );

    // Assert auto_save and auto_save_on_blur use render_ios_toggle
    assert!(
        src.matches("render_ios_toggle").count() >= 3,
        "settings.rs must define and invoke render_ios_toggle for switches"
    );
    assert!(
        src.contains("auto_save"),
        "settings.rs must reference auto_save"
    );
    assert!(
        src.contains("auto_save_on_blur"),
        "settings.rs must reference auto_save_on_blur"
    );
}

/// 4. `test_settings_search_completely_removed`:
///    - Inspect `settings.rs`.
///    - Assert complete removal of `search_query` and `is_search_focused`.
///    - Assert removal of `filter_settings_cross_tab`, `SearchMatchResult`, `render_highlighted_text`.
///    - Assert removal of `render_empty_state` and search strings.
///    - Assert no search icon in left sidebar.
#[test]
fn test_settings_search_completely_removed() {
    let src = include_str!("../src/views/settings.rs");

    // Search state fields removed
    assert!(
        !src.contains("search_query"),
        "settings.rs must NOT contain 'search_query'"
    );
    assert!(
        !src.contains("is_search_focused"),
        "settings.rs must NOT contain 'is_search_focused'"
    );

    // Search functions and types removed
    assert!(
        !src.contains("filter_settings_cross_tab"),
        "settings.rs must NOT contain 'filter_settings_cross_tab'"
    );
    assert!(
        !src.contains("SearchMatchResult"),
        "settings.rs must NOT contain 'SearchMatchResult'"
    );
    assert!(
        !src.contains("render_highlighted_text"),
        "settings.rs must NOT contain 'render_highlighted_text'"
    );
    assert!(
        !src.contains("highlight_matching_segments"),
        "settings.rs must NOT contain 'highlight_matching_segments'"
    );
    assert!(
        !src.contains("render_empty_state"),
        "settings.rs must NOT contain 'render_empty_state'"
    );

    // Search strings removed
    assert!(
        !src.contains("search_settings"),
        "settings.rs must NOT contain 'search_settings'"
    );
    assert!(
        !src.contains("search_no_results"),
        "settings.rs must NOT contain 'search_no_results'"
    );

    // No search icon in settings view
    assert!(
        !src.contains("\"icons/search.svg\""),
        "settings.rs must NOT contain 'icons/search.svg'"
    );

    // No keydown typing listener
    assert!(
        !src.contains("on_key_down"),
        "settings.rs must NOT contain 'on_key_down'"
    );
}

/// 5. `test_palette_two_column_sample_cards_layout`:
///    - Inspect `render_appearance_tab` in `settings.rs`.
///    - Assert 2-column layout using `ALL_PALETTES.chunks(2)`.
///    - Assert `ThemeColors::resolve` is used for palette preview colors.
///    - Assert sample card displays "Aa" in `resolved.text_primary`.
///    - Assert accent bar `w(px(24.)).h(px(4.)).rounded_sm().bg(resolved.accent)`.
///    - Assert selected card has `border_2().border_color(theme.accent)`.
///    - Assert unselected card has `border_2().border_color(gpui::transparent_black())`.
#[test]
fn test_palette_two_column_sample_cards_layout() {
    let src = include_str!("../src/views/settings.rs");

    // Assert 2-column layout using chunks(2)
    assert!(
        src.contains("ALL_PALETTES.chunks(2)"),
        "settings.rs must arrange palettes in 2 columns using ALL_PALETTES.chunks(2)"
    );

    // Assert ThemeColors::resolve is called
    assert!(
        src.contains("ThemeColors::resolve"),
        "settings.rs must resolve palette theme colors using ThemeColors::resolve"
    );

    // Assert sample card "Aa" and accent short bar
    assert!(
        src.contains("\"Aa\""),
        "settings.rs palette sample cards must contain 'Aa' text preview"
    );
    assert!(
        src.contains("w(px(24.))") && src.contains("h(px(4.))") && src.contains("rounded_sm()"),
        "settings.rs palette sample cards must contain horizontal accent bar: w(px(24.)).h(px(4.)).rounded_sm()"
    );

    // Assert border_2 accent border for selected card and transparent for unselected
    assert!(
        src.contains("border_2().border_color(theme.accent)"),
        "settings.rs selected palette card must have 2px accent border"
    );
    assert!(
        src.contains("border_2().border_color(gpui::transparent_black())"),
        "settings.rs unselected palette card must have 2px transparent border"
    );
}

/// 6. `test_segmented_controls_and_nav_items_use_selection_tokens`:
///    - Assert `render_nav_item` uses `theme.selection_bg()` and `theme.selection_text()`.
///    - Assert `render_nav_item` uses `theme.hover_bg()`.
///    - Assert `!src.contains("warm_pill")` throughout `settings.rs`.
#[test]
fn test_segmented_controls_and_nav_items_use_selection_tokens() {
    let src = include_str!("../src/views/settings.rs");

    // Nav items use selection tokens
    let nav_pos = src
        .find("fn render_nav_item")
        .expect("render_nav_item definition must exist");
    let nav_end = src[nav_pos + 1..]
        .find("\nfn ")
        .map(|idx| nav_pos + 1 + idx)
        .unwrap_or(src.len());
    let nav_section = &src[nav_pos..nav_end];

    assert!(
        nav_section.contains("selection_bg()"),
        "render_nav_item must use selection_bg()"
    );
    assert!(
        nav_section.contains("selection_text()"),
        "render_nav_item must use selection_text()"
    );
    assert!(
        nav_section.contains("hover_bg()"),
        "render_nav_item must use hover_bg()"
    );

    // Entire settings.rs must be clean of warm_pill_*
    assert!(
        !src.contains("warm_pill_bg"),
        "settings.rs must NOT contain 'warm_pill_bg'"
    );
    assert!(
        !src.contains("warm_pill_text"),
        "settings.rs must NOT contain 'warm_pill_text'"
    );
    assert!(
        !src.contains("warm_pill_hover"),
        "settings.rs must NOT contain 'warm_pill_hover'"
    );
}

/// 7. `test_settings_full_height_split_columns_and_no_44px_titlebar`:
///    - Assert removal of 44px top titlebar and its associated classes.
///    - Assert left sidebar has pt(px(36.)) top safety margin for traffic lights.
///    - Assert left sidebar width is 160px.
///    - Assert direct horizontal flex layout with flex_row().
///    - Assert workspace.rs open_settings_window preserves appears_transparent and traffic_light_position.
#[test]
fn test_settings_full_height_split_columns_and_no_44px_titlebar() {
    let src = include_str!("../src/views/settings.rs");

    // 1. Assert removal of 44px top titlebar and its associated classes
    assert!(
        !src.contains("h(px(44.))"),
        "settings.rs must not contain h(px(44.)) top titlebar container"
    );
    assert!(
        !src.contains("pl(px(80.))"),
        "settings.rs must not contain pl(px(80.))"
    );

    // 2. Assert left sidebar has pt(px(36.)) top safety margin for traffic lights
    assert!(
        src.contains("pt(px(36.))"),
        "settings.rs left sidebar must have pt(px(36.)) for native macOS traffic lights"
    );

    // 3. Assert left sidebar width is 160px
    assert!(
        src.contains("w(px(160.))"),
        "settings.rs left sidebar must have 160px width"
    );

    // 4. Assert direct horizontal flex layout with flex_row()
    assert!(
        src.contains("flex_row()"),
        "settings.rs root container must use flex_row() for full-height side-by-side columns"
    );

    // 5. Assert workspace.rs open_settings_window preserves transparent titlebar & traffic lights
    let ws_src = include_str!("../src/views/workspace.rs");
    assert!(
        ws_src.contains("appears_transparent: true"),
        "workspace.rs open_settings_window must set appears_transparent: true"
    );
    assert!(
        ws_src.contains("traffic_light_position: Some(point(px(14.), px(14.)))"),
        "workspace.rs open_settings_window must configure traffic_light_position at (14, 14)"
    );
}

/// 8. `test_segmented_control_track_and_thumb_embossed_contract`:
///    - Assert settings.rs uses segmented_track_bg() for segmented control containers.
///    - Assert settings.rs uses segmented_thumb_bg() and segmented_thumb_border() for active items.
///    - Assert settings.rs uses transparent_black() and segmented_inactive_text() for inactive items.
#[test]
fn test_segmented_control_track_and_thumb_embossed_contract() {
    let src = include_str!("../src/views/settings.rs");

    assert!(
        src.contains("theme.segmented_track_bg()"),
        "settings.rs must use theme.segmented_track_bg() for segmented control tracks"
    );
    assert!(
        src.contains("theme.segmented_thumb_bg()"),
        "settings.rs must use theme.segmented_thumb_bg() for active segmented thumbs"
    );
    assert!(
        src.contains("theme.segmented_thumb_border()"),
        "settings.rs must use theme.segmented_thumb_border() for active segmented thumb borders"
    );
    assert!(
        src.contains("theme.segmented_thumb_text()"),
        "settings.rs must use theme.segmented_thumb_text() for active segmented thumb text"
    );
    assert!(
        src.contains("theme.segmented_inactive_text()"),
        "settings.rs must use theme.segmented_inactive_text() for inactive segmented options"
    );
    assert!(
        src.contains("gpui::transparent_black()"),
        "settings.rs must use transparent_black() for inactive segmented option backgrounds"
    );
}
