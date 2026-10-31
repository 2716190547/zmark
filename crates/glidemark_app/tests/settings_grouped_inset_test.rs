use glidemark_app::assets::AppAssets;
use glidemark_core::i18n::{get_strings, SupportedLocale};
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

/// 2. `test_settings_grouped_inset_cards_and_dividers`:
///    - Inspect `crates/glidemark_app/src/views/settings.rs` via `include_str!`.
///    - Assert `rounded_2xl` is used for Grouped Inset Cards.
///    - Assert hairline divider (`h(px(1.))` or `bg(theme.border)`) is used between card items.
///    - Assert `!src.contains(".border_1()")` (strictly zero hard borders).
#[test]
fn test_settings_grouped_inset_cards_and_dividers() {
    let src = include_str!("../src/views/settings.rs");

    // Assert rounded_2xl is used for Grouped Inset Cards
    assert!(
        src.contains("rounded_2xl"),
        "settings.rs must use 'rounded_2xl' for Grouped Inset Cards"
    );

    // Assert hairline divider (h(px(1.)) or bg(theme.border)) is used between card items
    assert!(
        src.contains(".h(px(1.))") || src.contains("h(px(1.))"),
        "settings.rs must use hairline divider with h(px(1.))"
    );
    assert!(
        src.contains("theme.border"),
        "settings.rs hairline divider must use theme.border"
    );

    // Assert strictly zero hard borders
    assert!(
        !src.contains(".border_1()"),
        "settings.rs must not contain any hard borders (.border_1())"
    );
}

/// 3. `test_ios_pill_toggle_switch_contract`:
///    - Inspect `settings.rs` for `render_ios_toggle`.
///    - Assert `rounded_full()` is used for both Track and Thumb.
///    - Assert Thumb is a white circle (`rgb(0xffffff)`).
///    - Assert Track uses `theme.accent` when active and `theme.warm_pill_bg()` when inactive.
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

    // Assert Track uses theme.accent when active and theme.warm_pill_bg() when inactive
    assert!(
        toggle_fn.contains("theme.accent"),
        "render_ios_toggle Track must use theme.accent when active"
    );
    assert!(
        toggle_fn.contains("warm_pill_bg"),
        "render_ios_toggle Track must use theme.warm_pill_bg() when inactive"
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

/// 4. `test_settings_search_filtering_contract`:
///    - Inspect `settings.rs`.
///    - Assert `"icons/search.svg"` is rendered in the left sidebar.
///    - Assert `search_query` field exists in `SettingsWindowView`.
///    - Assert placeholder binds `strings.search_settings`.
///    - Assert case-insensitive filtering logic exists in tab rendering.
#[test]
fn test_settings_search_filtering_contract() {
    let src = include_str!("../src/views/settings.rs");

    // Assert "icons/search.svg" is rendered in the left sidebar
    assert!(
        src.contains("\"icons/search.svg\""),
        "settings.rs must render 'icons/search.svg' in the left sidebar"
    );

    // Assert search_query field exists in SettingsWindowView
    let struct_pos = src
        .find("pub struct SettingsWindowView")
        .expect("SettingsWindowView struct must exist");
    let struct_end = src[struct_pos..]
        .find('}')
        .map(|idx| struct_pos + idx)
        .expect("SettingsWindowView closing brace");
    let struct_src = &src[struct_pos..struct_end];
    assert!(
        struct_src.contains("search_query"),
        "SettingsWindowView must contain 'search_query' field"
    );

    // Assert placeholder binds strings.search_settings
    assert!(
        src.contains("strings.search_settings"),
        "settings.rs search input placeholder must bind strings.search_settings"
    );

    // Assert case-insensitive filtering logic exists in tab rendering
    assert!(
        src.contains(".to_lowercase()") || src.contains(".to_ascii_lowercase()"),
        "settings.rs must contain case-insensitive filtering logic"
    );
}

/// 5. `test_ten_locales_search_settings_string_completeness`:
///    - Iterate over `glidemark_core::i18n::SupportedLocale::all()`.
///    - Assert `!t.search_settings.is_empty()` for all 10 locales.
///    - Assert exact strings: `ZhCn` -> "搜索设置", `ZhTw` -> "搜尋設定", `En` -> "Search Settings", `Ja` -> "設定を検索".
#[test]
fn test_ten_locales_search_settings_string_completeness() {
    let locales = SupportedLocale::all();
    assert_eq!(
        locales.len(),
        10,
        "SupportedLocale::all() must have exactly 10 locales"
    );

    for &(locale, tag, _name) in locales {
        let t = get_strings(locale);
        assert!(
            !t.search_settings.is_empty(),
            "search_settings string must not be empty for locale {} ({})",
            tag,
            _name
        );
    }

    // Assert exact strings: ZhCn -> "搜索设置", ZhTw -> "搜尋設定", En -> "Search Settings", Ja -> "設定を検索"
    assert_eq!(get_strings(SupportedLocale::ZhCn).search_settings, "搜索设置");
    assert_eq!(get_strings(SupportedLocale::ZhTw).search_settings, "搜尋設定");
    assert_eq!(get_strings(SupportedLocale::En).search_settings, "Search Settings");
    assert_eq!(get_strings(SupportedLocale::Ja).search_settings, "設定を検索");
}
