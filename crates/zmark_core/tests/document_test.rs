use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use zmark_core::document::{Document, DocumentStatus};

#[test]
fn test_scratch_document_lifecycle() {
    let mut doc = Document::new_scratch();
    assert_eq!(doc.status, DocumentStatus::Clean);
    assert_eq!(doc.buffer, "");
    assert!(doc.parsed.blocks.is_empty());

    doc.update_buffer("# Hello World\n\nNew note.".into());
    assert_eq!(doc.status, DocumentStatus::Modified);
    assert_eq!(doc.parsed.blocks.len(), 2);
}

#[test]
fn test_cloned_document_shares_parsed_ast() {
    let doc = Document::new_scratch();
    let clone = doc.clone();
    assert!(Arc::ptr_eq(&doc.parsed, &clone.parsed));
}

#[test]
fn test_set_buffer_defers_parse_until_reparse() {
    let mut doc = Document::new_scratch();
    assert!(doc.set_buffer("# One".into()));
    assert!(doc.is_parse_stale());
    assert!(doc.parsed.blocks.is_empty());

    assert!(doc.reparse());
    assert!(!doc.is_parse_stale());
    assert_eq!(doc.parsed.blocks.len(), 1);

    assert!(!doc.reparse());
}

#[test]
fn test_file_save_and_clean_transition() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "# Initial").unwrap();

    let mut doc = Document::from_file(file_path.clone()).expect("Should open file");
    assert_eq!(doc.status, DocumentStatus::Clean);
    assert_eq!(doc.parsed.blocks.len(), 1);

    // Modify buffer
    doc.update_buffer("# Initial\n\nAdded paragraph.".into());
    assert_eq!(doc.status, DocumentStatus::Modified);

    // Save
    doc.save().expect("Save should succeed");
    assert_eq!(doc.status, DocumentStatus::Clean);

    let on_disk = fs::read_to_string(&file_path).unwrap();
    assert_eq!(on_disk, "# Initial\n\nAdded paragraph.");
}

#[test]
fn test_external_disk_change_clean_silent_reload() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("external.md");
    fs::write(&file_path, "# Original").unwrap();

    let mut doc = Document::from_file(file_path).expect("Should open");
    assert_eq!(doc.status, DocumentStatus::Clean);

    // External change occurs when local buffer is Clean -> Silent reload
    doc.handle_disk_change("# Changed by Git".into());
    assert_eq!(doc.status, DocumentStatus::Clean);
    assert_eq!(doc.buffer, "# Changed by Git");
    assert_eq!(doc.parsed.blocks.len(), 1);
}

#[test]
fn test_external_disk_change_dirty_causes_conflict() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("conflict.md");
    fs::write(&file_path, "# Baseline").unwrap();

    let mut doc = Document::from_file(file_path).expect("Should open");
    doc.update_buffer("# My Unsaved Edits".into());
    assert_eq!(doc.status, DocumentStatus::Modified);

    // External disk change arrives while local buffer is dirty
    doc.handle_disk_change("# Upstream Commit".into());
    assert_eq!(doc.status, DocumentStatus::ConflictOnDisk);
    // User edits are NOT wiped out
    assert_eq!(doc.buffer, "# My Unsaved Edits");

    // Resolve conflict by keeping local edits
    doc.resolve_conflict_keep_local();
    assert_eq!(doc.status, DocumentStatus::Modified);
}

use zmark_core::watcher::{FileWatcher, WatcherEvent};
use std::time::Duration;

#[test]
fn test_file_watcher_detects_modification() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("watched.md");
    fs::write(&file_path, "initial content").unwrap();

    let watcher = FileWatcher::watch(&file_path).expect("Should start watch");

    // Give watcher time to attach on macOS FSEvents
    std::thread::sleep(Duration::from_millis(50));

    fs::write(&file_path, "modified content").unwrap();

    let event = watcher.rx.recv_timeout(Duration::from_secs(3));
    assert!(event.is_ok(), "Expected modification event within 3 seconds");
    match event.unwrap() {
        WatcherEvent::Modified(p) => assert_eq!(p, file_path.canonicalize().unwrap()),
        WatcherEvent::Removed(_) => panic!("Expected Modified event"),
    }
}
