use zmark_app::state::{AppState, CloseDecision, DocKey, RootAddError, ViewMode};
use zmark_core::config::ThemeMode;
use zmark_core::document::Document;
use zmark_core::i18n::SupportedLocale;
use tempfile::tempdir;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_app_state_initialization_and_mode_switching() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

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
    let mut state = AppState::new_isolated(doc);

    // Initially drawer closed
    assert!(!state.drawer_open);

    // Add workspace root and verify file tree is loaded correctly
    let idx = state.add_workspace_root(dir.path().to_path_buf()).unwrap();
    assert_eq!(idx, 0);
    assert_eq!(state.workspace_roots[0].tree.len(), 2);
    assert!(state.workspace_roots[0].tree.iter().any(|node| node.name == "doc1.md"));
    assert!(state.workspace_roots[0].tree.iter().any(|node| node.name == "doc2.md"));
    assert!(!state.workspace_roots[0].tree.iter().any(|node| node.name == "ignored.txt"));

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
    let state = AppState::new_isolated(doc);

    let html = state.export_standalone_html();
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<title>spec.md</title>"));
    assert!(html.contains("<h1>Test Heading</h1>"));
    assert!(html.contains("type=\"checkbox\" checked disabled"));
}

#[test]
fn test_theme_palettes_and_appearance_modes() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

    // Verify 8 palettes can all be set cleanly
    let palettes = [
        "default", "claude", "macos", "tokyo-night", "nord", "github", "solarized", "one-dark",
    ];
    for palette in palettes {
        state.set_palette(palette);
        assert_eq!(state.preferences.palette, palette);
        assert_eq!(state.theme.id, palette);
    }

    // Verify appearance mode switching
    state.set_theme_mode(ThemeMode::Light);
    assert_eq!(state.preferences.theme, ThemeMode::Light);
    assert!(!state.theme.is_dark);

    state.set_theme_mode(ThemeMode::Dark);
    assert_eq!(state.preferences.theme, ThemeMode::Dark);
    assert!(state.theme.is_dark);

    // Auto with appearance update
    state.set_theme_mode(ThemeMode::Auto);
    state.update_appearance(false);
    assert!(!state.theme.is_dark);
    state.update_appearance(true);
    assert!(state.theme.is_dark);

    // Switching palette in Auto mode when system is light should stay light
    state.update_appearance(false);
    state.set_theme_mode(ThemeMode::Auto);
    state.set_palette("forest");
    assert!(!state.theme.is_dark);
    assert_eq!(state.preferences.palette, "forest");
}

#[test]
fn test_locale_switching_and_strings() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

    state.set_locale(SupportedLocale::ZhCn);
    assert_eq!(state.strings.app_title, "ZMark");
    assert_eq!(state.strings.save_file, "保存");
    assert_eq!(state.strings.open_file, "打开");

    state.set_locale(SupportedLocale::En);
    assert_eq!(state.strings.save_file, "Save");
    assert_eq!(state.strings.open_file, "Open");

    state.set_locale(SupportedLocale::Ja);
    assert_eq!(state.strings.save_file, "保存");
    assert_eq!(state.strings.open_file, "開く");
}

#[test]
fn test_editor_typing_and_state_sync() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

    state.editor.insert_text("# Hello World");
    state.editor.insert_newline();
    state.editor.insert_text("120fps GPU editor");
    state.sync_from_editor();

    assert!(state.doc().buffer.contains("# Hello World"));
    assert!(state.doc().buffer.contains("120fps GPU editor"));
    assert!(state.parse_is_stale());
    state.parse_now();
    assert_eq!(state.doc().parsed.blocks.len(), 2);
}

#[test]
fn test_rename_file_on_disk() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("original.md");
    fs::write(&file, "# Original Content").unwrap();

    let doc = Document::from_file(file.clone()).unwrap();
    let mut state = AppState::new_isolated(doc);

    // Rename to renamed.md
    state.rename_file("renamed.md").unwrap();
    assert_eq!(state.doc().path.as_ref().unwrap().file_name().unwrap(), "renamed.md");
    assert!(!file.exists());
    assert!(dir.path().join("renamed.md").exists());
}

