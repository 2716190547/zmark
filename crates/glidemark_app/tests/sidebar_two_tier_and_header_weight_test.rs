use glidemark_app::assets::AppAssets;
use glidemark_app::state::{AppState, DocKey, ViewMode};
use glidemark_app::views::workspace::{adaptive_header_padding, canvas_title_offset};
use glidemark_core::document::Document;
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
fn test_sidebar_two_tier_typography_and_arrow_removal() {
    let src = include_str!("../src/views/drawer.rs");

    // 1. Assert triangle characters are completely eradicated
    assert!(
        !src.contains("▸") && !src.contains("▾"),
        "Triangle characters ('▸' and '▾') must be completely eradicated from drawer.rs"
    );

    // 2. root_header uses FontWeight::MEDIUM and text_size(px(13.))
    let root_header_pos = src
        .find("drawer_root_header_")
        .expect("drawer.rs must define drawer_root_header_");
    let root_header_section = &src[root_header_pos..(root_header_pos + 1200).min(src.len())];
    assert!(
        root_header_section.contains("FontWeight::MEDIUM"),
        "root_header must use FontWeight::MEDIUM"
    );
    assert!(
        root_header_section.contains(".text_size(px(13.))"),
        "root_header must use text_size(px(13.))"
    );

    // 3. drawer_tree_dir uses FontWeight::MEDIUM and text_size(px(13.))
    let dir_pos = src
        .find("drawer_tree_dir_")
        .expect("drawer.rs must define drawer_tree_dir_");
    let dir_section = &src[dir_pos..(dir_pos + 1200).min(src.len())];
    assert!(
        dir_section.contains("FontWeight::MEDIUM"),
        "drawer_tree_dir must use FontWeight::MEDIUM"
    );
    assert!(
        dir_section.contains(".text_size(px(13.))"),
        "drawer_tree_dir must use text_size(px(13.))"
    );

    // 4. drawer_tree_file uses FontWeight::NORMAL and text_size(px(13.))
    let file_pos = src
        .find("drawer_tree_file_")
        .expect("drawer.rs must define drawer_tree_file_");
    let file_section = &src[file_pos..(file_pos + 1200).min(src.len())];
    assert!(
        file_section.contains("FontWeight::NORMAL"),
        "drawer_tree_file must use FontWeight::NORMAL"
    );
    assert!(
        file_section.contains(".text_size(px(13.))"),
        "drawer_tree_file must use text_size(px(13.))"
    );

    // 5. drawer_draft_item and drawer_loose_file use FontWeight::NORMAL and text_size(px(13.))
    let draft_pos = src
        .find("drawer_draft_item")
        .expect("drawer.rs must define drawer_draft_item");
    let draft_section = &src[draft_pos..(draft_pos + 1200).min(src.len())];
    assert!(
        draft_section.contains("FontWeight::NORMAL"),
        "drawer_draft_item must use FontWeight::NORMAL"
    );
    assert!(
        draft_section.contains(".text_size(px(13.))"),
        "drawer_draft_item must use text_size(px(13.))"
    );

    let loose_pos = src
        .find("drawer_loose_file_")
        .expect("drawer.rs must define drawer_loose_file_");
    let loose_section = &src[loose_pos..(loose_pos + 1200).min(src.len())];
    assert!(
        loose_section.contains("FontWeight::NORMAL"),
        "drawer_loose_file must use FontWeight::NORMAL"
    );
    assert!(
        loose_section.contains(".text_size(px(13.))"),
        "drawer_loose_file must use text_size(px(13.))"
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

    // Collapsed drawer: 76.0 (macOS inset) + 28.0 (btn_expand_drawer) + 6.0 (gap_1p5) + 0.0 (offset) + 8.0 (px_2) == 118.0
    let padding_collapsed = adaptive_header_padding(is_macos, is_fullscreen, false);
    assert_eq!(padding_collapsed, 76.0);
    let expand_btn_w = 28.0;
    let gap_w = 6.0;
    let offset_collapsed = canvas_title_offset(ViewMode::Source, false);
    assert_eq!(offset_collapsed, 0.0);
    let title_inner_padding = 8.0;
    let total_collapsed =
        padding_collapsed + expand_btn_w + gap_w + offset_collapsed + title_inner_padding;
    assert_eq!(
        total_collapsed, 118.0,
        "Collapsed drawer safe zone math: 76 + 28 + 6 + 0 + 8 == 118"
    );

    // Open drawer: 8.0 (header pl) + 48.0 (canvas_title_offset) + 8.0 (px_2) == 64.0 (aligns with code gutter)
    let padding_open = adaptive_header_padding(is_macos, is_fullscreen, true);
    assert_eq!(padding_open, 8.0);
    let offset_open = canvas_title_offset(ViewMode::Source, true);
    assert_eq!(offset_open, 48.0);
    let total_open = padding_open + offset_open + title_inner_padding;
    assert_eq!(
        total_open, 64.0,
        "Open drawer alignment math: 8 + 48 + 8 == 64"
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
