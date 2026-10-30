use std::fs;
use tempfile::tempdir;

use glidemark_app::state::{
    normalize_loaded_roots, root_label, scan_root_layer, AppState, DocKey, RootAddError,
    ScanBudget, WorkspaceRoot,
};
use glidemark_core::config::Preferences;
use glidemark_core::document::{Document, DocumentStatus};

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
        entries_used: 0,
        files_used: 0,
        deferred: false,
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
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
        WorkspaceRoot {
            path: docs_b.clone(),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
        WorkspaceRoot {
            path: unique.clone(),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
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
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
        WorkspaceRoot {
            path: docs_b.clone(),
            tree: Vec::new(),
            is_expanded: false,
            is_missing: true,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
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
        entries_used: 0,
        files_used: 0,
        deferred: false,
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
        entries_used: 0,
        files_used: 0,
        deferred: false,
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
        entries_used: 0,
        files_used: 0,
        deferred: false,
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
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
        WorkspaceRoot {
            path: std::path::PathBuf::from("/b/shared/docs"),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
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

    // Global budget of 35 files total across 2 roots.
    // Under dynamic floor scaling, floor_files = min(30, 35 / 2) = 17 files.
    // Root B reserves 17 files, so Root A is allowed at most 35 - 17 = 18 files.
    // Root B then receives remaining 17 files, successfully scanning all its 5 files without starvation.
    state.refresh_all_roots_with_budget(ScanBudget::new(500, 35));

    // Root A truncated at 18 files
    assert_eq!(state.workspace_roots[0].tree.len(), 18);
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

#[test]
fn test_global_budget_enforced_across_multiple_adds() {
    let tmp = tempdir().unwrap();
    let root_1 = tmp.path().join("bulk_1");
    let root_2 = tmp.path().join("bulk_2");
    let root_3 = tmp.path().join("bulk_3");
    fs::create_dir_all(&root_1).unwrap();
    fs::create_dir_all(&root_2).unwrap();
    fs::create_dir_all(&root_3).unwrap();

    // 200 files in each root (total 600 files > global budget 400)
    for i in 0..200 {
        fs::write(root_1.join(format!("doc_{:03}.md", i)), "content").unwrap();
        fs::write(root_2.join(format!("doc_{:03}.md", i)), "content").unwrap();
        fs::write(root_3.join(format!("doc_{:03}.md", i)), "content").unwrap();
    }

    let mut state = clean_test_state(&tmp);
    let initial_files = state.scan_budget.files_left;
    let initial_entries = state.scan_budget.entries_left;
    assert_eq!(initial_files, 400);
    assert_eq!(initial_entries, 2000);

    let idx1 = state.add_workspace_root(root_1).expect("add root 1");
    assert_eq!(idx1, 0);

    let idx2 = state.add_workspace_root(root_2).expect("add root 2");
    assert_eq!(idx2, 1);

    let idx3 = state.add_workspace_root(root_3).expect("add root 3");
    assert_eq!(idx3, 2);

    let total_files_used: usize = state.workspace_roots.iter().map(|r| r.files_used).sum();
    let total_entries_used: usize = state.workspace_roots.iter().map(|r| r.entries_used).sum();

    // Total files used across all roots must strictly not exceed global budget 400
    assert!(
        total_files_used <= 400,
        "Total files used ({}) exceeds global cap 400",
        total_files_used
    );
    assert!(
        total_entries_used <= 2000,
        "Total entries used ({}) exceeds global cap 2000",
        total_entries_used
    );

    // Global budget left must match remaining
    assert_eq!(state.scan_budget.files_left + total_files_used, initial_files);
    assert_eq!(state.scan_budget.entries_left + total_entries_used, initial_entries);

    // Total files used is 400, files_left reaches 0
    assert_eq!(total_files_used, 400);
    assert_eq!(state.scan_budget.files_left, 0);

    // Later roots are truncated
    assert!(state.workspace_roots[0].truncated);
    assert!(state.workspace_roots[1].truncated);
    assert!(state.workspace_roots[2].truncated);

    // Adding another root when budget is exhausted results in deferred
    let root_4 = tmp.path().join("bulk_4");
    fs::create_dir_all(&root_4).unwrap();
    fs::write(root_4.join("doc_000.md"), "content").unwrap();
    let idx4 = state.add_workspace_root(root_4).expect("add root 4");
    assert_eq!(idx4, 3);
    assert!(state.workspace_roots[3].deferred);
    assert!(state.workspace_roots[3].tree.is_empty());
}

#[test]
fn test_lazy_expand_deducts_from_global_budget() {
    let tmp = tempdir().unwrap();
    let root_dir = tmp.path().join("lazy_budget_project");
    let docs_dir = root_dir.join("docs");
    let extra_dir = root_dir.join("extra");
    fs::create_dir_all(&docs_dir).unwrap();
    fs::create_dir_all(&extra_dir).unwrap();

    for i in 0..5 {
        fs::write(docs_dir.join(format!("doc_{}.md", i)), "doc").unwrap();
    }
    fs::write(extra_dir.join("e1.md"), "extra").unwrap();
    fs::write(extra_dir.join("e2.md"), "extra").unwrap();

    let mut state = clean_test_state(&tmp);
    let idx = state.add_workspace_root(root_dir.clone()).unwrap();

    let budget_after_add = state.scan_budget.clone();
    let root_files_after_add = state.workspace_roots[idx].files_used;

    let canonical_docs = fs::canonicalize(&docs_dir).unwrap();
    let toggled = state.toggle_root_folder(idx, &canonical_docs);
    assert!(toggled);

    // files_left decreased by 5, files_used increased by 5
    assert_eq!(
        state.scan_budget.files_left,
        budget_after_add.files_left - 5
    );
    assert_eq!(
        state.workspace_roots[idx].files_used,
        root_files_after_add + 5
    );

    // Artificially deplete scan budget
    state.scan_budget = ScanBudget::new(0, 0);

    let canonical_extra = fs::canonicalize(&extra_dir).unwrap();
    let toggled_extra = state.toggle_root_folder(idx, &canonical_extra);
    assert!(toggled_extra);

    // Root marked truncated due to 0 budget remaining
    assert!(state.workspace_roots[idx].truncated);
}

#[test]
fn test_dynamic_fair_floor_scaling_with_many_roots() {
    let tmp = tempdir().unwrap();
    let mut root_paths = Vec::new();

    for i in 0..25 {
        let root_dir = tmp.path().join(format!("root_{:02}", i));
        fs::create_dir_all(&root_dir).unwrap();
        fs::write(root_dir.join("a.md"), "a").unwrap();
        fs::write(root_dir.join("b.md"), "b").unwrap();
        root_paths.push(root_dir);
    }

    let mut state = clean_test_state(&tmp);
    for path in root_paths {
        state.add_workspace_root(path).expect("add root");
    }

    // Refresh all roots with global budget (which uses dynamic fair floor scaling)
    state.refresh_all_roots();

    // Verify root 0 was not starved to empty (would have been 0 under static 30-file floor reservation!)
    assert_eq!(state.workspace_roots[0].tree.len(), 2);
    assert!(!state.workspace_roots[0].deferred);

    // Verify all 25 roots received floor quota and scanned their 2 files
    assert_eq!(state.workspace_roots.len(), 25);
    for (i, root) in state.workspace_roots.iter().enumerate() {
        assert_eq!(
            root.tree.len(),
            2,
            "Root {} should have scanned both files, got {}",
            i,
            root.tree.len()
        );
        assert!(!root.deferred, "Root {} should not be deferred", i);
    }
}

#[test]
fn test_truncated_vs_deferred_distinction() {
    let tmp = tempdir().unwrap();
    let root_a = tmp.path().join("dist_a");
    let root_b = tmp.path().join("dist_b");
    let root_c = tmp.path().join("dist_c");
    fs::create_dir_all(&root_a).unwrap();
    fs::create_dir_all(&root_b).unwrap();
    fs::create_dir_all(&root_c).unwrap();

    for i in 0..5 {
        fs::write(root_a.join(format!("a_{}.md", i)), "a").unwrap();
    }
    for i in 0..3 {
        fs::write(root_b.join(format!("b_{}.md", i)), "b").unwrap();
    }
    for i in 0..10 {
        fs::write(root_c.join(format!("c_{}.md", i)), "c").unwrap();
    }

    let mut state = clean_test_state(&tmp);

    // 1. Root A added with ample budget
    let idx_a = state.add_workspace_root(root_a).unwrap();
    assert_eq!(idx_a, 0);
    assert_eq!(state.workspace_roots[0].tree.len(), 5);
    assert!(!state.workspace_roots[0].truncated);
    assert!(!state.workspace_roots[0].deferred);
    assert!(state.scan_budget.files_left > 0);

    // 2. Set scan_budget to zero: Root B must be deferred, not truncated
    state.scan_budget = ScanBudget::new(0, 0);
    let idx_b = state.add_workspace_root(root_b).unwrap();
    assert_eq!(idx_b, 1);
    assert!(
        state.workspace_roots[1].deferred,
        "Zero-budget root must be marked deferred"
    );
    assert!(
        state.workspace_roots[1].tree.is_empty(),
        "Deferred root tree must be empty"
    );
    assert!(
        !state.workspace_roots[1].truncated,
        "Deferred root was not scanned, so truncated must be false"
    );

    // 3. Partial budget (quota of 2 files for 10 files): Root C must be truncated, not deferred
    state.scan_budget = ScanBudget::new(100, 2);
    let idx_c = state.add_workspace_root(root_c).unwrap();
    assert_eq!(idx_c, 2);
    assert!(
        state.workspace_roots[2].truncated,
        "Partially scanned root hitting limit must be marked truncated"
    );
    assert!(
        !state.workspace_roots[2].deferred,
        "Partially scanned root must NOT be deferred"
    );
    assert_eq!(state.workspace_roots[2].tree.len(), 2);
}

#[test]
fn test_refresh_root_refunds_quota_before_check_preventing_accidental_deferred() {
    let tmp = tempdir().unwrap();
    let root_dir = tmp.path().join("refresh_refund_test");
    fs::create_dir_all(&root_dir).unwrap();

    for i in 0..5 {
        fs::write(root_dir.join(format!("doc_{}.md", i)), "content").unwrap();
    }

    let mut state = clean_test_state(&tmp);
    let idx = state.add_workspace_root(root_dir).expect("add root");
    assert_eq!(idx, 0);
    assert_eq!(state.workspace_roots[0].tree.len(), 5);
    assert_eq!(state.workspace_roots[0].files_used, 5);
    assert!(!state.workspace_roots[0].deferred);

    // 人为将全局预算耗尽
    state.scan_budget = ScanBudget::new(0, 0);

    // 此时对该根调用 state.refresh_root(0)
    state.refresh_root(0);

    // 1. state.workspace_roots[0].deferred == false (不会因全局预算耗尽被误标记为 deferred)
    assert!(
        !state.workspace_roots[0].deferred,
        "Root should not be deferred when its refunded quota is sufficient"
    );

    // 2. state.workspace_roots[0].tree.len() == 5 (文件树没有被意外清空)
    assert_eq!(
        state.workspace_roots[0].tree.len(),
        5,
        "File tree should not be cleared on refresh"
    );

    // 3. state.workspace_roots[0].files_used == 5
    assert_eq!(
        state.workspace_roots[0].files_used,
        5,
        "files_used should be 5 after re-scanning with refunded quota"
    );

    // 4. 验证退还与再分配逻辑正常工作
    assert_eq!(state.workspace_roots[0].entries_used, 0);
    assert!(!state.workspace_roots[0].truncated);
    assert_eq!(state.scan_budget.files_left, 0);
    assert_eq!(state.scan_budget.entries_left, 0);
}

#[test]
fn test_remove_workspace_root_preserves_open_docs_in_loose_docs() {
    let tmp = tempdir().unwrap();
    let root_a = tmp.path().join("root_a");
    fs::create_dir_all(&root_a).unwrap();
    let file_a = root_a.join("doc_a.md");
    fs::write(&file_a, "# Doc A initial").unwrap();

    let mut state = clean_test_state(&tmp);

    // 1. Add workspace root root_a
    let idx = state.add_workspace_root(root_a.clone()).expect("add root_a");
    assert_eq!(idx, 0);

    // 2. Open doc_a inside root_a
    state.open_file(file_a.clone()).unwrap();
    let doc_a_key = DocKey::Path(file_a.clone());
    assert_eq!(state.active, Some(doc_a_key.clone()));

    // Make doc_a dirty
    state.editor.insert_text("\nDirty edit in doc_a");
    state.sync_from_editor();
    assert!(state.is_dirty());

    // 3. Open an unnamed draft (scratch) and make it dirty
    state.focus_or_create_draft();
    let scratch_key = state.active.clone().expect("Draft slot active");
    assert!(scratch_key.is_scratch());
    state.editor.insert_text("Dirty draft content");
    state.sync_from_editor();
    assert!(state.is_dirty());

    assert_eq!(state.open_docs.len(), 2);
    let loose_before = state.loose_docs();
    assert!(loose_before.contains(&scratch_key));
    assert!(!loose_before.contains(&doc_a_key));

    // 4. Call remove_workspace_root(0)
    let removed = state.remove_workspace_root(0).unwrap();
    assert!(removed.is_some());
    assert!(state.workspace_roots.is_empty());

    // 5. Verify: open_docs still contains doc_a and scratch; dirty status intact
    assert_eq!(state.open_docs.len(), 2);
    let open_doc_a = state
        .open_docs
        .iter()
        .find(|d| d.key == doc_a_key)
        .expect("doc_a preserved");
    assert!(open_doc_a.doc.buffer.contains("Dirty edit in doc_a"));
    assert_eq!(open_doc_a.doc.status, DocumentStatus::Modified);

    let open_scratch = state
        .open_docs
        .iter()
        .find(|d| d.key == scratch_key)
        .expect("scratch preserved");
    assert!(open_scratch.doc.buffer.contains("Dirty draft content"));
    assert_eq!(open_scratch.doc.status, DocumentStatus::Modified);

    // 6. Verify: state.loose_docs() smoothly includes both doc_a and scratch
    let loose_after = state.loose_docs();
    assert_eq!(loose_after.len(), 2);
    assert!(loose_after.contains(&doc_a_key));
    assert!(loose_after.contains(&scratch_key));
}

#[test]
fn test_open_external_file_does_not_pollute_roots_and_enters_loose_docs() {
    let tmp = tempdir().unwrap();
    let root_a = tmp.path().join("root_a");
    fs::create_dir_all(&root_a).unwrap();
    let file_inside = root_a.join("inside.md");
    fs::write(&file_inside, "# Inside root").unwrap();

    let external_dir = tmp.path().join("external_folder");
    fs::create_dir_all(&external_dir).unwrap();
    let external_file = external_dir.join("external.md");
    fs::write(&external_file, "# External Document").unwrap();

    let mut state = clean_test_state(&tmp);

    // 1. Add root_a
    let idx = state.add_workspace_root(root_a.clone()).expect("add root_a");
    assert_eq!(idx, 0);
    assert_eq!(state.workspace_roots.len(), 1);

    let tree_len_before = state.workspace_roots[0].tree.len();
    let files_used_before = state.workspace_roots[0].files_used;

    // 2. Open external file
    state.open_file(external_file.clone()).expect("open external file");

    // 3. Verify: state.workspace_roots.len() remains 1, no pollution
    assert_eq!(
        state.workspace_roots.len(),
        1,
        "Workspace roots must not be polluted by opening external file"
    );
    let canonical_root_a = fs::canonicalize(&root_a).unwrap();
    assert_eq!(state.workspace_roots[0].path, canonical_root_a);

    // 4. Verify: state.active switched to external.md
    let external_key = DocKey::Path(external_file.clone());
    assert_eq!(state.active, Some(external_key.clone()));

    // 5. Verify: state.loose_docs() contains external.md
    let loose = state.loose_docs();
    assert!(
        loose.contains(&external_key),
        "External document must enter loose_docs"
    );

    // 6. Verify: root_a tree and quota usage are untouched
    assert_eq!(
        state.workspace_roots[0].tree.len(),
        tree_len_before,
        "Root tree must remain unchanged"
    );
    assert_eq!(
        state.workspace_roots[0].files_used,
        files_used_before,
        "Files used quota must remain unchanged"
    );
    assert!(
        !state.workspace_roots[0]
            .tree
            .iter()
            .any(|n| n.name == "external.md"),
        "External file must not appear in root_a tree"
    );
}

#[test]
fn test_root_label_disambiguation_hierarchical() {
    // 1. Single root: displays short folder name
    let single_root = vec![WorkspaceRoot {
        path: std::path::PathBuf::from("/path1/shared/notes"),
        tree: Vec::new(),
        is_expanded: true,
        is_missing: false,
        truncated: false,
        entries_used: 0,
        files_used: 0,
        deferred: false,
    }];
    assert_eq!(root_label(&single_root, 0), "notes");

    // 2. Colliding folder names with distinct parent directories: disambiguates via parent directory
    let colliding_different_parents = vec![
        WorkspaceRoot {
            path: std::path::PathBuf::from("/work/shared/notes"),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
        WorkspaceRoot {
            path: std::path::PathBuf::from("/home/personal/notes"),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
    ];
    assert_eq!(
        root_label(&colliding_different_parents, 0),
        "notes \u{2014} shared"
    );
    assert_eq!(
        root_label(&colliding_different_parents, 1),
        "notes \u{2014} personal"
    );

    // 3. Colliding folder names AND identical parent directories: fallback to full path
    let colliding_same_parents = vec![
        WorkspaceRoot {
            path: std::path::PathBuf::from("/path1/shared/notes"),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
        WorkspaceRoot {
            path: std::path::PathBuf::from("/path2/shared/notes"),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
    ];
    assert_eq!(
        root_label(&colliding_same_parents, 0),
        "/path1/shared/notes"
    );
    assert_eq!(
        root_label(&colliding_same_parents, 1),
        "/path2/shared/notes"
    );

    // 4. Missing root vs normal root disambiguation:
    // A normal root does not see a missing root as a collision (keeps short name),
    // but the missing root disambiguates against the normal root.
    let normal_and_missing = vec![
        WorkspaceRoot {
            path: std::path::PathBuf::from("/path1/shared/notes"),
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
        WorkspaceRoot {
            path: std::path::PathBuf::from("/path2/backup/notes"),
            tree: Vec::new(),
            is_expanded: false,
            is_missing: true,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
    ];
    // Normal root 0 does not conflict with missing root 1 -> short name
    assert_eq!(root_label(&normal_and_missing, 0), "notes");
    // Missing root 1 conflicts with normal root 0 -> parent disambiguated
    assert_eq!(root_label(&normal_and_missing, 1), "notes \u{2014} backup");

    // Missing roots colliding with each other both disambiguate
    let two_missing = vec![
        WorkspaceRoot {
            path: std::path::PathBuf::from("/path1/shared/notes"),
            tree: Vec::new(),
            is_expanded: false,
            is_missing: true,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
        WorkspaceRoot {
            path: std::path::PathBuf::from("/path2/backup/notes"),
            tree: Vec::new(),
            is_expanded: false,
            is_missing: true,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        },
    ];
    assert_eq!(root_label(&two_missing, 0), "notes \u{2014} shared");
    assert_eq!(root_label(&two_missing, 1), "notes \u{2014} backup");
}