#[test]
fn test_auto_save_and_settings_tabs() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("autosave_doc.md");
    fs::write(&file, "Initial text").unwrap();

    let doc = Document::from_file(file.clone()).unwrap();
    let mut state = AppState::new_isolated(doc);
    state.preferences = zmark_core::config::Preferences::default();

    // Verify auto_save defaults to true and tabs can be switched
    assert!(state.preferences.auto_save);
    assert_eq!(state.preferences.auto_save_delay_ms, 1000);
    assert_eq!(state.settings_tab, zmark_app::state::SettingsTab::Appearance);

    // Switch tabs
    state.set_settings_tab(zmark_app::state::SettingsTab::EditorSave);
    assert_eq!(state.settings_tab, zmark_app::state::SettingsTab::EditorSave);

    state.set_settings_tab(zmark_app::state::SettingsTab::Language);
    assert_eq!(state.settings_tab, zmark_app::state::SettingsTab::Language);

    state.set_settings_tab(zmark_app::state::SettingsTab::About);
    assert_eq!(state.settings_tab, zmark_app::state::SettingsTab::About);

    // Toggle auto-save
    state.set_auto_save(false);
    assert!(!state.preferences.auto_save);
    state.set_auto_save(true);
    assert!(state.preferences.auto_save);

    // Test tab size
    state.set_tab_size(2);
    assert_eq!(state.preferences.tab_size, 2);
    state.set_tab_size(4);
    assert_eq!(state.preferences.tab_size, 4);

    // Verify editing dirty state and saving to disk
    state.editor.insert_text(" - updated");
    state.sync_from_editor();
    assert!(state.is_dirty());
    state.save().unwrap();
    assert!(!state.is_dirty());
    let saved_content = fs::read_to_string(&file).unwrap();
    assert!(saved_content.contains("updated"));
}

#[test]
fn test_document_lifecycle_and_switching() {
    let dir = tempdir().unwrap();
    let file1 = dir.path().join("first.md");
    let file2 = dir.path().join("second.md");
    fs::write(&file1, "# First Document").unwrap();
    fs::write(&file2, "# Second Document").unwrap();

    let doc = Document::from_file(file1.clone()).unwrap();
    let mut state = AppState::new_isolated(doc);

    assert_eq!(state.open_documents().len(), 1);
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file1.clone())));

    // Open second file: adds to open_docs and activates it
    state.open_file(file2.clone()).unwrap();
    assert_eq!(state.open_documents().len(), 2);
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file2.clone())));
    assert_eq!(state.doc().path.as_ref(), Some(&file2));

    // Switch back to file 1 using DocKey
    state.switch_document(&DocKey::Path(file1.clone()));
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file1.clone())));
    assert_eq!(state.doc().path.as_ref(), Some(&file1));

    // Open draft slot
    state.focus_or_create_draft();
    assert_eq!(state.open_documents().len(), 3);
    assert!(state.active_key().map_or(false, |k| k.is_scratch()));
    assert!(state.doc().path.is_none());

    let draft_key = state.active_key().cloned().unwrap();

    // Close the draft
    state.close_document_forced(&draft_key);
    assert_eq!(state.open_documents().len(), 2);

    // Re-opening an already open file should switch to it, not duplicate
    state.open_file(file1.clone()).unwrap();
    assert_eq!(state.open_documents().len(), 2);
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file1.clone())));
}

#[test]
fn test_workspace_folder_and_tabs() {
    let root = tempdir().unwrap();
    let sub = root.path().join("subfolder");
    fs::create_dir_all(&sub).unwrap();
    let f1 = root.path().join("root.md");
    let f2 = sub.join("nested.md");
    fs::write(&f1, "# Root").unwrap();
    fs::write(&f2, "# Nested").unwrap();

    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

    let canonical_root = fs::canonicalize(root.path()).unwrap();
    let idx = state.add_workspace_root(root.path().to_path_buf()).unwrap();
    assert_eq!(idx, 0);
    assert_eq!(state.workspace_roots.len(), 1);
    assert_eq!(state.workspace_roots[0].path, canonical_root);
    assert!(state.preferences.workspace_roots.contains(&canonical_root));

    let label = zmark_app::state::root_label(&state.workspace_roots, 0);
    let expected_name = canonical_root.file_name().unwrap().to_str().unwrap();
    assert_eq!(label, expected_name);
}

