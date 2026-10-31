use glidemark_app::assets::AppAssets;
use glidemark_app::views::settings::{filter_settings_cross_tab, highlight_matching_segments};
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

/// 6. `test_search_no_results_ten_locales`:
///    - Iterate over `SupportedLocale::all()`.
///    - Verify `!t.search_no_results.is_empty()`.
///    - Verify exact translations for all 10 locales.
#[test]
fn test_search_no_results_ten_locales() {
    let locales = SupportedLocale::all();
    assert_eq!(
        locales.len(),
        10,
        "SupportedLocale::all() must have exactly 10 locales"
    );

    for &(locale, tag, _name) in locales {
        let t = get_strings(locale);
        assert!(
            !t.search_no_results.is_empty(),
            "search_no_results string must not be empty for locale {} ({})",
            tag,
            _name
        );
    }

    assert_eq!(
        get_strings(SupportedLocale::ZhCn).search_no_results,
        "未找到相关设置"
    );
    assert_eq!(
        get_strings(SupportedLocale::ZhTw).search_no_results,
        "未找到相關設定"
    );
    assert_eq!(
        get_strings(SupportedLocale::En).search_no_results,
        "No matching settings found"
    );
    assert_eq!(
        get_strings(SupportedLocale::Ja).search_no_results,
        "一致する設定が見つかりません"
    );
    assert_eq!(
        get_strings(SupportedLocale::Ko).search_no_results,
        "일치하는 설정이 없습니다"
    );
    assert_eq!(
        get_strings(SupportedLocale::Fr).search_no_results,
        "Aucun paramètre correspondant trouvé"
    );
    assert_eq!(
        get_strings(SupportedLocale::De).search_no_results,
        "Keine passenden Einstellungen gefunden"
    );
    assert_eq!(
        get_strings(SupportedLocale::Es).search_no_results,
        "No se encontraron ajustes coincidentes"
    );
    assert_eq!(
        get_strings(SupportedLocale::Ru).search_no_results,
        "Настройки не найдены"
    );
    assert_eq!(
        get_strings(SupportedLocale::Pt).search_no_results,
        "Nenhuma definição correspondente encontrada"
    );
}

/// 7. `test_highlight_matching_segments_algorithm`:
///    - Test `glidemark_app::views::settings::highlight_matching_segments`.
///    - Verify exact matches, splitting, multiple matches, non-matches, and empty cases.
#[test]
fn test_highlight_matching_segments_algorithm() {
    assert_eq!(
        highlight_matching_segments("Auto Save When Idle", "save"),
        vec![("Auto ", false), ("Save", true), (" When Idle", false)]
    );

    assert_eq!(
        highlight_matching_segments("auto save", "SAVE"),
        vec![("auto ", false), ("save", true)]
    );

    let segs = highlight_matching_segments("Mono Roboto", "o");
    assert_eq!(
        segs,
        vec![
            ("M", false),
            ("o", true),
            ("n", false),
            ("o", true),
            (" R", false),
            ("o", true),
            ("b", false),
            ("o", true),
            ("t", false),
            ("o", true),
        ]
    );

    assert_eq!(
        highlight_matching_segments("Hello World", "xyz"),
        vec![("Hello World", false)]
    );

    assert_eq!(
        highlight_matching_segments("Hello World", ""),
        vec![("Hello World", false)]
    );

    assert_eq!(
        highlight_matching_segments("", "test"),
        vec![("", false)]
    );
}

/// 8. `test_cross_tab_global_search_aggregation`:
///    - Test `glidemark_app::views::settings::filter_settings_cross_tab`.
///    - Verify cross-tab recall across categories (Appearance, Editor & Save, Language, About).
#[test]
fn test_cross_tab_global_search_aggregation() {
    let strings = get_strings(SupportedLocale::En);

    // For "save":
    let save_results = filter_settings_cross_tab("save", strings);
    assert!(
        save_results.iter().any(|r| r.category == "Editor & Save"),
        "Results for 'save' must contain matches from category 'Editor & Save'"
    );
    assert!(
        save_results.iter().any(|r| r.category == "About"),
        "Results for 'save' must contain matches from category 'About'"
    );

    // For "light":
    let light_results = filter_settings_cross_tab("light", strings);
    assert!(
        light_results.iter().any(|r| r.category == "Appearance"),
        "Results for 'light' must contain matches from category 'Appearance'"
    );
    assert!(
        light_results.iter().any(|r| r.category == "Editor & Save"),
        "Results for 'light' must contain matches from category 'Editor & Save'"
    );

    // For "chinese" or "中文":
    let chinese_results = filter_settings_cross_tab("chinese", strings);
    assert!(
        chinese_results.iter().any(|r| r.category == "Language"),
        "Results for 'chinese' must contain matches from category 'Language'"
    );
    let zh_results = filter_settings_cross_tab("中文", strings);
    assert!(
        zh_results.iter().any(|r| r.category == "Language"),
        "Results for '中文' must contain matches from category 'Language'"
    );

    // For "xyznonexistentquery999":
    let empty_results = filter_settings_cross_tab("xyznonexistentquery999", strings);
    assert!(
        empty_results.is_empty(),
        "Results for 'xyznonexistentquery999' must be empty"
    );
}

/// 9. `test_settings_search_source_contract`:
///    - Inspect `crates/glidemark_app/src/views/settings.rs` via `include_str!`.
///    - Assert it contains `strings.search_no_results`.
///    - Assert it does NOT contain `"No results for"`.
///    - Assert it contains `highlight_matching_segments`.
///    - Assert it contains `filter_settings_cross_tab`.
///    - Assert zero occurrences of `.border_1()`.
#[test]
fn test_settings_search_source_contract() {
    let src = include_str!("../src/views/settings.rs");

    assert!(
        src.contains("strings.search_no_results"),
        "settings.rs must use 'strings.search_no_results'"
    );
    assert!(
        !src.contains("No results for"),
        "settings.rs must NOT contain 'No results for'"
    );
    assert!(
        src.contains("highlight_matching_segments"),
        "settings.rs must contain 'highlight_matching_segments'"
    );
    assert!(
        src.contains("filter_settings_cross_tab"),
        "settings.rs must contain 'filter_settings_cross_tab'"
    );
    assert!(
        !src.contains(".border_1()"),
        "settings.rs must have zero occurrences of '.border_1()'"
    );
}
