use glidemark_app::state::{AppState, ViewMode};
use glidemark_core::config::ThemeMode;
use glidemark_core::document::Document;
use glidemark_core::i18n::SupportedLocale;
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

#[test]
fn test_theme_palettes_and_appearance_modes() {
    let doc = Document::new_scratch();
    let mut state = AppState::new(doc);

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
    let mut state = AppState::new(doc);

    state.set_locale(SupportedLocale::ZhCn);
    assert_eq!(state.strings.app_title, "GlideMark");
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
    let mut state = AppState::new(doc);

    state.editor.insert_text("# Hello World");
    state.editor.insert_newline();
    state.editor.insert_text("120fps GPU editor");
    state.sync_from_editor();

    assert!(state.doc.buffer.contains("# Hello World"));
    assert!(state.doc.buffer.contains("120fps GPU editor"));
    assert!(state.parse_is_stale());
    state.parse_now();
    assert_eq!(state.doc.parsed.blocks.len(), 2);
}

#[test]
fn test_rename_file_on_disk() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("original.md");
    fs::write(&file, "# Original Content").unwrap();

    let doc = Document::from_file(file.clone()).unwrap();
    let mut state = AppState::new(doc);

    // Rename to renamed.md
    state.rename_file("renamed.md").unwrap();
    assert_eq!(state.doc.path.as_ref().unwrap().file_name().unwrap(), "renamed.md");
    assert!(!file.exists());
    assert!(dir.path().join("renamed.md").exists());
}

#[test]
fn test_auto_save_and_settings_tabs() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("autosave_doc.md");
    fs::write(&file, "Initial text").unwrap();

    let doc = Document::from_file(file.clone()).unwrap();
    let mut state = AppState::new(doc);
    state.preferences = glidemark_core::config::Preferences::default();

    // Verify auto_save defaults to true and tabs can be switched
    assert!(state.preferences.auto_save);
    assert_eq!(state.preferences.auto_save_delay_ms, 1000);
    assert_eq!(state.settings_tab, glidemark_app::state::SettingsTab::Appearance);

    // Switch tabs
    state.set_settings_tab(glidemark_app::state::SettingsTab::EditorSave);
    assert_eq!(state.settings_tab, glidemark_app::state::SettingsTab::EditorSave);

    state.set_settings_tab(glidemark_app::state::SettingsTab::Language);
    assert_eq!(state.settings_tab, glidemark_app::state::SettingsTab::Language);

    state.set_settings_tab(glidemark_app::state::SettingsTab::About);
    assert_eq!(state.settings_tab, glidemark_app::state::SettingsTab::About);

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
fn test_multi_tab_workflow() {
    let dir = tempdir().unwrap();
    let file1 = dir.path().join("first.md");
    let file2 = dir.path().join("second.md");
    fs::write(&file1, "# First Document").unwrap();
    fs::write(&file2, "# Second Document").unwrap();

    let doc = Document::from_file(file1.clone()).unwrap();
    let mut state = AppState::new(doc);

    assert_eq!(state.open_docs.len(), 1);
    assert_eq!(state.active_doc_idx, 0);

    // Open second file: should add tab, not close the first!
    state.open_file(file2.clone()).unwrap();
    assert_eq!(state.open_docs.len(), 2);
    assert_eq!(state.active_doc_idx, 1);
    assert_eq!(state.doc.path.as_ref(), Some(&file2));

    // Switch back to tab 0
    state.switch_tab(0);
    assert_eq!(state.active_doc_idx, 0);
    assert_eq!(state.doc.path.as_ref(), Some(&file1));

    // Open a third scratch tab
    state.new_tab();
    assert_eq!(state.open_docs.len(), 3);
    assert_eq!(state.active_doc_idx, 2);
    assert!(state.doc.path.is_none());

    // Close the scratch tab
    state.close_tab(2);
    assert_eq!(state.open_docs.len(), 2);
    assert_eq!(state.active_doc_idx, 1);

    // Re-opening an already open file should switch to it, not duplicate
    state.open_file(file1.clone()).unwrap();
    assert_eq!(state.open_docs.len(), 2);
    assert_eq!(state.active_doc_idx, 0);
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
    let mut state = AppState::new(doc);

    // Set workspace folder to root
    state.set_workspace_folder(root.path().to_path_buf());
    assert_eq!(state.workspace_folder.as_ref(), Some(&root.path().to_path_buf()));
    assert_eq!(state.sibling_files.len(), 2);
    assert!(state.sibling_files.contains(&f1));
    assert!(state.sibling_files.contains(&f2));
}