#[test]
fn test_document_transitions_and_buffer_sync() {
    let dir = tempdir().unwrap();
    let file_a = dir.path().join("doc_a.md");
    let file_b = dir.path().join("doc_b.md");
    let file_c = dir.path().join("doc_c.md");
    fs::write(&file_a, "# Doc A").unwrap();
    fs::write(&file_b, "# Doc B").unwrap();
    fs::write(&file_c, "# Doc C").unwrap();

    let doc_a = Document::from_file(file_a.clone()).unwrap();
    let mut state = AppState::new_isolated(doc_a);

    state.open_file(file_b.clone()).unwrap();
    state.open_file(file_c.clone()).unwrap();
    assert_eq!(state.open_documents().len(), 3);
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file_c.clone())));

    // 1. Buffer synchronization when switching documents
    state.editor.insert_text("\nAdded to C");
    state.switch_document(&DocKey::Path(file_a.clone()));
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file_a.clone())));
    assert_eq!(state.doc().path.as_ref(), Some(&file_a));
    assert!(state.open_documents().iter().find(|d| d.key == DocKey::Path(file_c.clone())).unwrap().doc.buffer.contains("Added to C"));

    state.editor.insert_text("\nAdded to A");
    state.switch_document(&DocKey::Path(file_b.clone()));
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file_b.clone())));
    assert_eq!(state.doc().path.as_ref(), Some(&file_b));
    assert!(state.open_documents().iter().find(|d| d.key == DocKey::Path(file_a.clone())).unwrap().doc.buffer.contains("Added to A"));

    // 2. Closing inactive document: close Doc A while Doc B is active
    state.close_document_forced(&DocKey::Path(file_a.clone()));
    assert_eq!(state.open_documents().len(), 2);
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file_b.clone())));

    // 3. Re-open Doc A and close it
    state.open_file(file_a.clone()).unwrap();
    assert_eq!(state.open_documents().len(), 3);
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file_a.clone())));
    state.switch_document(&DocKey::Path(file_b.clone()));
    state.close_document_forced(&DocKey::Path(file_a.clone()));
    assert_eq!(state.open_documents().len(), 2);
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file_b.clone())));

    // 4. Closing the active document (Doc B): activates neighbor Doc C
    state.close_document_forced(&DocKey::Path(file_b.clone()));
    assert_eq!(state.open_documents().len(), 1);
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file_c.clone())));
    assert!(state.doc().buffer.contains("Added to C"));

    // 5. Closing the last remaining document leaves open_docs empty and active None (Empty State)
    state.close_document_forced(&DocKey::Path(file_c.clone()));
    assert_eq!(state.open_documents().len(), 0);
    assert_eq!(state.active_key().cloned(), None);
    assert!(!state.is_dirty());
}

#[test]
fn test_dockey_stable_switching() {
    let dir = tempdir().unwrap();
    let file1 = dir.path().join("d1.md");
    let file2 = dir.path().join("d2.md");
    fs::write(&file1, "Doc 1").unwrap();
    fs::write(&file2, "Doc 2").unwrap();

    let doc = Document::from_file(file1.clone()).unwrap();
    let mut state = AppState::new_isolated(doc);
    state.open_file(file2.clone()).unwrap();
    state.focus_or_create_draft();

    let k1 = DocKey::Path(file1.clone());
    let k2 = DocKey::Path(file2.clone());
    let k_draft = state.active_key().cloned().unwrap();

    // Switch between all 3 in arbitrary order
    state.switch_document(&k1);
    assert_eq!(state.active_key().cloned(), Some(k1.clone()));
    assert_eq!(state.doc().buffer, "Doc 1");

    state.switch_document(&k_draft);
    assert_eq!(state.active_key().cloned(), Some(k_draft.clone()));

    state.switch_document(&k2);
    assert_eq!(state.active_key().cloned(), Some(k2.clone()));
    assert_eq!(state.doc().buffer, "Doc 2");
}

#[test]
fn test_close_dirty_document_decision() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("dirty_test.md");
    fs::write(&file, "Original text").unwrap();

    let doc = Document::from_file(file.clone()).unwrap();
    let mut state = AppState::new_isolated(doc);
    let key = DocKey::Path(file.clone());

    // Clean doc: closes immediately
    let decision = state.plan_close(&key);
    assert_eq!(decision, Some(CloseDecision::CloseImmediately));

    // Modify editor text
    state.editor.insert_text(" updated");
    state.sync_from_editor();

    // Dirty doc: prompts Save / Discard / Cancel
    let decision = state.plan_close(&key);
    match decision {
        Some(CloseDecision::PromptDirty { key: dk, has_path, .. }) => {
            assert_eq!(dk, key);
            assert!(has_path);
        }
        _ => panic!("Expected PromptDirty"),
    }

    // request_close sets pending_close
    let req = state.request_close(&key);
    assert!(matches!(req, CloseDecision::PromptDirty { .. }));
    assert!(state.pending_close.is_some());

    // Cancel preserves document
    state.cancel_close();
    assert!(state.pending_close.is_none());
    assert_eq!(state.open_documents().len(), 1);
}

