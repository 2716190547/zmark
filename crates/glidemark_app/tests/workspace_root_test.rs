use std::fs;
use tempfile::tempdir;

use glidemark_app::state::{
    normalize_loaded_roots, root_label, scan_root_layer, AppState, RootAddError, ScanBudget,
    WorkspaceRoot,
};
use glidemark_core::config::Preferences;
use glidemark_core::document::Document;

fn clean_test_state(tmp: &tempfile::TempDir) -> AppState {
    let config_path = tmp.path().join("test_config.toml");
    AppState::new_with_preferences_at(
        Document::new_scratch(),
        Preferences::default(),
        config_path,
    )
}

#[test]
fn test_workspace_root_add_canonicalize_and_dedup() {
    let tmp = tempdir().unwrap();
    let root_a = tmp.path().join("project_a");
    fs::create_dir_all(&root_a).unwrap();

    let file_not_dir = tmp.path().join("some_file.txt");
    fs::write(&file_not_dir, "hello").unwrap();

    let nonexistent = tmp.path().join("does_not_exist");

    let mut state = clean_test_state(&tmp);

    // 1. Non-existent directory returns NotFound
    assert_eq!(
        state.add_workspace_root(nonexistent),
        Err(RootAddError::NotFound)
    );

    // 2. File path returns NotADirectory
    assert_eq!(
        state.add_workspace_root(file_not_dir),
        Err(RootAddError::NotADirectory)
    );

    // 3. Valid directory succeeds
    let idx = state.add_workspace_root(root_a.clone()).expect("Should add root");
    assert_eq!(idx, 0);
    let canonical_a = fs::canonicalize(&root_a).unwrap();
    assert_eq!(state.workspace_roots[0].path, canonical_a);
    assert!(!state.workspace_roots[0].is_missing);
    assert!(state.workspace_roots[0].is_expanded);

    // 4. Duplicate addition returns AlreadyExists
    assert_eq!(
        state.add_workspace_root(root_a.clone()),
        Err(RootAddError::AlreadyExists)
    );

    // 5. Canonicalized path or relative path with '.' resolves and also detects AlreadyExists
    let root_a_with_dot = root_a.join(".").join(".");
    assert_eq!(
        state.add_workspace_root(root_a_with_dot),
        Err(RootAddError::AlreadyExists)
    );
}

#[test]
fn test_workspace_root_nested_and_parent_rejection() {
    let tmp = tempdir().unwrap();
    let parent = tmp.path().join("parent_workspace");
    let child = parent.join("nested_folder");
    fs::create_dir_all(&child).unwrap();

    let canonical_parent = fs::canonicalize(&parent).unwrap();
    let canonical_child = fs::canonicalize(&child).unwrap();

    // Case 1: Existing parent rejects child (NestedInExisting)
    {
        let mut state = clean_test_state(&tmp);
        state.add_workspace_root(parent.clone()).unwrap();
        let err = state.add_workspace_root(child.clone()).unwrap_err();
        assert_eq!(err, RootAddError::NestedInExisting(canonical_parent.clone()));
    }

    // Case 2: Existing child rejects parent (ContainsExisting)
    {
        let mut state = clean_test_state(&tmp);
        state.add_workspace_root(child.clone()).unwrap();
        let err = state.add_workspace_root(parent.clone()).unwrap_err();
        assert_eq!(err, RootAddError::ContainsExisting(canonical_child.clone()));
    }
}

#[test]
fn test_workspace_root_missing_recovery() {
    let tmp = tempdir().unwrap();
    let missing_path = tmp.path().join("revived_project");

    let mut state = clean_test_state(&tmp);
    // Simulate a missing workspace root loaded from preferences
    state.workspace_roots.push(WorkspaceRoot {
        path: missing_path.clone(),
        tree: Vec::new(),
        is_expanded: false,
        is_missing: true,
        truncated: false,
    });
    assert_eq!(state.workspace_roots.len(), 1);
    assert!(state.workspace_roots[0].is_missing);
    assert!(!state.workspace_roots[0].is_expanded);

    // Before directory exists, add_workspace_root should return NotFound
    assert_eq!(
        state.add_workspace_root(missing_path.clone()),
        Err(RootAddError::NotFound)
    );

    // Create the directory on disk
    fs::create_dir_all(&missing_path).unwrap();
    let canonical = fs::canonicalize(&missing_path).unwrap();

    // Adding it now should revive the existing missing root instead of creating a new one
    let idx = state
        .add_workspace_root(missing_path.clone())
        .expect("Revival should succeed");
    assert_eq!(idx, 0);
    assert_eq!(state.workspace_roots.len(), 1);
    assert_eq!(state.workspace_roots[0].path, canonical);
    assert!(!state.workspace_roots[0].is_missing);
    assert!(state.workspace_roots[0].is_expanded);
    assert!(state.preferences.workspace_roots.contains(&canonical));
}

