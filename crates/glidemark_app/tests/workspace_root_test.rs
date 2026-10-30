use std::fs;
use tempfile::tempdir;

use glidemark_app::state::{
    normalize_loaded_roots, root_label, AppState, RootAddError, WorkspaceRoot,
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
