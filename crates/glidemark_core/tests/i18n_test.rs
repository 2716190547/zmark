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
        assert!(!t.new_file.is_empty(), "new_file missing in {}", tag);
        assert!(!t.remove_root_confirm.is_empty(), "remove_root_confirm missing in {}", tag);
        assert!(!t.remove_root_confirm_prompt.is_empty(), "remove_root_confirm_prompt missing in {}", tag);
        assert!(!t.remove_root_confirm_detail.is_empty(), "remove_root_confirm_detail missing in {}", tag);
        assert!(!t.draft_unopened.is_empty(), "draft_unopened missing in {}", tag);
        assert!(!t.draft_focus_hint.is_empty(), "draft_focus_hint missing in {}", tag);
        assert!(!t.draft_create_hint.is_empty(), "draft_create_hint missing in {}", tag);
        assert!(!t.workspace_empty.is_empty(), "workspace_empty missing in {}", tag);
        assert!(!t.no_markdown_files.is_empty(), "no_markdown_files missing in {}", tag);
        assert!(!t.untitled_file.is_empty(), "untitled_file missing in {}", tag);
        assert!(!t.collapse_sidebar.is_empty(), "collapse_sidebar missing in {}", tag);
        assert!(!t.modal_save_prompt.is_empty(), "modal_save_prompt missing in {}", tag);
        assert!(!t.modal_save_detail.is_empty(), "modal_save_detail missing in {}", tag);
        assert!(!t.modal_cancel.is_empty(), "modal_cancel missing in {}", tag);
        assert!(!t.modal_discard.is_empty(), "modal_discard missing in {}", tag);
        assert!(!t.modal_save.is_empty(), "modal_save missing in {}", tag);
        assert!(!t.no_document_open.is_empty(), "no_document_open missing in {}", tag);
        assert!(!t.no_document_open_desc.is_empty(), "no_document_open_desc missing in {}", tag);
        assert!(!t.draft_unsaved_hint.is_empty(), "draft_unsaved_hint missing in {}", tag);
        assert!(!t.error_root_not_found.is_empty(), "error_root_not_found missing in {}", tag);
        assert!(!t.error_root_not_dir.is_empty(), "error_root_not_dir missing in {}", tag);
        assert!(!t.error_root_already_exists.is_empty(), "error_root_already_exists missing in {}", tag);
        assert!(!t.error_root_nested.is_empty(), "error_root_nested missing in {}", tag);
        assert!(!t.error_root_parent.is_empty(), "error_root_parent missing in {}", tag);
        assert!(!t.error_save_failed.is_empty(), "error_save_failed missing in {}", tag);
        assert!(!t.error_remove_failed.is_empty(), "error_remove_failed missing in {}", tag);
        assert!(!t.retry_scan.is_empty(), "retry_scan missing in {}", tag);
        assert!(!t.error_canonicalize_failed.is_empty(), "error_canonicalize_failed missing in {}", tag);
        assert!(!t.mermaid_zoom_hint.is_empty(), "mermaid_zoom_hint missing in {}", tag);
        assert!(!t.lightbox_zoom_in.is_empty(), "lightbox_zoom_in missing in {}", tag);
        assert!(!t.lightbox_zoom_out.is_empty(), "lightbox_zoom_out missing in {}", tag);
        assert!(!t.lightbox_reset.is_empty(), "lightbox_reset missing in {}", tag);
        assert!(!t.lightbox_fit_window.is_empty(), "lightbox_fit_window missing in {}", tag);
        assert!(!t.lightbox_close.is_empty(), "lightbox_close missing in {}", tag);
        assert!(!t.search_settings.is_empty(), "search_settings missing in {}", tag);
        assert!(!t.search_no_results.is_empty(), "search_no_results is empty for {:?}", locale);
        assert!(t.modal_save_prompt.contains("{}"), "modal_save_prompt must contain placeholder in {}", tag);
        assert!(t.remove_root_confirm_prompt.contains("{}"), "remove_root_confirm_prompt must contain placeholder in {}", tag);
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
    assert_eq!(t.new_file, "新建文件");
    assert_eq!(t.remove_root_confirm, "确认移除");
    assert_eq!(t.remove_root_confirm_prompt, "确定要从工作区移除“{}”吗？");
    assert_eq!(t.remove_root_confirm_detail, "此操作仅从列表中移除工作区，不会删除磁盘上的实际文件。");
    assert_eq!(t.modal_cancel, "取消");
    assert_eq!(t.modal_discard, "不保存");
    assert_eq!(t.modal_save, "保存");
    assert_eq!(t.no_document_open, "未打开文档");
    assert_eq!(t.retry_scan, "重新扫描");
    assert_eq!(t.error_canonicalize_failed, "路径无效或无法规范化");
    assert_eq!(t.mermaid_zoom_hint, "点击放大查看流程图");
    assert_eq!(t.lightbox_zoom_in, "放大");
    assert_eq!(t.lightbox_zoom_out, "缩小");
    assert_eq!(t.lightbox_reset, "重置");
    assert_eq!(t.lightbox_fit_window, "适应窗口");
    assert_eq!(t.lightbox_close, "关闭灯箱");
    assert_eq!(t.search_settings, "搜索设置");
    assert_eq!(t.search_no_results, "未找到相关设置");
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
