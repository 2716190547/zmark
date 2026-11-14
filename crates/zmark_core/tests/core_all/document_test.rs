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
fn source_file_highlighting_preserves_unicode_crlf_and_first_line() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("example.TS");
    let source = "const 名称: string = '🙂';\r\nconsole.log(名称);";
    fs::write(&path, source).unwrap();
    let doc = Document::from_file(path).unwrap();
    let block = &doc.parsed.code_blocks[0];
    assert_eq!(block.child_path, "source-file");
    assert_eq!(block.key.payload.as_ref(), source);
    assert!(block.key.language.is_some());
    assert_eq!(block.lines[0].source_line_idx, 0);
    assert_eq!(block.lines[0].eol_len, 2);
    assert_eq!(block.lines[1].eol_len, 0);
    for line in &block.lines {
        assert!(source.is_char_boundary(line.payload_range.start));
        assert!(source.is_char_boundary(line.payload_range.end));
    }
    assert_eq!(doc.buffer, source);
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

#[test]
fn saving_refuses_external_changes_until_the_exact_disk_version_is_confirmed() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("conflict.md");
    fs::write(&path, "original").unwrap();
    let mut doc = Document::from_file(path.clone()).unwrap();
    doc.set_buffer("local edit".into());
    fs::write(&path, "external edit").unwrap();
    assert!(doc.save().is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "external edit");
    assert_eq!(doc.buffer, "local edit");
    assert_eq!(doc.status, DocumentStatus::ConflictOnDisk);
    doc.resolve_conflict_keep_local();
    fs::write(&path, "newer external edit").unwrap();
    assert!(doc.save().is_err(), "confirmation must not authorize a later external version");
    assert_eq!(fs::read_to_string(&path).unwrap(), "newer external edit");
    doc.resolve_conflict_keep_local();
    doc.save().unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "local edit");
    assert_eq!(doc.status, DocumentStatus::Clean);
}

#[test]
fn saving_a_deleted_file_does_not_recreate_it_or_clear_local_changes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("deleted.md");
    fs::write(&path, "original").unwrap();
    let mut doc = Document::from_file(path.clone()).unwrap();
    doc.set_buffer("local".into());
    fs::remove_file(&path).unwrap();
    assert!(doc.save().is_err());
    assert!(!path.exists());
    assert_eq!(doc.buffer, "local");
    assert_eq!(doc.status, DocumentStatus::Modified);
}

#[cfg(unix)]
#[test]
fn atomic_save_preserves_permissions_and_symlink_but_refuses_hardlinks() {
    use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
    let dir = tempdir().unwrap();
    let path = dir.path().join("note.md");
    let link = dir.path().join("link.md");
    fs::write(&path, "original").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let inode = fs::metadata(&path).unwrap().ino();
    symlink(&path, &link).unwrap();
    let mut doc = Document::from_file(link.clone()).unwrap();
    doc.set_buffer("saved".into());
    doc.save().unwrap();
    assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_ne!(fs::metadata(&path).unwrap().ino(), inode, "save must replace, not truncate, the original inode");
    assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o640);
    let hardlink = dir.path().join("hard.md");
    fs::hard_link(&path, &hardlink).unwrap();
    doc.set_buffer("must not write".into());
    assert!(doc.save().is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "saved");
    assert_eq!(fs::read_to_string(&hardlink).unwrap(), "saved");
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3, "failed save must remove its temporary file");
}

#[test]
fn save_as_failure_keeps_draft_identity_and_rejects_readonly_targets() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("readonly.md");
    fs::write(&path, "original").unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    let restore = permissions.clone();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let mut doc = Document::new_scratch_with_text("draft".into());
    doc.rename_scratch("named").unwrap();
    let before = doc.clone();
    assert!(doc.save_to(path.clone()).is_err());
    assert_eq!(doc, before);
    assert_eq!(fs::read_to_string(&path).unwrap(), "original");
    fs::set_permissions(&path, restore).unwrap();
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