#[test]
fn test_confirm_close_save_and_discard() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("save_discard.md");
    fs::write(&file, "Base text").unwrap();

    let doc = Document::from_file(file.clone()).unwrap();
    let mut state = AppState::new_isolated(doc);
    let key = DocKey::Path(file.clone());

    // 1. Discard changes on close
    state.editor.insert_text(" - discarded modification");
    state.sync_from_editor();
    state.request_close(&key);
    state.confirm_close_discard(&key);

    assert_eq!(state.open_documents().len(), 0);
    assert_eq!(state.active_key().cloned(), None);
    // File on disk was NOT updated
    let disk_text = fs::read_to_string(&file).unwrap();
    assert_eq!(disk_text, "Base text");

    // 2. Save changes on close
    state.open_file(file.clone()).unwrap();
    state.editor.insert_text(" - saved modification");
    state.sync_from_editor();
    state.request_close(&key);
    let saved = state.confirm_close_save(&key).unwrap();
    assert!(saved);

    assert_eq!(state.open_documents().len(), 0);
    let disk_text = fs::read_to_string(&file).unwrap();
    assert!(disk_text.contains("- saved modification"));
}

#[test]
fn test_draft_save_as_decision() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("newly_saved.md");

    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

    state.editor.insert_text("# Brand New Draft Content");
    state.sync_from_editor();

    let scratch_key = state.active_key().cloned().unwrap();
    assert!(scratch_key.is_scratch());

    // Save As target path: in-place promotion from DocKey::Scratch to DocKey::Path
    state.save_doc_as(&scratch_key, target.clone()).unwrap();

    // Check disk content
    assert!(target.exists());
    let content = fs::read_to_string(&target).unwrap();
    assert!(content.contains("# Brand New Draft Content"));

    // Key transitioned to Path; no old Scratch key left
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(target.clone())));
    assert_eq!(state.open_documents()[0].key, DocKey::Path(target));
    assert!(state.open_documents().iter().all(|od| !od.key.is_scratch()));
    assert_eq!(state.open_documents().len(), 1);

    // Because previous scratch became a path file, Cmd+N can create a new scratch
    state.focus_or_create_draft();
    assert_eq!(state.open_documents().len(), 2);
    assert!(state.active_key().map_or(false, |k| k.is_scratch()));
}

#[test]
fn test_single_draft_slot_constraint() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);
    state.editor.insert_text("Draft notes");
    state.sync_from_editor();

    assert_eq!(state.open_documents().len(), 1);
    let first_draft_key = state.active_key().cloned().unwrap();

    // Repeated calls to focus_or_create_draft must not create multiple drafts
    state.focus_or_create_draft();
    assert_eq!(state.open_documents().len(), 1);
    assert_eq!(state.active_key().cloned(), Some(first_draft_key.clone()));

    // Open another file, then call focus_or_create_draft -> switches back to draft
    let dir = tempdir().unwrap();
    let file = dir.path().join("other.md");
    fs::write(&file, "Other").unwrap();

    state.open_file(file.clone()).unwrap();
    assert_eq!(state.open_documents().len(), 2);
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(file)));

    state.focus_or_create_draft();
    assert_eq!(state.open_documents().len(), 2);
    assert_eq!(state.active_key().cloned(), Some(first_draft_key));
}

#[test]
fn test_exit_window_dirty_save_strategy() {
    let dir = tempdir().unwrap();
    let f1 = dir.path().join("f1.md");
    fs::write(&f1, "Original").unwrap();

    let doc = Document::from_file(f1.clone()).unwrap();
    let mut state = AppState::new_isolated(doc);

    state.editor.insert_text(" Edited");
    state.sync_from_editor();
    assert!(state.is_dirty());

    // When dirty, handle_window_should_close returns false synchronously, sets is_quitting, and opens prompt modal
    let can_close = state.handle_window_should_close();
    assert!(!can_close);
    assert!(state.is_quitting);
    assert!(state.pending_close.is_some());

    // User confirms save for pending dirty doc
    let pending_key = state.pending_close.as_ref().unwrap().key.clone();
    let save_ok = state.confirm_close_save(&pending_key).unwrap();
    assert!(save_ok);

    let disk = fs::read_to_string(&f1).unwrap();
    assert!(disk.contains("Edited"));

    // Now clean, next dirty check has no dirty docs
    assert!(state.next_dirty_doc_key().is_none());
}

