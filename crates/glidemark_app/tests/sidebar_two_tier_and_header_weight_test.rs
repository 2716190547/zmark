use glidemark_app::assets::AppAssets;
use glidemark_app::state::{AppState, DocKey};
use glidemark_app::views::workspace::{adaptive_header_padding, CANVAS_TITLE_LEFT_MARGIN};
use glidemark_core::document::Document;
use glidemark_core::i18n::{get_strings, SupportedLocale};
use gpui::AssetSource;
use tempfile::tempdir;

#[test]
fn test_assets_registration_for_folder_open() {
    let assets = AppAssets;
    let list = assets.list("").expect("AppAssets::list should succeed");
    assert!(
        list.iter().any(|p| p.as_ref() == "icons/folder-open.svg"),
        "AppAssets::list must include 'icons/folder-open.svg'"
    );

    let loaded = assets
        .load("icons/folder-open.svg")
        .expect("AppAssets::load should succeed");
    assert!(
        loaded.is_some(),
        "AppAssets::load('icons/folder-open.svg') must return Some"
    );
    let bytes = loaded.unwrap();
    assert!(
        !bytes.is_empty(),
        "Loaded bytes for folder-open.svg must not be empty"
    );
}

#[test]
fn test_dynamic_folder_open_icon_and_hover_new_file_action() {
    let src = include_str!("../src/views/drawer.rs");

    // Dynamic folder icons
    assert!(
        src.contains("icons/folder-open.svg"),
        "drawer.rs must reference icons/folder-open.svg for open folder state"
    );
    assert!(
        src.contains("icons/folder.svg"),
        "drawer.rs must reference icons/folder.svg for closed folder state"
    );

    // New file action buttons
    assert!(
        src.contains("btn_new_file_root_"),
        "drawer.rs must include btn_new_file_root_ on root hover"
    );
    assert!(
        src.contains("btn_new_file_dir_"),
        "drawer.rs must include btn_new_file_dir_ on directory hover"
    );

    // Hover child is "+" for new file creation
    assert!(
        src.contains(".child(\"+\")"),
        "drawer.rs right hover action must render '+' for new file creation"
    );

    // Root removal button removed from hover
    assert!(
        !src.contains("btn_remove_root_"),
        "btn_remove_root_ must be removed from hover to prevent accidental root removal"
    );

    // Protected root removal via right click
    assert!(
        src.contains("MouseButton::Right"),
        "Root removal must be protected via MouseButton::Right"
    );

    // Hover "+" tooltip binds strings.new_file (not draft_create_hint)
    assert!(
        src.contains("strings.new_file"),
        "drawer.rs must contain strings.new_file"
    );
    let root_btn_pos = src
        .find("btn_new_file_root_")
        .expect("drawer.rs must define btn_new_file_root_");
    let root_btn_section = &src[root_btn_pos..(root_btn_pos + 1500).min(src.len())];
    assert!(
        root_btn_section.contains("strings.new_file"),
        "btn_new_file_root_ tooltip must use strings.new_file"
    );
    assert!(
        !root_btn_section.contains("draft_create_hint"),
        "btn_new_file_root_ tooltip must not use draft_create_hint"
    );

    let dir_btn_pos = src
        .find("btn_new_file_dir_")
        .expect("drawer.rs must define btn_new_file_dir_");
    let dir_btn_section = &src[dir_btn_pos..(dir_btn_pos + 1500).min(src.len())];
    assert!(
        dir_btn_section.contains("strings.new_file"),
        "btn_new_file_dir_ tooltip must use strings.new_file"
    );
    assert!(
        !dir_btn_section.contains("draft_create_hint"),
        "btn_new_file_dir_ tooltip must not use draft_create_hint"
    );
}

#[test]
fn test_secondary_confirmation_for_root_removal() {
    let drawer_src = include_str!("../src/views/drawer.rs");
    assert!(
        drawer_src.contains("MouseButton::Right"),
        "drawer.rs must handle MouseButton::Right on root header"
    );

    let workspace_src = include_str!("../src/views/workspace.rs");
    assert!(
        workspace_src.contains("render_remove_root_modal"),
        "workspace.rs must define render_remove_root_modal"
    );
    assert!(
        workspace_src.contains("remove_root_modal_card"),
        "workspace.rs must define remove_root_modal_card element id"
    );
    assert!(
        workspace_src.contains("btn_remove_root_confirm"),
        "workspace.rs must define btn_remove_root_confirm"
    );
    assert!(
        workspace_src.contains("btn_remove_root_cancel"),
        "workspace.rs must define btn_remove_root_cancel"
    );

    // Functional integration test on AppState
    let temp_dir = tempdir().expect("Failed to create tempdir");
    let mut state = AppState::new_isolated(Document::new_scratch());

    let add_res = state.add_workspace_root(temp_dir.path().to_path_buf());
    assert!(add_res.is_ok(), "add_workspace_root should succeed");
    assert_eq!(state.workspace_roots.len(), 1, "Should have 1 workspace root");

    // Request removal of root 0
    state.request_remove_workspace_root(0);
    assert_eq!(
        state.pending_remove_root,
        Some(0),
        "pending_remove_root should be Some(0) after request_remove_workspace_root(0)"
    );

    // Cancel removal
    state.cancel_remove_workspace_root();
    assert_eq!(
        state.pending_remove_root, None,
        "pending_remove_root should be None after cancel"
    );
    assert_eq!(
        state.workspace_roots.len(),
        1,
        "Workspace roots must remain untouched after cancel"
    );

    // Request removal again and confirm
    state.request_remove_workspace_root(0);
    assert_eq!(state.pending_remove_root, Some(0));
    let _ = state.confirm_remove_workspace_root();
    assert_eq!(
        state.pending_remove_root, None,
        "pending_remove_root should be None after confirm"
    );
    assert!(
        state.workspace_roots.is_empty(),
        "Workspace roots must be empty after confirming root removal"
    );
}

