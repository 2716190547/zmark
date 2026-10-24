use glidemark_core::i18n::{get_strings, SupportedLocale};

#[test]
fn test_all_10_locales_have_complete_translations() {
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
        let t = get_strings(locale);
        assert!(!t.app_title.is_empty());
        assert!(!t.open_file.is_empty());
        assert!(!t.save_file.is_empty());
        assert!(!t.export_html.is_empty());
        assert!(!t.source_mode.is_empty());
        assert!(!t.split_mode.is_empty());
        assert!(!t.preview_mode.is_empty());
        assert!(!t.directory_drawer.is_empty());
        assert!(!t.settings.is_empty());
        assert!(!t.palette.is_empty());
        assert!(!t.appearance.is_empty());
        assert!(!t.conflict_warning.is_empty());
        assert!(!t.keep_local.is_empty());
        assert!(!t.reload_disk.is_empty());
    }
}

#[test]
fn test_simplified_chinese_content() {
    let t = get_strings(SupportedLocale::ZhCn);
    assert_eq!(t.open_file, "打开");
    assert_eq!(t.save_file, "保存");
    assert_eq!(t.export_html, "导出 HTML");
    assert_eq!(t.source_mode, "源码");
    assert_eq!(t.split_mode, "分屏");
    assert_eq!(t.preview_mode, "预览");
}

#[test]
fn test_locale_from_tag() {
    assert_eq!(SupportedLocale::from_tag("zh-CN"), SupportedLocale::ZhCn);
    assert_eq!(SupportedLocale::from_tag("zh_CN"), SupportedLocale::ZhCn);
    assert_eq!(SupportedLocale::from_tag("zh-TW"), SupportedLocale::ZhTw);
    assert_eq!(SupportedLocale::from_tag("en-US"), SupportedLocale::En);
    assert_eq!(SupportedLocale::from_tag("ja-JP"), SupportedLocale::Ja);
    assert_eq!(SupportedLocale::from_tag("unknown"), SupportedLocale::En);
}