#[test]
fn test_window_should_close_blocks_on_dirty_draft() {
    let mut state = AppState::new_isolated(Document::new_scratch());
    state.editor.insert_text("Unsaved notes in draft");
    state.sync_from_editor();
    assert!(state.is_dirty());

    // Because there is a dirty unpathed draft, window closing is blocked
    let can_close = state.handle_window_should_close();
    assert!(!can_close);
    assert!(state.pending_close.is_some());
}

#[test]
fn test_background_dirty_document_switches_on_close_request() {
    let dir = tempdir().unwrap();
    let f1 = dir.path().join("doc1.md");
    let f2 = dir.path().join("doc2.md");
    fs::write(&f1, "Doc 1").unwrap();
    fs::write(&f2, "Doc 2").unwrap();

    let doc1 = Document::from_file(f1.clone()).unwrap();
    let mut state = AppState::new_isolated(doc1);
    state.open_file(f2.clone()).unwrap();

    // Now f2 is active. Switch to f1, edit it, then switch back to f2
    let k1 = DocKey::Path(f1.clone());
    let k2 = DocKey::Path(f2.clone());

    state.switch_document(&k1);
    state.editor.insert_text(" modified");
    state.sync_from_editor();

    state.switch_document(&k2);
    assert_eq!(state.active_key().cloned(), Some(k2));

    // Request closing background dirty doc k1
    let decision = state.request_close(&k1);
    assert!(matches!(decision, CloseDecision::PromptDirty { .. }));

    // Automatically switched to k1 before presenting prompt
    assert_eq!(state.active_key().cloned(), Some(k1));
    assert!(state.pending_close.is_some());
}

#[test]
fn test_save_doc_as_failure_preserves_scratch_key() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);
    state.editor.insert_text("Preserved scratch content");
    state.sync_from_editor();

    let scratch_key = state.active_key().cloned().unwrap();
    assert!(scratch_key.is_scratch());

    // Attempt to save to an illegal/nonexistent path (atomic save failure)
    let invalid_path = PathBuf::from("/nonexistent_forbidden_directory_zmark/sub/doc.md");
    let res = state.save_doc_as(&scratch_key, invalid_path);
    assert!(res.is_err());

    // Verify atomic rollback: state retains scratch identity and content completely
    assert_eq!(state.active_key().cloned(), Some(scratch_key.clone()));
    assert_eq!(state.open_documents().len(), 1);
    assert_eq!(state.open_documents()[0].key, scratch_key);
    assert!(state.doc().path.is_none());
    assert_eq!(state.doc().buffer, "Preserved scratch content");
}

#[test]
fn test_save_doc_as_target_already_open_returns_err() {
    let dir = tempdir().unwrap();
    let file_a = dir.path().join("a.md");
    fs::write(&file_a, "# Doc A").unwrap();

    let doc_a = Document::from_file(file_a.clone()).unwrap();
    let mut state = AppState::new_isolated(doc_a);

    state.focus_or_create_draft();
    let scratch_key = state.active_key().cloned().unwrap();
    assert!(scratch_key.is_scratch());

    // Attempting to Save As a path that is already open in open_docs must return Err(AlreadyExists)
    let res = state.save_doc_as(&scratch_key, file_a.clone());
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);

    // Scratch remains scratch, no duplicated keys in open_docs
    assert_eq!(state.active_key().cloned(), Some(scratch_key.clone()));
    assert_eq!(state.open_documents().len(), 2);
    assert!(state.open_documents().iter().any(|d| d.key == scratch_key));
    assert_eq!(
        state.open_documents().iter().filter(|d| d.key == DocKey::Path(file_a.clone())).count(),
        1
    );
}