#[test]
fn test_workspace_root_disambiguation_labels() {
    let tmp = tempdir().unwrap();
    let parent_a = tmp.path().join("proj_a");
    let parent_b = tmp.path().join("proj_b");
    let docs_a = parent_a.join("docs");
    let docs_b = parent_b.join("docs");
    let unique = tmp.path().join("unique_repo");

    fs::create_dir_all(&docs_a).unwrap();
    fs::create_dir_all(&docs_b).unwrap();
    fs::create_dir_all(&unique).unwrap();

    let roots = vec![
        WorkspaceRoot {
            path: docs_a.clone(),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
        },
        WorkspaceRoot {
            path: docs_b.clone(),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
        },
        WorkspaceRoot {
            path: unique.clone(),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
        },
    ];

    // Unique folder name is unaffected
    assert_eq!(root_label(&roots, 2), "unique_repo");

    // Colliding folder names are disambiguated by parent directory name with em-dash
    assert_eq!(root_label(&roots, 0), "docs \u{2014} proj_a");
    assert_eq!(root_label(&roots, 1), "docs \u{2014} proj_b");

    // Missing roots do not force active unique roots to disambiguate
    let roots_with_missing = vec![
        WorkspaceRoot {
            path: docs_a.clone(),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
        },
        WorkspaceRoot {
            path: docs_b.clone(),
            tree: Vec::new(),
            is_expanded: false,
            is_missing: true,
            truncated: false,
        },
    ];
    assert_eq!(root_label(&roots_with_missing, 0), "docs");

    // Out-of-bounds index returns empty string
    assert_eq!(root_label(&roots, 99), "");
}

#[test]
fn test_workspace_root_remove_preserves_docs() {
    let tmp = tempdir().unwrap();
    let root_path = tmp.path().join("preserved_workspace");
    fs::create_dir_all(&root_path).unwrap();
    let file_path = root_path.join("readme.md");
    fs::write(&file_path, "# Preserved Document").unwrap();

    let mut state = clean_test_state(&tmp);
    let root_idx = state.add_workspace_root(root_path.clone()).unwrap();
    assert_eq!(root_idx, 0);

    // Open file inside the workspace root
    state.open_file(file_path.clone()).unwrap();
    assert_eq!(state.open_docs.len(), 1);
    let open_doc_key = state.active.clone().unwrap();

    // Remove the workspace root
    let removed = state.remove_workspace_root(0).unwrap();
    assert!(removed.is_some());
    assert!(state.workspace_roots.is_empty());

    // CRITICAL: open_docs MUST be preserved, active key unchanged
    assert_eq!(state.open_docs.len(), 1);
    assert_eq!(state.active, Some(open_doc_key));
    assert_eq!(
        state.open_docs[0].doc.path.as_ref(),
        Some(&file_path)
    );
    assert!(state.preferences.workspace_roots.is_empty());

    // Out of bounds remove returns None
    assert!(state.remove_workspace_root(0).unwrap().is_none());
}

#[test]
fn test_workspace_root_host_config_untouched() {
    let host_config_path = Preferences::default_config_path();
    let host_content_before = fs::read_to_string(&host_config_path).ok();

    let tmp = tempdir().unwrap();
    let project = tmp.path().join("isolated_project");
    fs::create_dir_all(&project).unwrap();

    let mut state = clean_test_state(&tmp);
    state.add_workspace_root(project.clone()).unwrap();
    state.remove_workspace_root(0).unwrap();

    let host_content_after = fs::read_to_string(&host_config_path).ok();
    assert_eq!(host_content_before, host_content_after);
}