#[test]
fn test_ten_languages_completeness_for_new_file_and_remove_root_confirm() {
    for (locale, code, name) in SupportedLocale::all() {
        let t = get_strings(*locale);
        assert!(
            !t.new_file.is_empty(),
            "new_file must not be empty for locale {} ({})",
            code,
            name
        );
        assert!(
            !t.remove_root_confirm.is_empty(),
            "remove_root_confirm must not be empty for locale {} ({})",
            code,
            name
        );
        assert!(
            !t.remove_root_confirm_prompt.is_empty(),
            "remove_root_confirm_prompt must not be empty for locale {} ({})",
            code,
            name
        );
        assert!(
            t.remove_root_confirm_prompt.contains("{}"),
            "remove_root_confirm_prompt must contain '{{}}' placeholder for root name in locale {} ({})",
            code,
            name
        );
        assert!(
            !t.remove_root_confirm_detail.is_empty(),
            "remove_root_confirm_detail must not be empty for locale {} ({})",
            code,
            name
        );
    }
}

#[test]
fn test_workspace_header_18px_bold_and_16px_toolbar_icons() {
    let src = include_str!("../src/views/workspace.rs");

    // 1. canvas_document_title and tab_rename_box use .text_size(px(18.)) and FontWeight::BOLD
    let title_pos = src
        .find("canvas_document_title")
        .expect("workspace.rs must define canvas_document_title");
    let title_section = &src[title_pos..(title_pos + 2500).min(src.len())];
    assert!(
        title_section.contains(".text_size(px(18.))"),
        "canvas_document_title must use .text_size(px(18.))"
    );
    assert!(
        title_section.contains("FontWeight::BOLD"),
        "canvas_document_title must use FontWeight::BOLD"
    );

    let rename_pos = src
        .find("tab_rename_box")
        .expect("workspace.rs must define tab_rename_box");
    let rename_section = &src[rename_pos..(rename_pos + 2500).min(src.len())];
    assert!(
        rename_section.contains(".text_size(px(18.))"),
        "tab_rename_box must use .text_size(px(18.))"
    );
    assert!(
        rename_section.contains("FontWeight::BOLD"),
        "tab_rename_box must use FontWeight::BOLD"
    );

    // 2. Toolbar icon helper functions must specify .size(px(16.))
    let icon_fns = [
        "render_source_mode_icon",
        "render_split_mode_icon",
        "render_preview_mode_icon",
        "render_export_icon",
        "render_save_icon",
        "render_settings_icon",
        "render_sidebar_icon",
    ];
    for fn_name in icon_fns {
        let fn_decl = format!("fn {}", fn_name);
        let pos = src
            .find(&fn_decl)
            .unwrap_or_else(|| panic!("workspace.rs must define {}", fn_name));
        let fn_section = &src[pos..(pos + 200).min(src.len())];
        assert!(
            fn_section.contains(".size(px(16.))"),
            "{} must specify .size(px(16.))",
            fn_name
        );
    }

    // 3. Traffic light safe zone math assertions
    let is_macos = true;
    let is_fullscreen = false;

    // Collapsed drawer: 76.0 (macOS inset) + 28.0 (btn_expand_drawer) + 6.0 (gap_1p5) + 8.0 (px_2) == 118.0
    let padding_collapsed = adaptive_header_padding(is_macos, is_fullscreen, false);
    assert_eq!(padding_collapsed, 76.0);
    let expand_btn_w = 28.0;
    let gap_w = 6.0;
    let title_inner_padding = 8.0;
    let total_collapsed =
        padding_collapsed + expand_btn_w + gap_w + title_inner_padding;
    assert_eq!(
        total_collapsed, 118.0,
        "Collapsed drawer safe zone math: 76 + 28 + 6 + 8 == 118"
    );

    // Open drawer: 8.0 (header pl) + 8.0 (px_2) == 16.0 (CANVAS_TITLE_LEFT_MARGIN, aligns with content margin)
    assert_eq!(CANVAS_TITLE_LEFT_MARGIN, 16.0);
    let padding_open = adaptive_header_padding(is_macos, is_fullscreen, true);
    assert_eq!(padding_open, 8.0);
    let total_open = padding_open + title_inner_padding;
    assert_eq!(
        total_open, CANVAS_TITLE_LEFT_MARGIN,
        "Open drawer alignment math: 8 + 8 == 16 (CANVAS_TITLE_LEFT_MARGIN)"
    );
}

