use glidemark_app::assets::AppAssets;
use glidemark_app::views::settings::{
    filter_settings_cross_tab, highlight_matching_segments, matches_auto_save, matches_line_height,
    matches_theme_mode,
};
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

    // Unicode special variable-length case-folding tests:
    // Turkish 'İ' (\u{0130})
    assert_eq!(
        highlight_matching_segments("İpek Yolu", "İpek"),
        vec![("İpek", true), (" Yolu", false)]
    );
    assert_eq!(
        highlight_matching_segments("İpek Yolu", "i\u{0307}pek"),
        vec![("İpek", true), (" Yolu", false)]
    );

    // German 'ß'
    assert_eq!(
        highlight_matching_segments("Straße in Berlin", "straße"),
        vec![("Straße", true), (" in Berlin", false)]
    );

    // French accented vowels
    assert_eq!(
        highlight_matching_segments("Éditeur de texte", "éditeur"),
        vec![("Éditeur", true), (" de texte", false)]
    );

    // Spanish
    assert_eq!(
        highlight_matching_segments("Ajustes de Configuración", "configuración"),
        vec![("Ajustes de ", false), ("Configuración", true)]
    );
}

/// 8. `test_cross_tab_global_search_aggregation`:
///    - Test `glidemark_app::views::settings::filter_settings_cross_tab`.
///    - Verify cross-tab recall across categories (Appearance, Editor & Save, Language, About).
#[test]
fn test_cross_tab_global_search_aggregation() {
    let strings = get_strings(SupportedLocale::En);

    // Test "light":
    let light_results = filter_settings_cross_tab("light", strings);
    assert!(
        !light_results.is_empty(),
        "Results for 'light' must not be empty"
    );
    assert!(
        light_results.iter().any(|r| r.category == "Appearance"),
        "Results for 'light' must contain matches from category 'Appearance'"
    );
    assert!(
        !light_results.iter().any(|r| r.category == "Editor & Save"),
        "Results for 'light' must DO NOT have any matches from category 'Editor & Save'"
    );

    // Test "theme":
    let theme_results = filter_settings_cross_tab("theme", strings);
    assert!(
        !theme_results.is_empty(),
        "Results for 'theme' must not be empty"
    );
    assert!(
        theme_results.iter().any(|r| r.category == "Appearance"),
        "Results for 'theme' must contain matches from category 'Appearance'"
    );

    // Test "mode":
    let mode_results = filter_settings_cross_tab("mode", strings);
    assert!(
        !mode_results.is_empty(),
        "Results for 'mode' must not be empty"
    );
    assert!(
        mode_results.iter().any(|r| r.category == "Appearance"),
        "Results for 'mode' must contain matches from category 'Appearance'"
    );

    // Test "save":
    let save_results = filter_settings_cross_tab("save", strings);
    assert!(
        !save_results.is_empty(),
        "Results for 'save' must not be empty"
    );
    assert!(
        save_results.iter().any(|r| r.category == "Editor & Save"),
        "Results for 'save' must contain matches from category 'Editor & Save'"
    );
    assert!(
        save_results.iter().any(|r| r.category == "About"),
        "Results for 'save' must contain matches from category 'About'"
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

    // Test "nonexistentquery12345":
    let nonexistent_results = filter_settings_cross_tab("nonexistentquery12345", strings);
    assert!(
        nonexistent_results.is_empty(),
        "Results for 'nonexistentquery12345' must be empty"
    );
}

/// 9. `test_render_highlighted_text_structure_and_matching`:
///    - Test `glidemark_app::views::settings::highlight_matching_segments`:
///      - Assert `highlight_matching_segments("Light Mode", "light") == vec![("Light", true), (" Mode", false)]`.
///      - Assert `highlight_matching_segments("Theme Mode", "mode") == vec![("Theme ", false), ("Mode", true)]`.
///    - Test `glidemark_app::views::settings::matches_theme_mode`:
///      - Assert `matches_theme_mode("light", strings)` is true.
///      - Assert `matches_theme_mode("theme", strings)` is true.
///      - Assert `matches_theme_mode("mode", strings)` is true.
///    - Test `glidemark_app::views::settings::matches_line_height`:
///      - Assert `matches_line_height("light", strings)` is FALSE!
///      - Assert `matches_line_height("line", strings)` is true.
///      - Assert `matches_line_height("height", strings)` is true.
///    - Test `glidemark_app::views::settings::matches_auto_save`:
///      - Assert `matches_auto_save("save", strings)` is true.
///      - Assert `matches_auto_save("light", strings)` is false.
#[test]
fn test_render_highlighted_text_structure_and_matching() {
    let strings = get_strings(SupportedLocale::En);

    // Test highlight_matching_segments
    assert_eq!(
        highlight_matching_segments("Light Mode", "light"),
        vec![("Light", true), (" Mode", false)]
    );
    assert_eq!(
        highlight_matching_segments("Theme Mode", "mode"),
        vec![("Theme ", false), ("Mode", true)]
    );

    // Test matches_theme_mode
    assert!(
        matches_theme_mode("light", strings),
        "matches_theme_mode('light', strings) must be true"
    );
    assert!(
        matches_theme_mode("theme", strings),
        "matches_theme_mode('theme', strings) must be true"
    );
    assert!(
        matches_theme_mode("mode", strings),
        "matches_theme_mode('mode', strings) must be true"
    );

    // Test matches_line_height
    assert!(
        !matches_line_height("light", strings),
        "matches_line_height('light', strings) must be false"
    );
    assert!(
        matches_line_height("line", strings),
        "matches_line_height('line', strings) must be true"
    );
    assert!(
        matches_line_height("height", strings),
        "matches_line_height('height', strings) must be true"
    );

    // Test matches_auto_save
    assert!(
        matches_auto_save("save", strings),
        "matches_auto_save('save', strings) must be true"
    );
    assert!(
        !matches_auto_save("light", strings),
        "matches_auto_save('light', strings) must be false"
    );
}

/// 10. `test_settings_search_source_contract`:
///    - Inspect `crates/glidemark_app/src/views/settings.rs` via `include_str!`.
///    - Assert `is_search_focused` field exists in `SettingsWindowView`.
///    - Assert `if !this.is_search_focused` exists inside `on_key_down`.
///    - Assert `render_highlighted_text` is called in `render_appearance_tab`, `render_editor_save_tab`, `render_language_tab`, and `render_about_tab`.
///    - Assert `strings.search_no_results` is present.
///    - Assert `!src.contains("\"No results for\"")`.
///    - Assert `!src.contains(".border_1()")` (strictly zero hard borders).
#[test]
fn test_settings_search_source_contract() {
    let src = include_str!("../src/views/settings.rs");

    // Assert is_search_focused field exists in SettingsWindowView
    let struct_pos = src
        .find("pub struct SettingsWindowView")
        .expect("SettingsWindowView struct must exist");
    let struct_end = src[struct_pos..]
        .find('}')
        .map(|idx| struct_pos + idx)
        .expect("SettingsWindowView closing brace");
    let struct_src = &src[struct_pos..struct_end];
    assert!(
        struct_src.contains("is_search_focused"),
        "SettingsWindowView must contain 'is_search_focused' field"
    );

    // Assert on_key_down gates keyboard events with if !this.is_search_focused
    let on_key_down_pos = src
        .find(".on_key_down")
        .expect("settings.rs must contain .on_key_down listener");
    let on_key_down_slice =
        &src[on_key_down_pos..on_key_down_pos + 400.min(src.len() - on_key_down_pos)];
    assert!(
        on_key_down_slice.contains("if !this.is_search_focused"),
        "settings.rs on_key_down listener must gate keyboard events with 'if !this.is_search_focused'"
    );
    assert!(
        src.contains("if !this.is_search_focused"),
        "settings.rs must contain 'if !this.is_search_focused'"
    );

    // Assert render_highlighted_text is called in render_appearance_tab, render_editor_save_tab, render_language_tab, and render_about_tab
    let app_tab_start = src
        .find("fn render_appearance_tab")
        .expect("render_appearance_tab function must exist");
    let app_tab_end = src[app_tab_start..]
        .find("fn render_editor_save_tab")
        .map(|i| app_tab_start + i)
        .expect("render_editor_save_tab function must exist");
    assert!(
        src[app_tab_start..app_tab_end].contains("render_highlighted_text"),
        "render_appearance_tab must call render_highlighted_text"
    );

    let editor_tab_start = app_tab_end;
    let editor_tab_end = src[editor_tab_start..]
        .find("fn render_language_tab")
        .map(|i| editor_tab_start + i)
        .expect("render_language_tab function must exist");
    assert!(
        src[editor_tab_start..editor_tab_end].contains("render_highlighted_text"),
        "render_editor_save_tab must call render_highlighted_text"
    );

    let lang_tab_start = editor_tab_end;
    let lang_tab_end = src[lang_tab_start..]
        .find("fn render_about_tab")
        .map(|i| lang_tab_start + i)
        .expect("render_about_tab function must exist");
    assert!(
        src[lang_tab_start..lang_tab_end].contains("render_highlighted_text"),
        "render_language_tab must call render_highlighted_text"
    );

    let about_tab_start = lang_tab_end;
    let about_tab_end = src[about_tab_start + 1..]
        .find("\nfn ")
        .map(|i| about_tab_start + 1 + i)
        .unwrap_or(src.len());
    assert!(
        src[about_tab_start..about_tab_end].contains("render_highlighted_text"),
        "render_about_tab must call render_highlighted_text"
    );

    // Assert strings.search_no_results is present
    assert!(
        src.contains("strings.search_no_results"),
        "settings.rs must use 'strings.search_no_results'"
    );

    // Assert !src.contains("\"No results for\"")
    assert!(
        !src.contains("\"No results for\""),
        "settings.rs must NOT contain '\"No results for\"'"
    );
    assert!(
        !src.contains("No results for"),
        "settings.rs must NOT contain 'No results for'"
    );

    // Assert strictly zero hard borders
    assert!(
        !src.contains(".border_1()"),
        "settings.rs must have zero occurrences of '.border_1()'"
    );
}
