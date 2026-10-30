use glidemark_core::i18n::{get_strings, SupportedLocale};

#[test]
fn test_all_10_locales_have_complete_translations() {
    for &(locale, tag, _name) in SupportedLocale::all() {
        let t = get_strings(locale);
        assert!(!t.app_title.is_empty(), "app_title missing in {}", tag);
        assert!(!t.open_file.is_empty(), "open_file missing in {}", tag);
        assert!(!t.save_file.is_empty(), "save_file missing in {}", tag);
        assert!(!t.export_html.is_empty(), "export_html missing in {}", tag);
        assert!(!t.source_mode.is_empty(), "source_mode missing in {}", tag);
        assert!(!t.split_mode.is_empty(), "split_mode missing in {}", tag);
        assert!(!t.preview_mode.is_empty(), "preview_mode missing in {}", tag);
        assert!(!t.directory_drawer.is_empty(), "directory_drawer missing in {}", tag);
        assert!(!t.settings.is_empty(), "settings missing in {}", tag);
        assert!(!t.palette.is_empty(), "palette missing in {}", tag);
        assert!(!t.appearance.is_empty(), "appearance missing in {}", tag);
        assert!(!t.conflict_warning.is_empty(), "conflict_warning missing in {}", tag);
        assert!(!t.keep_local.is_empty(), "keep_local missing in {}", tag);
        assert!(!t.reload_disk.is_empty(), "reload_disk missing in {}", tag);
        assert!(!t.add_folder.is_empty(), "add_folder missing in {}", tag);
        assert!(!t.loose_files.is_empty(), "loose_files missing in {}", tag);
        assert!(!t.drafts_group.is_empty(), "drafts_group missing in {}", tag);
        assert!(!t.workspace_group.is_empty(), "workspace_group missing in {}", tag);
        assert!(!t.missing_root.is_empty(), "missing_root missing in {}", tag);
        assert!(!t.truncated_hint.is_empty(), "truncated_hint missing in {}", tag);
        assert!(!t.deferred_hint.is_empty(), "deferred_hint missing in {}", tag);
        assert!(!t.remove_root.is_empty(), "remove_root missing in {}", tag);
        assert!(!t.draft_unopened.is_empty(), "draft_unopened missing in {}", tag);
        assert!(!t.draft_focus_hint.is_empty(), "draft_focus_hint missing in {}", tag);
        assert!(!t.draft_create_hint.is_empty(), "draft_create_hint missing in {}", tag);
        assert!(!t.workspace_empty.is_empty(), "workspace_empty missing in {}", tag);
        assert!(!t.no_markdown_files.is_empty(), "no_markdown_files missing in {}", tag);
        assert!(!t.untitled_file.is_empty(), "untitled_file missing in {}", tag);
        assert!(!t.collapse_sidebar.is_empty(), "collapse_sidebar missing in {}", tag);
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
    assert_eq!(t.draft_unopened, "草稿 (未打开)");
    assert_eq!(t.draft_focus_hint, "聚焦草稿");
    assert_eq!(t.draft_create_hint, "新建草稿");
    assert_eq!(t.workspace_empty, "暂无工作区文件夹");
    assert_eq!(t.no_markdown_files, "无 Markdown 文件");
    assert_eq!(t.untitled_file, "未命名");
    assert_eq!(t.collapse_sidebar, "收起侧栏");
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