#[test]
fn test_quit_draft_save_as_continues_exit_pipeline() {
    let dir = tempdir().unwrap();
    let f_pathed = dir.path().join("existing.md");
    let target_save = dir.path().join("saved_draft.md");
    fs::write(&f_pathed, "Original").unwrap();

    let doc_pathed = Document::from_file(f_pathed.clone()).unwrap();
    let mut state = AppState::new_isolated(doc_pathed);

    // Make pathed doc dirty
    state.editor.insert_text(" (pathed dirty)");
    state.sync_from_editor();

    // Create draft and make dirty
    state.focus_or_create_draft();
    let draft_key = state.active_key().cloned().unwrap();
    state.editor.insert_text("Draft dirty notes");
    state.sync_from_editor();

    // 1. Trigger window close: synchronous return false, is_quitting = true
    let can_close = state.handle_window_should_close();
    assert!(!can_close);
    assert!(state.is_quitting);
    assert!(state.pending_close.is_some());

    // 2. Resolve draft via Save As
    state.save_doc_as(&draft_key, target_save.clone()).unwrap();
    state.close_document_forced(&draft_key);

    // 3. Quitting continues: next dirty document (f_pathed) is identified
    assert!(state.is_quitting);
    let next_dirty = state.next_dirty_doc_key();
    assert_eq!(next_dirty, Some(DocKey::Path(f_pathed.clone())));

    // 4. Resolve pathed document
    let decision = state.request_close(&DocKey::Path(f_pathed.clone()));
    assert!(matches!(decision, CloseDecision::PromptDirty { .. }));
    let save_ok = state.confirm_close_save(&DocKey::Path(f_pathed.clone())).unwrap();
    assert!(save_ok);

    // 5. All dirty documents resolved, ready to remove_window
    assert!(state.next_dirty_doc_key().is_none());
}

#[test]
fn test_rename_file_boundaries_and_error_handling() {
    let dir = tempdir().unwrap();
    let f1 = dir.path().join("file1.md");
    let f2 = dir.path().join("file2.md");
    fs::write(&f1, "File 1").unwrap();
    fs::write(&f2, "File 2").unwrap();

    let doc1 = Document::from_file(f1.clone()).unwrap();
    let mut state = AppState::new_isolated(doc1);
    state.open_file(f2.clone()).unwrap();

    // 1. Rename to existing file on disk returns Err(AlreadyExists)
    state.switch_document(&DocKey::Path(f1.clone()));
    let res = state.rename_file("file2");
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);

    // 2. Draft rename changes its in-memory title without creating a file.
    state.focus_or_create_draft();
    let res_draft = state.rename_file("draft_renamed");
    assert!(res_draft.is_ok());
    assert!(state.doc().path.is_none());
    assert_eq!(state.doc().display_name_with_fallback("Untitled"), "draft_renamed.md");

    // 3. Normal rename to non-existing name succeeds
    state.switch_document(&DocKey::Path(f1.clone()));
    let res_ok = state.rename_file("file1_renamed");
    assert!(res_ok.is_ok());
    let new_path = dir.path().join("file1_renamed.md");
    assert!(new_path.exists());
    assert!(!f1.exists());
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(new_path)));
}

#[test]
fn test_workspace_folder_symlink_defense_and_robustness() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // 1. Regular markdown files with various casings
    let f1 = root.join("test.md");
    let f2 = root.join("UPPER.MD");
    let f3 = root.join("Mixed.Markdown");
    let non_md = root.join("notes.txt");
    fs::write(&f1, "# Test 1").unwrap();
    fs::write(&f2, "# Upper").unwrap();
    fs::write(&f3, "# Mixed").unwrap();
    fs::write(&non_md, "plain text").unwrap();

    // 2. Symlink loop: root/loop -> root
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let loop_link = root.join("symlink_loop");
        let _ = symlink(root, &loop_link);
    }

    // 3. Nested folder and hidden folder
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    let f_sub = sub.join("nested.md");
    fs::write(&f_sub, "# Nested").unwrap();

    let hidden = root.join(".hidden");
    fs::create_dir_all(&hidden).unwrap();
    let f_hidden = hidden.join("secret.md");
    fs::write(&f_hidden, "# Secret").unwrap();

    let node_modules = root.join("node_modules");
    fs::create_dir_all(&node_modules).unwrap();
    let f_nm = node_modules.join("package.md");
    fs::write(&f_nm, "# Package").unwrap();

    let mut state = AppState::new_isolated(Document::new_scratch());
    let canonical_root = fs::canonicalize(root).unwrap();

    // Adding valid root canonicalizes path and scans tree safely without infinite loop
    let idx = state.add_workspace_root(root.to_path_buf()).unwrap();
    assert_eq!(idx, 0);
    assert_eq!(state.workspace_roots[0].path, canonical_root);

    // .hidden and node_modules are ignored in the scanned tree
    let tree = &state.workspace_roots[0].tree;
    assert!(!tree.iter().any(|node| node.name == ".hidden"));
    assert!(!tree.iter().any(|node| node.name == "node_modules"));
    assert!(tree.iter().any(|node| node.name == "sub"));
    assert!(tree.iter().any(|node| node.name == "test.md"));

    // Passing a file path returns NotADirectory
    assert_eq!(
        state.add_workspace_root(f1.clone()),
        Err(RootAddError::NotADirectory)
    );

    // Adding a nested child directory is rejected with NestedInExisting
    assert_eq!(
        state.add_workspace_root(sub.clone()),
        Err(RootAddError::NestedInExisting(canonical_root.clone()))
    );

    // Duplicate addition is rejected with AlreadyExists
    assert_eq!(
        state.add_workspace_root(root.to_path_buf()),
        Err(RootAddError::AlreadyExists)
    );

    // Adding an empty directory works cleanly
    let empty_dir = tempdir().unwrap();
    let empty_canonical = fs::canonicalize(empty_dir.path()).unwrap();
    let empty_idx = state.add_workspace_root(empty_dir.path().to_path_buf()).unwrap();
    assert_eq!(empty_idx, 1);
    assert_eq!(state.workspace_roots[1].path, empty_canonical);
    assert!(state.workspace_roots[1].tree.is_empty());
}