#[test]
fn test_normalize_loaded_roots_dedup_and_nesting() {
    let tmp = tempdir().unwrap();
    let parent = tmp.path().join("norm_parent");
    let child = parent.join("norm_child");
    let sibling = tmp.path().join("norm_sibling");
    let missing_parent = tmp.path().join("missing_parent");
    let missing_child = missing_parent.join("missing_child");

    fs::create_dir_all(&child).unwrap();
    fs::create_dir_all(&sibling).unwrap();

    let canonical_parent = fs::canonicalize(&parent).unwrap();
    let canonical_sibling = fs::canonicalize(&sibling).unwrap();

    let configured_paths = vec![
        parent.clone(),
        child.clone(),                  // Should be dropped (nested inside parent)
        parent.clone(),                 // Should be dropped (duplicate)
        sibling.clone(),                // Valid sibling
        missing_parent.clone(),         // Missing parent
        missing_child.clone(),          // Should be dropped (nested inside missing parent)
        missing_parent.clone(),         // Should be dropped (duplicate missing)
    ];

    let normalized = normalize_loaded_roots(&configured_paths);
    assert_eq!(normalized.roots.len(), 3);
    assert_eq!(normalized.discarded.len(), 4);

    // 1. Parent is valid and canonicalized
    assert_eq!(normalized.roots[0].path, canonical_parent);
    assert!(!normalized.roots[0].is_missing);
    assert!(normalized.roots[0].is_expanded);

    // 2. Sibling is valid and canonicalized
    assert_eq!(normalized.roots[1].path, canonical_sibling);
    assert!(!normalized.roots[1].is_missing);
    assert!(normalized.roots[1].is_expanded);

    // 3. Missing parent is preserved and marked missing
    assert_eq!(normalized.roots[2].path, missing_parent);
    assert!(normalized.roots[2].is_missing);
    assert!(!normalized.roots[2].is_expanded);
}

#[test]
fn test_workspace_root_missing_root_blocks_nesting() {
    let tmp = tempdir().unwrap();
    let missing_parent = tmp.path().join("missing_parent");
    let real_child = missing_parent.join("real_child");
    fs::create_dir_all(&real_child).unwrap();

    let mut state = clean_test_state(&tmp);
    state.workspace_roots.push(WorkspaceRoot {
        path: missing_parent.clone(),
        tree: Vec::new(),
        is_expanded: false,
        is_missing: true,
        truncated: false,
    });

    // Attempting to add a real subdirectory under a missing root is rejected
    let err = state.add_workspace_root(real_child.clone()).unwrap_err();
    assert_eq!(err, RootAddError::NestedInExisting(missing_parent.clone()));

    // Attempting to add the parent of a missing child is also rejected
    let tmp_nested = tempdir().unwrap();
    let real_parent = tmp_nested.path().join("real_parent");
    let missing_child = real_parent.join("missing_child");
    fs::create_dir_all(&real_parent).unwrap();

    let mut state2 = clean_test_state(&tmp_nested);
    state2.workspace_roots.push(WorkspaceRoot {
        path: missing_child.clone(),
        tree: Vec::new(),
        is_expanded: false,
        is_missing: true,
        truncated: false,
    });

    let err2 = state2.add_workspace_root(real_parent.clone()).unwrap_err();
    assert_eq!(err2, RootAddError::ContainsExisting(missing_child.clone()));
}

