use glidemark_app::state::{AppState, DocKey, FileTreeNode, OpenDoc, WorkspaceRoot};
use glidemark_app::views::drawer::DrawerSearchParams;
use glidemark_core::document::Document;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_set_search_query_and_clear_search() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

    // Initial state
    assert_eq!(state.search_query, "");
    assert!(state.search_original_expanded.is_none());
    assert!(!state.search_in_progress);
    assert!(!state.search_truncated);
    assert!(state.has_search_matches());

    // Setting query
    state.set_search_query("test_file");
    assert_eq!(state.search_query, "test_file");
    assert!(state.search_original_expanded.is_some());

    // Clearing query
    state.clear_search();
    assert_eq!(state.search_query, "");
    assert!(state.search_original_expanded.is_none());
    assert!(!state.search_truncated);
    assert!(state.has_search_matches());
}

#[test]
fn test_case_insensitive_matching() {
    let doc = Document::new_scratch();
    let state = AppState::new_isolated(doc);

    // Empty query matches anything
    assert!(state.file_matches_search("README.md"));
    assert!(state.file_matches_search("anything.txt"));

    let mut state = state;
    // Substring matching is case-insensitive
    state.set_search_query("README");
    assert!(state.file_matches_search("README.md"));
    assert!(state.file_matches_search("readme.md"));
    assert!(state.file_matches_search("ReadMe.markdown"));

    state.set_search_query("readme");
    assert!(state.file_matches_search("README.md"));
    assert!(state.file_matches_search("readme.md"));

    state.set_search_query("Read");
    assert!(state.file_matches_search("README.md"));
    assert!(state.file_matches_search("unread_items.md"));

    state.set_search_query("xyz");
    assert!(!state.file_matches_search("README.md"));
}

#[test]
fn test_matching_across_roots_loose_and_drafts() {
    let tmp = tempdir().unwrap();
    let doc_path = tmp.path().join("main.md");
    fs::write(&doc_path, "# Main Document").unwrap();

    let doc = Document::from_file(doc_path.clone()).unwrap();
    let mut state = AppState::new_isolated(doc);

    // 1. Workspace root matching
    let root_dir = tmp.path().join("workspace");
    fs::create_dir_all(&root_dir).unwrap();
    let ws_file = root_dir.join("workspace_notes.md");
    fs::write(&ws_file, "# Notes").unwrap();
    state.add_workspace_root(root_dir).unwrap();

    state.set_search_query("workspace_notes");
    assert!(state.has_search_matches(), "Should match file in workspace root");

    // 2. Loose docs matching (external file opened)
    let ext_path = tmp.path().join("external_loose.md");
    fs::write(&ext_path, "# External").unwrap();
    let ext_doc = Document::from_file(ext_path.clone()).unwrap();
    state.open_docs.push(OpenDoc {
        key: DocKey::Path(ext_path),
        doc: ext_doc,
    });

    state.set_search_query("external_loose");
    assert!(state.has_search_matches(), "Should match loose doc");

    // 3. Draft matching (scratch doc with display name or heading)
    let mut scratch_doc = Document::new_scratch();
    scratch_doc.update_buffer("# My Daily Scratch\nDraft content".to_string());
    state.open_docs.push(OpenDoc {
        key: DocKey::Scratch(99),
        doc: scratch_doc,
    });

    state.set_search_query("Daily");
    assert!(state.has_search_matches(), "Should match scratch draft title");

    // 4. Non-matching query
    state.set_search_query("completely_nonexistent_token_98765");
    assert!(!state.has_search_matches(), "Should not match nonexistent file");
}

#[test]
fn test_auto_expansion_and_restoration() {
    let tmp = tempdir().unwrap();
    let root_path = tmp.path().join("project");
    let sub1 = root_path.join("src");
    let sub2 = sub1.join("components");
    fs::create_dir_all(&sub2).unwrap();

    let target_file = sub2.join("Button.md");
    fs::write(&target_file, "# Button Component").unwrap();

    let other_dir = root_path.join("tests");
    fs::create_dir_all(&other_dir).unwrap();
    fs::write(other_dir.join("test1.md"), "# Test").unwrap();

    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);
    state.add_workspace_root(root_path.clone()).unwrap();

    // Collapse all folders initially
    for node in state.workspace_roots[0].tree.iter_mut() {
        node.is_expanded = false;
        for child in node.children.iter_mut() {
            child.is_expanded = false;
        }
    }

    let src_node = state.workspace_roots[0].tree.iter().find(|n| n.name == "src").unwrap();
    assert!(!src_node.is_expanded, "src should be collapsed initially");

    // Search for Button -> should auto-expand ancestor directories src and components
    state.set_search_query("Button");
    assert_eq!(state.search_query, "Button");
    assert!(state.has_search_matches());

    let src_node_after = state.workspace_roots[0].tree.iter().find(|n| n.name == "src").unwrap();
    assert!(src_node_after.is_expanded, "src must be auto-expanded during search");

    let comp_node = src_node_after.children.iter().find(|n| n.name == "components").unwrap();
    assert!(comp_node.is_expanded, "components must be auto-expanded during search");

    // Clear search -> original expansion states should be restored
    state.clear_search();
    let src_node_restored = state.workspace_roots[0].tree.iter().find(|n| n.name == "src").unwrap();
    assert!(!src_node_restored.is_expanded, "src must be restored to collapsed after clearing search");
}