#[test]
fn test_app_state_create_file_in_dir_lifecycle() {
    let temp_dir = tempdir().expect("Failed to create tempdir");
    let mut state = AppState::new_isolated(Document::new_scratch());

    // 1. Create first file in temp directory
    let res1 = state.create_file_in_dir(temp_dir.path());
    assert!(res1.is_ok(), "create_file_in_dir should succeed");
    let file1_path = res1.unwrap();
    let expected_path1 = temp_dir.path().join("Untitled.md");
    assert_eq!(
        file1_path, expected_path1,
        "First created file must be Untitled.md"
    );
    assert!(
        file1_path.exists(),
        "Created file must exist on disk at {:?}",
        file1_path
    );

    // Assert state.active is Some(DocKey::Path(...))
    assert_eq!(
        state.active,
        Some(DocKey::Path(expected_path1.clone())),
        "Active doc must point to the newly created file path"
    );

    // Assert state.open_docs contains the new document
    assert!(
        state
            .open_docs
            .iter()
            .any(|d| d.key == DocKey::Path(expected_path1.clone())),
        "open_docs must contain the new document"
    );

    // 2. Sequential naming avoids overwriting: create second file in temp directory
    let res2 = state.create_file_in_dir(temp_dir.path());
    assert!(res2.is_ok(), "create_file_in_dir second call should succeed");
    let file2_path = res2.unwrap();
    let expected_path2 = temp_dir.path().join("Untitled 1.md");
    assert_eq!(
        file2_path, expected_path2,
        "Second created file must be Untitled 1.md"
    );
    assert!(
        file2_path.exists(),
        "Second created file must exist on disk at {:?}",
        file2_path
    );
    assert_eq!(
        state.active,
        Some(DocKey::Path(expected_path2.clone())),
        "Active doc must point to the second created file path"
    );
    assert!(
        state
            .open_docs
            .iter()
            .any(|d| d.key == DocKey::Path(expected_path2.clone())),
        "open_docs must contain the second document"
    );

    // 3. Error case: calling on non-existent folder returns Err
    let non_existent_folder = temp_dir.path().join("non_existent_folder");
    let err_res = state.create_file_in_dir(&non_existent_folder);
    assert!(
        err_res.is_err(),
        "create_file_in_dir on non-existent directory must return Err"
    );
}

#[test]
fn test_file_tree_scan_populates_mtime_cached() {
    let temp_dir = tempdir().expect("Failed to create tempdir");
    let test_file = temp_dir.path().join("Notes.md");
    std::fs::write(&test_file, "# Notes\n\nSome text").expect("write test file");

    let sub_dir = temp_dir.path().join("SubFolder");
    std::fs::create_dir(&sub_dir).expect("create subdir");

    let mut budget = glidemark_app::state::ScanBudget::new(100, 100);
    let expanded = std::collections::HashSet::new();
    let (nodes, _truncated) = glidemark_app::state::scan_root_layer(
        temp_dir.path(),
        0,
        &mut budget,
        &expanded,
        2,
    );

    // Markdown file node has cached mtime in YYYY/MM/DD HH:mm format
    let file_node = nodes.iter().find(|n| !n.is_dir).expect("markdown file node must exist");
    assert_eq!(file_node.name, "Notes.md");
    assert!(file_node.mtime.is_some(), "Scanned markdown file node must cache mtime string");
    let mtime_str = file_node.mtime.as_ref().unwrap();
    assert_eq!(mtime_str.len(), 16, "mtime string must be 16 chars: YYYY/MM/DD HH:mm");
    assert_eq!(&mtime_str[4..5], "/");
    assert_eq!(&mtime_str[7..8], "/");
    assert_eq!(&mtime_str[10..11], " ");
    assert_eq!(&mtime_str[13..14], ":");

    // Directory node has mtime None
    let dir_node = nodes.iter().find(|n| n.is_dir).expect("dir node must exist");
    assert_eq!(dir_node.name, "SubFolder");
    assert!(dir_node.mtime.is_none(), "Directory node must have None mtime");
}

#[test]
fn test_format_mtime_format_and_civil_calculation() {
    let now = std::time::SystemTime::now();
    let formatted = glidemark_app::state::format_mtime(Some(now), now).expect("format_mtime should succeed");
    assert_eq!(formatted.len(), 16, "formatted now must be 16 characters");
    assert_eq!(&formatted[4..5], "/");
    assert_eq!(&formatted[7..8], "/");
    assert_eq!(&formatted[10..11], " ");
    assert_eq!(&formatted[13..14], ":");

    // Epoch timestamp
    let epoch = std::time::SystemTime::UNIX_EPOCH;
    let epoch_formatted = glidemark_app::state::format_mtime(Some(epoch), now).expect("format epoch");
    assert_eq!(epoch_formatted.len(), 16);
}