#[test]
fn test_multi_tab_transitions_and_buffer_sync() {
    let dir = tempdir().unwrap();
    let file_a = dir.path().join("doc_a.md");
    let file_b = dir.path().join("doc_b.md");
    let file_c = dir.path().join("doc_c.md");
    fs::write(&file_a, "# Doc A").unwrap();
    fs::write(&file_b, "# Doc B").unwrap();
    fs::write(&file_c, "# Doc C").unwrap();

    let doc_a = Document::from_file(file_a.clone()).unwrap();
    let mut state = AppState::new(doc_a);

    state.open_file(file_b.clone()).unwrap();
    state.open_file(file_c.clone()).unwrap();
    assert_eq!(state.open_docs.len(), 3);
    assert_eq!(state.active_doc_idx, 2);

    // 1. Buffer synchronization when switching tabs
    // Edit Doc C in editor
    state.editor.insert_text("\nAdded to C");
    // Switch to Doc A (index 0)
    state.switch_tab(0);
    assert_eq!(state.active_doc_idx, 0);
    assert_eq!(state.doc.path.as_ref(), Some(&file_a));
    assert!(state.open_docs[2].buffer.contains("Added to C"));

    // Edit Doc A
    state.editor.insert_text("\nAdded to A");
    // Switch to Doc B (index 1)
    state.switch_tab(1);
    assert_eq!(state.active_doc_idx, 1);
    assert_eq!(state.doc.path.as_ref(), Some(&file_b));
    assert!(state.open_docs[0].buffer.contains("Added to A"));

    // 2. Closing inactive tab before active: close index 0 (Doc A) while active is 1 (Doc B)
    state.close_tab(0);
    assert_eq!(state.open_docs.len(), 2);
    // Active index should adjust from 1 to 0 (still pointing to Doc B)
    assert_eq!(state.active_doc_idx, 0);
    assert_eq!(state.doc.path.as_ref(), Some(&file_b));

    // 3. Closing inactive tab after active: open Doc A again, active is 2
    state.open_file(file_a.clone()).unwrap();
    assert_eq!(state.open_docs.len(), 3); // [Doc B, Doc C, Doc A]
    assert_eq!(state.active_doc_idx, 2);
    state.switch_tab(0); // Active is 0 (Doc B)
    state.close_tab(2); // Close Doc A (after active)
    assert_eq!(state.open_docs.len(), 2); // [Doc B, Doc C]
    assert_eq!(state.active_doc_idx, 0); // Stays 0 (Doc B)
    assert_eq!(state.doc.path.as_ref(), Some(&file_b));

    // 4. Closing the active tab at index 0: [Doc B, Doc C] -> close Doc B
    state.close_tab(0);
    assert_eq!(state.open_docs.len(), 1); // [Doc C]
    assert_eq!(state.active_doc_idx, 0);
    assert_eq!(state.doc.path.as_ref(), Some(&file_c));
    assert!(state.doc.buffer.contains("Added to C"));

    // 5. Closing the last remaining tab resets to a clean scratch document
    state.close_tab(0);
    assert_eq!(state.open_docs.len(), 1);
    assert_eq!(state.active_doc_idx, 0);
    assert!(state.doc.path.is_none());
    assert_eq!(state.doc.buffer, "");

    // 6. Out-of-bounds safety: switch_tab and close_tab with invalid indices
    state.switch_tab(42);
    assert_eq!(state.active_doc_idx, 0);
    state.close_tab(42);
    assert_eq!(state.open_docs.len(), 1);
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

    let mut state = AppState::new(Document::new_scratch());
    state.set_workspace_folder(root.to_path_buf());

    // Verified:
    // - f1, f2, f3, f_sub are detected (4 markdown files)
    // - .hidden and node_modules are ignored
    // - symlink loop does not cause infinite recursion or duplicate f1/f2/f3
    assert_eq!(state.sibling_files.len(), 4);
    assert!(state.sibling_files.contains(&f1));
    assert!(state.sibling_files.contains(&f2));
    assert!(state.sibling_files.contains(&f3));
    assert!(state.sibling_files.contains(&f_sub));
    assert!(!state.sibling_files.contains(&f_hidden));
    assert!(!state.sibling_files.contains(&f_nm));

    // 4. Passing a file path to set_workspace_folder gracefully resolves to parent dir
    state.set_workspace_folder(f1.clone());
    assert_eq!(state.workspace_folder.as_ref(), Some(&root.to_path_buf()));
    assert_eq!(state.sibling_files.len(), 4);

    // 5. Empty folder / 0 markdown files
    let empty_dir = tempdir().unwrap();
    state.set_workspace_folder(empty_dir.path().to_path_buf());
    assert_eq!(state.sibling_files.len(), 0);
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

    let mut state = AppState::new(Document::new_scratch());
    state.set_workspace_folder(root.to_path_buf());

    // Directory comes before file in root
    assert_eq!(state.file_tree.len(), 2);
    assert_eq!(state.file_tree[0].name, "docs");
    assert!(state.file_tree[0].is_dir);
    assert_eq!(state.file_tree[1].name, "guide.md");
    assert!(!state.file_tree[1].is_dir);

    // node_modules is ignored
    assert!(!state.file_tree.iter().any(|n| n.name == "node_modules"));

    // docs children are scanned (api.md and internal directory)
    assert!(state.file_tree[0].children.iter().any(|n| n.name == "api.md"));
    assert!(state.file_tree[0].children.iter().any(|n| n.name == "internal"));
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

    let mut state = AppState::new(Document::new_scratch());
    state.set_workspace_folder(root.to_path_buf());

    // Initially subdirectories are collapsed
    let rows_init = state.flatten_tree();
    // Only "docs" and "index.md" visible at depth 0
    assert_eq!(rows_init.len(), 2);
    assert_eq!(rows_init[0].name, "docs");
    assert!(!rows_init[0].is_expanded);
    assert_eq!(rows_init[0].depth, 0);
    assert_eq!(rows_init[1].name, "index.md");
    assert_eq!(rows_init[1].depth, 0);

    // Expand "docs"
    state.toggle_tree_folder(&docs);
    let rows_expanded = state.flatten_tree();
    assert_eq!(rows_expanded.len(), 3);
    assert_eq!(rows_expanded[0].name, "docs");
    assert!(rows_expanded[0].is_expanded);
    assert_eq!(rows_expanded[1].name, "doc1.md");
    assert_eq!(rows_expanded[1].depth, 1);
    assert_eq!(rows_expanded[2].name, "index.md");
    assert_eq!(rows_expanded[2].depth, 0);

    // Collapse "docs"
    state.toggle_tree_folder(&docs);
    let rows_collapsed = state.flatten_tree();
    assert_eq!(rows_collapsed.len(), 2);
    assert!(!rows_collapsed[0].is_expanded);
}