#[test]
fn test_restoration_preserves_pre_search_expanded_folders() {
    let tmp = tempdir().unwrap();
    let root_path = tmp.path().join("repo");
    let dir_kept = root_path.join("docs");
    let dir_auto = root_path.join("archive");
    fs::create_dir_all(&dir_kept).unwrap();
    fs::create_dir_all(&dir_auto).unwrap();

    fs::write(dir_kept.join("guide.md"), "# Guide").unwrap();
    fs::write(dir_auto.join("old_spec.md"), "# Old Spec").unwrap();

    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);
    state.add_workspace_root(root_path.clone()).unwrap();

    // Explicitly set docs as expanded and archive as collapsed
    for node in state.workspace_roots[0].tree.iter_mut() {
        if node.name == "docs" {
            node.is_expanded = true;
        } else if node.name == "archive" {
            node.is_expanded = false;
        }
    }

    // Search for "old_spec" in archive
    state.set_search_query("old_spec");

    let archive_node = state.workspace_roots[0].tree.iter().find(|n| n.name == "archive").unwrap();
    assert!(archive_node.is_expanded, "archive must be expanded during search");

    // Clear search
    state.clear_search();

    // Verify docs is still expanded, and archive is collapsed back
    let docs_node = state.workspace_roots[0].tree.iter().find(|n| n.name == "docs").unwrap();
    let archive_node_final = state.workspace_roots[0].tree.iter().find(|n| n.name == "archive").unwrap();
    assert!(docs_node.is_expanded, "docs was originally expanded and must remain expanded");
    assert!(!archive_node_final.is_expanded, "archive was originally collapsed and must be restored to collapsed");
}

#[test]
fn test_search_truncated_flag() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

    state.workspace_roots.push(WorkspaceRoot {
        path: PathBuf::from("/mock/root"),
        tree: vec![FileTreeNode {
            path: PathBuf::from("/mock/root/file.md"),
            name: "file.md".to_string(),
            is_dir: false,
            is_expanded: false,
            children: Vec::new(),
            mtime: None,
        }],
        is_expanded: true,
        is_missing: false,
        truncated: true, // Marked truncated!
        entries_used: 100,
        files_used: 50,
        deferred: false,
    });

    state.set_search_query("file");
    assert!(state.search_truncated, "search_truncated must be true when root is truncated");

    state.clear_search();
    assert!(!state.search_truncated, "search_truncated must reset to false on clear");
}

#[test]
fn test_drawer_render_contract() {
    let src = include_str!("../src/views/drawer.rs");

    // 1. Search container and clear button
    assert!(
        src.contains("drawer_search_container"),
        "drawer.rs must define drawer_search_container"
    );
    assert!(
        src.contains("btn_clear_search"),
        "drawer.rs must define btn_clear_search"
    );
    assert!(
        src.contains("drawer_search_query_display"),
        "drawer.rs must define drawer_search_query_display"
    );

    // 2. Status indicators
    assert!(
        src.contains("drawer_search_no_results"),
        "drawer.rs must define drawer_search_no_results"
    );
    assert!(
        src.contains("drawer_search_scanning"),
        "drawer.rs must define drawer_search_scanning"
    );
    assert!(
        src.contains("drawer_search_incomplete"),
        "drawer.rs must define drawer_search_incomplete"
    );

    // 3. String tokens
    assert!(
        src.contains("strings.search_placeholder"),
        "drawer.rs must use strings.search_placeholder"
    );
    assert!(
        src.contains("strings.search_no_results"),
        "drawer.rs must use strings.search_no_results"
    );
    assert!(
        src.contains("strings.search_scanning"),
        "drawer.rs must use strings.search_scanning"
    );
    assert!(
        src.contains("strings.search_incomplete"),
        "drawer.rs must use strings.search_incomplete"
    );

    // 4. Filtering logic
    assert!(
        src.contains("dir_matches_search"),
        "drawer.rs must filter directory nodes using dir_matches_search"
    );
    assert!(
        src.contains("show_draft"),
        "drawer.rs must filter draft items using show_draft"
    );
    assert!(
        src.contains("render_drawer_with_search"),
        "drawer.rs must export render_drawer_with_search"
    );
}

#[test]
fn test_drawer_search_params_from_state() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);
    state.set_search_query("test");

    let params = DrawerSearchParams::from_state(&state, None);
    assert_eq!(params.search_query, "test");
    assert!(!params.has_search_matches);
    assert!(!params.search_in_progress);
    assert!(!params.search_truncated);

    // When query is empty, has_search_matches is true
    state.clear_search();
    let params_empty = DrawerSearchParams::from_state(&state, None);
    assert_eq!(params_empty.search_query, "");
    assert!(params_empty.has_search_matches);
}