#[test]
fn test_workspace_root_atomic_rollback_on_save_failure() {
    let tmp = tempdir().unwrap();
    let root_a = tmp.path().join("project_a");
    let root_b = tmp.path().join("project_b");
    fs::create_dir_all(&root_a).unwrap();
    fs::create_dir_all(&root_b).unwrap();

    let mut state = clean_test_state(&tmp);
    state.add_workspace_root(root_a.clone()).unwrap();
    assert_eq!(state.workspace_roots.len(), 1);

    // Point preferences_path to a forbidden read-only path to force save failure
    state.preferences_path = std::path::PathBuf::from("/dev/null/forbidden/config.toml");

    // 1. add_workspace_root failure must roll back newly added root
    let add_err = state.add_workspace_root(root_b.clone()).unwrap_err();
    assert!(matches!(add_err, RootAddError::SaveFailed(_)));
    assert_eq!(state.workspace_roots.len(), 1);
    assert_eq!(state.preferences.workspace_roots.len(), 1);
    assert_eq!(
        state.workspace_roots[0].path,
        fs::canonicalize(&root_a).unwrap()
    );

    // 2. remove_workspace_root failure must roll back removed root
    let remove_err = state.remove_workspace_root(0).unwrap_err();
    assert!(remove_err.contains("Failed to save preferences"));
    assert_eq!(state.workspace_roots.len(), 1);
    assert_eq!(state.preferences.workspace_roots.len(), 1);
    assert_eq!(
        state.workspace_roots[0].path,
        fs::canonicalize(&root_a).unwrap()
    );
}

#[test]
fn test_root_label_root_path_and_full_path_fallback() {
    // 1. Root path "/" returns "/"
    let root_slash = vec![WorkspaceRoot {
        path: std::path::PathBuf::from("/"),
        tree: Vec::new(),
        is_expanded: true,
        is_missing: false,
        truncated: false,
    }];
    assert_eq!(root_label(&root_slash, 0), "/");

    // 2. Roots with identical folder name AND identical parent folder name fall back to full path
    let roots_same_parent = vec![
        WorkspaceRoot {
            path: std::path::PathBuf::from("/a/shared/docs"),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
        },
        WorkspaceRoot {
            path: std::path::PathBuf::from("/b/shared/docs"),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
        },
    ];

    assert_eq!(root_label(&roots_same_parent, 0), "/a/shared/docs");
    assert_eq!(root_label(&roots_same_parent, 1), "/b/shared/docs");
}

#[test]
fn test_scan_budget_allocation_and_depletion() {
    let tmp = tempdir().unwrap();
    let root_dir = tmp.path().join("depletion_test");
    fs::create_dir_all(&root_dir).unwrap();

    // Verify budget constants and default values
    let default_budget = ScanBudget::default();
    assert_eq!(default_budget.entries_left, 2000);
    assert_eq!(default_budget.files_left, 400);
    assert_eq!(ScanBudget::PER_ROOT_ENTRY_CAP, 800);
    assert_eq!(ScanBudget::PER_ROOT_FILE_CAP, 150);
    assert_eq!(ScanBudget::FIRST_LAYER_FLOOR_ENTRIES, 100);
    assert_eq!(ScanBudget::FIRST_LAYER_FLOOR_FILES, 30);

    // Verify unit consumption and can_scan_entry
    let mut b = ScanBudget::new(2, 1);
    assert!(b.can_scan_entry());
    assert!(b.consume_entry());
    assert_eq!(b.entries_left, 1);
    assert!(b.can_scan_entry());
    assert!(b.consume_entry());
    assert_eq!(b.entries_left, 0);
    assert!(!b.consume_entry());
    assert!(!b.can_scan_entry());

    assert!(b.consume_file());
    assert_eq!(b.files_left, 0);
    assert!(!b.consume_file());

    // Create 5 markdown files on disk
    for i in 0..5 {
        fs::write(root_dir.join(format!("doc_{}.md", i)), "# test").unwrap();
    }

    // Scan with budget restricted to 2 files
    let mut scan_b = ScanBudget::new(100, 2);
    let empty_set = std::collections::HashSet::new();
    let (nodes, truncated) = scan_root_layer(&root_dir, 0, &mut scan_b, &empty_set, 4);

    assert_eq!(nodes.len(), 2);
    assert!(truncated);
    assert_eq!(scan_b.files_left, 0);

    // Scan with sufficient budget
    let mut full_b = ScanBudget::new(100, 10);
    let (full_nodes, full_trunc) = scan_root_layer(&root_dir, 0, &mut full_b, &empty_set, 4);
    assert_eq!(full_nodes.len(), 5);
    assert!(!full_trunc);
    assert_eq!(full_b.files_left, 5);
}

