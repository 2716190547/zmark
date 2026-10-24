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
