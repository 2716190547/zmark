use std::fs;
use tempfile::tempdir;

use glidemark_app::state::{root_label, AppState, RootAddError, WorkspaceRoot};
use glidemark_core::document::Document;

#[test]
fn test_workspace_root_add_canonicalize_and_dedup() {
    let tmp = tempdir().unwrap();
    let root_a = tmp.path().join("project_a");
    fs::create_dir_all(&root_a).unwrap();

    let file_not_dir = tmp.path().join("some_file.txt");
    fs::write(&file_not_dir, "hello").unwrap();

    let nonexistent = tmp.path().join("does_not_exist");

    let doc = Document::new_scratch();
    let mut state = AppState::new(doc);

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
        let mut state = AppState::new(Document::new_scratch());
        state.add_workspace_root(parent.clone()).unwrap();
        let err = state.add_workspace_root(child.clone()).unwrap_err();
        assert_eq!(err, RootAddError::NestedInExisting(canonical_parent.clone()));
    }

    // Case 2: Existing child rejects parent (ContainsExisting)
    {
        let mut state = AppState::new(Document::new_scratch());
        state.add_workspace_root(child.clone()).unwrap();
        let err = state.add_workspace_root(parent.clone()).unwrap_err();
        assert_eq!(err, RootAddError::ContainsExisting(canonical_child.clone()));
    }
}

#[test]
fn test_workspace_root_missing_recovery() {
    let tmp = tempdir().unwrap();
    let missing_path = tmp.path().join("revived_project");

    let mut state = AppState::new(Document::new_scratch());
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

    let mut state = AppState::new(Document::new_scratch());
    let root_idx = state.add_workspace_root(root_path.clone()).unwrap();
    assert_eq!(root_idx, 0);

    // Open file inside the workspace root
    state.open_file(file_path.clone()).unwrap();
    assert_eq!(state.open_docs.len(), 1);
    let open_doc_key = state.active.clone().unwrap();

    // Remove the workspace root
    let removed = state.remove_workspace_root(0);
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
    assert!(state.remove_workspace_root(0).is_none());
}