#[test]
fn test_workspace_tree_switch_folder() {
    let tmp1 = tempdir().unwrap();
    let f1 = tmp1.path().join("a.md");
    fs::write(&f1, "# A").unwrap();

    let tmp2 = tempdir().unwrap();
    let f2 = tmp2.path().join("b.md");
    fs::write(&f2, "# B").unwrap();

    let mut state = AppState::new(Document::new_scratch());
    state.set_workspace_folder(tmp1.path().to_path_buf());
    assert_eq!(state.file_tree.len(), 1);
    assert_eq!(state.file_tree[0].name, "a.md");

    // Switch to folder 2
    state.set_workspace_folder(tmp2.path().to_path_buf());
    assert_eq!(state.workspace_folder.as_ref(), Some(&tmp2.path().to_path_buf()));
    assert_eq!(state.file_tree.len(), 1);
    assert_eq!(state.file_tree[0].name, "b.md");
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

    // Start with empty scratch doc with no workspace folder
    let mut state = AppState::new(Document::new_scratch());
    assert!(state.workspace_folder.is_none());

    // Open single file
    state.open_file(f_single.clone()).unwrap();

    // Automatically falls back to parent directory as workspace folder
    assert_eq!(state.workspace_folder.as_ref(), Some(&nested));
    let rows = state.flatten_tree();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|r| r.name == "readme.md"));
    assert!(rows.iter().any(|r| r.name == "spec.md"));
}