#[test]
fn test_scan_budget_fairness_guarantee() {
    let tmp = tempdir().unwrap();
    let root_a = tmp.path().join("root_a");
    let root_b = tmp.path().join("root_b");
    fs::create_dir_all(&root_a).unwrap();
    fs::create_dir_all(&root_b).unwrap();

    // Root A has 50 files
    for i in 0..50 {
        fs::write(root_a.join(format!("a_{:02}.md", i)), "content").unwrap();
    }
    // Root B has 5 files
    for i in 0..5 {
        fs::write(root_b.join(format!("b_{:02}.md", i)), "content").unwrap();
    }

    let mut state = clean_test_state(&tmp);
    state.add_workspace_root(root_a).unwrap();
    state.add_workspace_root(root_b).unwrap();

    // Global budget of 35 files total.
    // Because Root B is remaining after Root A, Root B reserves FIRST_LAYER_FLOOR_FILES (30).
    // Therefore, Root A is allowed at most 35 - 30 = 5 files.
    // Root B then receives up to 30 files, successfully scanning all its 5 files without starvation.
    state.refresh_all_roots_with_budget(ScanBudget::new(500, 35));

    // Root A truncated at 5 files
    assert_eq!(state.workspace_roots[0].tree.len(), 5);
    assert!(state.workspace_roots[0].truncated);

    // Root B scanned all 5 files thanks to floor reservation
    assert_eq!(state.workspace_roots[1].tree.len(), 5);
    assert!(!state.workspace_roots[1].truncated);
}

#[test]
fn test_lazy_tree_expand_on_demand() {
    let tmp = tempdir().unwrap();
    let root_dir = tmp.path().join("lazy_project");
    let docs_dir = root_dir.join("docs");
    let other_dir = root_dir.join("other");
    fs::create_dir_all(&docs_dir).unwrap();
    fs::create_dir_all(&other_dir).unwrap();

    fs::write(root_dir.join("readme.md"), "# Readme").unwrap();
    fs::write(docs_dir.join("api.md"), "# API").unwrap();
    fs::write(docs_dir.join("guide.md"), "# Guide").unwrap();
    fs::write(other_dir.join("note.md"), "# Note").unwrap();

    let mut state = clean_test_state(&tmp);
    let idx = state.add_workspace_root(root_dir.clone()).unwrap();

    // Initially, subfolders in root tree must have children.is_empty() and is_expanded == false
    let tree = &state.workspace_roots[idx].tree;
    let docs_node = tree.iter().find(|n| n.name == "docs").expect("docs node");
    assert!(docs_node.is_dir);
    assert!(!docs_node.is_expanded);
    assert!(docs_node.children.is_empty());

    let other_node = tree.iter().find(|n| n.name == "other").expect("other node");
    assert!(other_node.is_dir);
    assert!(!other_node.is_expanded);
    assert!(other_node.children.is_empty());

    let canonical_docs = fs::canonicalize(&docs_dir).unwrap();
    // Expand docs subfolder on demand
    let toggled = state.toggle_root_folder(idx, &canonical_docs);
    assert!(toggled);

    // After toggle, docs is expanded and its children populated
    let tree_after = &state.workspace_roots[idx].tree;
    let docs_after = tree_after.iter().find(|n| n.name == "docs").unwrap();
    assert!(docs_after.is_expanded);
    assert_eq!(docs_after.children.len(), 2);
    assert_eq!(docs_after.children[0].name, "api.md");
    assert_eq!(docs_after.children[1].name, "guide.md");

    // Other folder remains unexpanded with empty children
    let other_after = tree_after.iter().find(|n| n.name == "other").unwrap();
    assert!(!other_after.is_expanded);
    assert!(other_after.children.is_empty());

    // Toggle again to collapse (children preserved, not re-scanned)
    assert!(state.toggle_root_folder(idx, &canonical_docs));
    let docs_collapsed = state.workspace_roots[idx].tree.iter().find(|n| n.name == "docs").unwrap();
    assert!(!docs_collapsed.is_expanded);
    assert_eq!(docs_collapsed.children.len(), 2);
}