#[test]
fn test_workspace_tree_recursive_scan() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();

    // Create structure:
    // root/
    //   guide.md
    //   docs/
    //     api.md
    //     internal/
    //       arch.md
    //   node_modules/
    //     ignored.md
    let docs = root.join("docs");
    let internal = docs.join("internal");
    let nm = root.join("node_modules");
    fs::create_dir_all(&internal).unwrap();
    fs::create_dir_all(&nm).unwrap();

    let f_guide = root.join("guide.md");
    let f_api = docs.join("api.md");
    let f_arch = internal.join("arch.md");
    let f_ignored = nm.join("ignored.md");
    fs::write(&f_guide, "# Guide").unwrap();
    fs::write(&f_api, "# API").unwrap();
    fs::write(&f_arch, "# Arch").unwrap();
    fs::write(&f_ignored, "# Ignored").unwrap();

    let mut state = AppState::new_isolated(Document::new_scratch());
    let idx = state.add_workspace_root(root.to_path_buf()).unwrap();

    // Directory comes before file in root
    let tree = &state.workspace_roots[idx].tree;
    assert_eq!(tree.len(), 2);
    assert_eq!(tree[0].name, "docs");
    assert!(tree[0].is_dir);
    assert_eq!(tree[1].name, "guide.md");
    assert!(!tree[1].is_dir);

    // node_modules is ignored
    assert!(!tree.iter().any(|n| n.name == "node_modules"));

    // Expanding docs populates children (api.md and internal directory)
    let canonical_docs = fs::canonicalize(&docs).unwrap();
    state.toggle_root_folder(idx, &canonical_docs);
    let docs_node = &state.workspace_roots[idx].tree[0];
    assert!(docs_node.children.iter().any(|n| n.name == "api.md"));
    assert!(docs_node.children.iter().any(|n| n.name == "internal"));
}

#[test]
fn test_workspace_tree_expand_and_collapse() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    let docs = root.join("docs");
    fs::create_dir_all(&docs).unwrap();

    let f_root = root.join("index.md");
    let f_doc = docs.join("doc1.md");
    fs::write(&f_root, "# Index").unwrap();
    fs::write(&f_doc, "# Doc1").unwrap();

    let mut state = AppState::new_isolated(Document::new_scratch());
    let idx = state.add_workspace_root(root.to_path_buf()).unwrap();

    let canonical_docs = fs::canonicalize(&docs).unwrap();

    // Initially subdirectories are collapsed
    let tree_init = &state.workspace_roots[idx].tree;
    assert_eq!(tree_init.len(), 2);
    assert_eq!(tree_init[0].name, "docs");
    assert!(!tree_init[0].is_expanded);
    assert!(tree_init[0].children.is_empty());
    assert_eq!(tree_init[1].name, "index.md");

    // Expand "docs"
    let toggled = state.toggle_root_folder(idx, &canonical_docs);
    assert!(toggled);
    let tree_expanded = &state.workspace_roots[idx].tree;
    assert!(tree_expanded[0].is_expanded);
    assert_eq!(tree_expanded[0].children.len(), 1);
    assert_eq!(tree_expanded[0].children[0].name, "doc1.md");

    // Collapse "docs"
    let toggled_again = state.toggle_root_folder(idx, &canonical_docs);
    assert!(toggled_again);
    let tree_collapsed = &state.workspace_roots[idx].tree;
    assert!(!tree_collapsed[0].is_expanded);
    assert_eq!(tree_collapsed[0].children.len(), 1);
}

