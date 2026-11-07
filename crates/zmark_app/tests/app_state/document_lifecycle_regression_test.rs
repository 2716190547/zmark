use zmark_app::state::{AppState, DocKey};
use zmark_core::document::Document;
use std::fs;

#[test]
fn closing_last_document_clears_active_editor_and_allows_new_draft() {
    let mut state = AppState::new_isolated(Document::new_scratch());
    let original = state.active_key().cloned().unwrap();
    state.editor.insert_text("discarded draft");
    state.sync_from_editor();
    state.close_document_forced(&original);
    assert!(state.active_key().cloned().is_none());
    assert!(state.open_documents().is_empty());
    assert_eq!(state.editor.to_text(), "");
    assert!(!state.is_dirty());
    assert!(!state.has_conflict());
    state.focus_or_create_draft();
    assert!(state.active_key().unwrap().is_scratch());
    assert_eq!(state.editor.to_text(), "");
}

#[test]
fn failed_save_keeps_local_undo_and_content_across_switches() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("removed");
    fs::create_dir(&folder).unwrap();
    let first = folder.join("first.md");
    let second = dir.path().join("second.md");
    fs::write(&first, "original").unwrap();
    fs::write(&second, "second").unwrap();
    let mut state = AppState::new_isolated(Document::from_file(first.clone()).unwrap());
    state.editor.insert_text(" local");
    state.sync_from_editor();
    let edited = state.editor.to_text();
    fs::remove_file(&first).unwrap();
    fs::remove_dir(&folder).unwrap();
    assert!(state.save().is_err());
    assert!(state.is_dirty());
    assert_eq!(state.editor.to_text(), edited);
    assert!(state.editor.undo());
    state.sync_from_editor();
    assert_eq!(state.editor.to_text(), "original");
    state.editor.insert_text(" retained");
    state.sync_from_editor();
    let retained = state.editor.to_text();
    state.open_file(second).unwrap();
    state.switch_document(&DocKey::Path(first));
    assert_eq!(state.editor.to_text(), retained);
    assert!(state.is_dirty());
}