#[test]
fn test_refresh_root_preserves_expanded_state() {
    let tmp = tempdir().unwrap();
    let root_0 = tmp.path().join("root_0");
    let root_1 = tmp.path().join("root_1");
    let sub_0 = root_0.join("sub0");
    let sub_1 = root_1.join("sub1");
    fs::create_dir_all(&sub_0).unwrap();
    fs::create_dir_all(&sub_1).unwrap();

    fs::write(sub_0.join("f0.md"), "f0").unwrap();
    fs::write(sub_1.join("f1.md"), "f1").unwrap();

    let mut state = clean_test_state(&tmp);
    let idx0 = state.add_workspace_root(root_0.clone()).unwrap();
    let idx1 = state.add_workspace_root(root_1.clone()).unwrap();

    let canonical_sub0 = fs::canonicalize(&sub_0).unwrap();
    // Expand sub0 in Root 0
    state.toggle_root_folder(idx0, &canonical_sub0);
    assert!(state.workspace_roots[idx0].tree[0].is_expanded);
    assert_eq!(state.workspace_roots[idx0].tree[0].children.len(), 1);

    // Root 1 subfolder is unexpanded
    assert!(!state.workspace_roots[idx1].tree[0].is_expanded);
    assert!(state.workspace_roots[idx1].tree[0].children.is_empty());

    // Add extra files to both roots on disk
    fs::write(sub_0.join("f0_new.md"), "f0 new").unwrap();
    fs::write(sub_1.join("f1_new.md"), "f1 new").unwrap();
    fs::write(root_1.join("root1_new.md"), "root1 new").unwrap();

    // Call refresh_root only for Root 0
    state.refresh_root(idx0);

    // Root 0 preserved its expanded state and updated its children
    let sub0_refreshed = &state.workspace_roots[idx0].tree[0];
    assert!(sub0_refreshed.is_expanded);
    assert_eq!(sub0_refreshed.children.len(), 2);
    assert!(sub0_refreshed.children.iter().any(|c| c.name == "f0_new.md"));

    // Root 1 was NOT refreshed (does not see root1_new.md or f1_new.md)
    let root1_tree = &state.workspace_roots[idx1].tree;
    assert_eq!(root1_tree.len(), 1); // Only sub1, not root1_new.md
    assert!(!root1_tree[0].is_expanded);
    assert!(root1_tree[0].children.is_empty());

    // Now explicitly refresh Root 1
    state.refresh_root(idx1);
    assert_eq!(state.workspace_roots[idx1].tree.len(), 2); // sub1 and root1_new.md
}

#[test]
fn test_root_truncated_flag_set_on_limit() {
    let tmp = tempdir().unwrap();
    let root_dir = tmp.path().join("truncated_test");
    fs::create_dir_all(&root_dir).unwrap();

    // Create 6 markdown files
    for i in 0..6 {
        fs::write(root_dir.join(format!("item_{}.md", i)), "content").unwrap();
    }

    let mut state = clean_test_state(&tmp);
    let idx = state.add_workspace_root(root_dir.clone()).unwrap();

    // Initially with default caps (150 files), 6 files fit without truncation
    assert_eq!(state.workspace_roots[idx].tree.len(), 6);
    assert!(!state.workspace_roots[idx].truncated);

    // Refresh with budget cap of 3 files
    state.refresh_all_roots_with_budget(ScanBudget::new(100, 3));
    assert_eq!(state.workspace_roots[idx].tree.len(), 3);
    assert!(state.workspace_roots[idx].truncated);

    // Test entry truncation with directories
    let dir_test = tmp.path().join("dir_trunc_test");
    fs::create_dir_all(&dir_test).unwrap();
    for i in 0..5 {
        fs::create_dir_all(dir_test.join(format!("dir_{}", i))).unwrap();
    }
    let mut state2 = clean_test_state(&tmp);
    let idx2 = state2.add_workspace_root(dir_test.clone()).unwrap();
    // Default: 5 dirs fit
    assert_eq!(state2.workspace_roots[idx2].tree.len(), 5);
    assert!(!state2.workspace_roots[idx2].truncated);

    // Refresh with entry budget of 2
    state2.refresh_all_roots_with_budget(ScanBudget::new(2, 50));
    assert_eq!(state2.workspace_roots[idx2].tree.len(), 2);
    assert!(state2.workspace_roots[idx2].truncated);
}
