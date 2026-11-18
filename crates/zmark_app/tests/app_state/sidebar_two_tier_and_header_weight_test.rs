use zmark_app::state::{AppState, DocKey};
use zmark_core::document::Document;
use zmark_core::i18n::{get_strings, SupportedLocale};
use tempfile::tempdir;

#[test]
fn test_secondary_confirmation_for_root_removal() {
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

    // Assert state.active_key().cloned() is Some(DocKey::Path(...))
    assert_eq!(
        state.active_key().cloned(),
        Some(DocKey::Path(expected_path1.canonicalize().unwrap())),
        "Active doc must point to the newly created file path"
    );

    // Assert state.open_documents() contains the new document
    assert!(
        state
            .open_documents()
            .iter()
            .any(|d| d.key == DocKey::Path(expected_path1.canonicalize().unwrap())),
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
        state.active_key().cloned(),
        Some(DocKey::Path(expected_path2.canonicalize().unwrap())),
        "Active doc must point to the second created file path"
    );
    assert!(
        state
            .open_documents()
            .iter()
            .any(|d| d.key == DocKey::Path(expected_path2.canonicalize().unwrap())),
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

    let mut budget = zmark_app::state::ScanBudget::new(100, 100);
    let expanded = std::collections::HashSet::new();
    let (nodes, _truncated) = zmark_app::state::scan_root_layer(
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