#[test]
fn test_workspace_tree_switch_folder() {
    let tmp1 = tempdir().unwrap();
    let f1 = tmp1.path().join("a.md");
    fs::write(&f1, "# A").unwrap();

    let tmp2 = tempdir().unwrap();
    let f2 = tmp2.path().join("b.md");
    fs::write(&f2, "# B").unwrap();

    let mut state = AppState::new_isolated(Document::new_scratch());
    let idx1 = state.add_workspace_root(tmp1.path().to_path_buf()).unwrap();
    assert_eq!(idx1, 0);
    assert_eq!(state.workspace_roots[0].tree.len(), 1);
    assert_eq!(state.workspace_roots[0].tree[0].name, "a.md");

    // Add root 2 alongside root 1
    let idx2 = state.add_workspace_root(tmp2.path().to_path_buf()).unwrap();
    assert_eq!(idx2, 1);
    assert_eq!(state.workspace_roots[1].tree.len(), 1);
    assert_eq!(state.workspace_roots[1].tree[0].name, "b.md");

    // Remove root 1, leaving root 2
    state.remove_workspace_root(0).unwrap();
    assert_eq!(state.workspace_roots.len(), 1);
    assert_eq!(state.workspace_roots[0].tree[0].name, "b.md");
}

#[test]
fn test_single_file_open_parent_fallback() {
    let tmp = tempdir().unwrap();
    let nested = tmp.path().join("projects").join("sample");
    fs::create_dir_all(&nested).unwrap();
    let f_single = nested.join("readme.md");
    let f_other = nested.join("spec.md");
    fs::write(&f_single, "# Sample Readme").unwrap();
    fs::write(&f_other, "# Spec").unwrap();

    // Start with empty scratch doc with no workspace roots
    let mut state = AppState::new_isolated(Document::new_scratch());
    assert!(state.workspace_roots.is_empty());

    // Open single file
    state.open_file(f_single.clone()).unwrap();

    // Decoupled: Opening a file does NOT automatically add its parent to workspace_roots
    assert!(state.workspace_roots.is_empty());
    assert_eq!(state.active_key().cloned(), Some(DocKey::Path(f_single.clone())));

    // The opened external file enters loose_docs
    let loose = state.loose_docs();
    assert!(loose.contains(&DocKey::Path(f_single)));
}

#[test]
fn test_background_save_failure_keeps_document_state() {
    let dir = tempdir().unwrap();
    let subdir = dir.path().join("removed");
    fs::create_dir(&subdir).unwrap();
    let first = subdir.join("first.md");
    let second = dir.path().join("second.md");
    fs::write(&first, "first").unwrap();
    fs::write(&second, "second").unwrap();
    let mut state = AppState::new_isolated(Document::from_file(first.clone()).unwrap());
    state.open_file(second).unwrap();
    let key = DocKey::Path(first.clone());
    let before = state.open_documents().iter().find(|doc| doc.key == key).unwrap().doc.clone();
    fs::remove_file(first).unwrap();
    fs::remove_dir(subdir).unwrap();
    assert!(state.save_doc(&key).is_err());
    assert_eq!(state.open_documents().iter().find(|doc| doc.key == key).unwrap().doc, before);
}

#[test]
fn test_conflicted_active_document_requires_close_and_exit_confirmation() {
    use zmark_core::document::DocumentStatus;
    let mut doc = Document::new_scratch();
    doc.status = DocumentStatus::ConflictOnDisk;
    let mut state = AppState::new_isolated(doc);
    let key = state.active_key().cloned().unwrap();
    assert!(state.is_dirty());
    assert!(matches!(state.plan_close(&key), Some(CloseDecision::PromptDirty { .. })));
    assert!(!state.handle_window_should_close());
    assert!(state.pending_close.is_some());
}

#[test]
fn test_conflicted_background_document_requires_confirmation() {
    use zmark_core::document::DocumentStatus;
    let dir = tempdir().unwrap();
    let first = dir.path().join("first.md");
    let second = dir.path().join("second.md");
    fs::write(&first, "first").unwrap();
    fs::write(&second, "second").unwrap();
    let mut doc = Document::from_file(first.clone()).unwrap();
    doc.status = DocumentStatus::ConflictOnDisk;
    let mut state = AppState::new_isolated(doc);
    let key = DocKey::Path(first);
    state.open_file(second).unwrap();
    assert!(matches!(state.plan_close(&key), Some(CloseDecision::PromptDirty { .. })));
    assert_eq!(state.next_dirty_doc_key(), Some(key));
    assert!(!state.handle_window_should_close());
    assert!(state.pending_close.is_some());
}
