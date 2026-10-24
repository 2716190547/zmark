use glidemark_app::state::{AppState, ViewMode};
use glidemark_core::document::Document;
use tempfile::tempdir;
use std::fs;

#[test]
fn test_app_state_initialization_and_mode_switching() {
    let doc = Document::new_scratch();
    let mut state = AppState::new(doc);

    assert_eq!(state.view_mode, ViewMode::Split);
    assert!(!state.drawer_open);
    assert!(!state.is_dirty());
    assert!(!state.has_conflict());

    // Switch to Source
    state.set_view_mode(ViewMode::Source);
    assert_eq!(state.view_mode, ViewMode::Source);

    // Switch to Preview
    state.set_view_mode(ViewMode::Preview);
    assert_eq!(state.view_mode, ViewMode::Preview);

    // Switch to Split
    state.set_view_mode(ViewMode::Split);
    assert_eq!(state.view_mode, ViewMode::Split);
}

#[test]
fn test_drawer_and_sibling_files_detection() {
    let dir = tempdir().unwrap();
    let file1 = dir.path().join("doc1.md");
    let file2 = dir.path().join("doc2.md");
    let other = dir.path().join("ignored.txt");

    fs::write(&file1, "# Doc 1").unwrap();
    fs::write(&file2, "# Doc 2").unwrap();
    fs::write(&other, "Ignore me").unwrap();

    let doc = Document::from_file(file1.clone()).unwrap();
    let mut state = AppState::new(doc);

    // Initially drawer closed
    assert!(!state.drawer_open);
    // Sibling files detected
    assert_eq!(state.sibling_files.len(), 2);
    assert!(state.sibling_files.contains(&file1));
    assert!(state.sibling_files.contains(&file2));
    assert!(!state.sibling_files.contains(&other));

    // Toggle drawer
    state.toggle_drawer();
    assert!(state.drawer_open);

    state.toggle_drawer();
    assert!(!state.drawer_open);
}

#[test]
fn test_export_standalone_html_from_app_state() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("spec.md");
    fs::write(&file, "# Test Heading\n\n- [x] Done item").unwrap();

    let doc = Document::from_file(file).unwrap();
    let state = AppState::new(doc);

    let html = state.export_standalone_html();
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<title>spec.md</title>"));
    assert!(html.contains("<h1>Test Heading</h1>"));
    assert!(html.contains("type=\"checkbox\" checked disabled"));
}
