use std::path::PathBuf;
use glidemark_core::config::{Preferences, ThemeMode};
use glidemark_core::i18n::SupportedLocale;
use tempfile::tempdir;

#[test]
fn test_default_preferences() {
    let pref = Preferences::default();
    assert_eq!(pref.font_size, 16.0);
    assert_eq!(pref.max_column_width, 820.0);
    assert_eq!(pref.theme, ThemeMode::Auto);
    assert_eq!(pref.palette, "default");
    assert_eq!(pref.locale, SupportedLocale::Auto);
    assert!(pref.auto_save);
    assert_eq!(pref.auto_save_delay_ms, 1000);
    assert!(pref.auto_save_on_blur);
    assert!(pref.workspace_roots.is_empty());
}

#[test]
fn test_save_and_load_preferences() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");

    let mut custom = Preferences::default();
    custom.font_family = "JetBrains Mono".into();
    custom.font_size = 18.0;
    custom.theme = ThemeMode::Dark;
    custom.palette = "tokyo-night".into();
    custom.locale = SupportedLocale::ZhCn;
    custom.max_column_width = 750.0;
    custom.workspace_roots = vec![PathBuf::from("/test/path/a"), PathBuf::from("/test/path/b")];

    custom.save_to_path(&config_path).expect("Save should succeed");
    assert!(config_path.exists());

    let loaded = Preferences::load_from_path(&config_path).expect("Load should succeed");
    assert_eq!(loaded.font_family, "JetBrains Mono");
    assert_eq!(loaded.font_size, 18.0);
    assert_eq!(loaded.theme, ThemeMode::Dark);
    assert_eq!(loaded.palette, "tokyo-night");
    assert_eq!(loaded.locale, SupportedLocale::ZhCn);
    assert_eq!(loaded.max_column_width, 750.0);
    assert_eq!(
        loaded.workspace_roots,
        vec![PathBuf::from("/test/path/a"), PathBuf::from("/test/path/b")]
    );
}

#[test]
fn test_load_without_workspace_roots_defaults_to_empty() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("old_config.toml");
    std::fs::write(&config_path, "font_family = 'Monaco'\nfont_size = 14.0\nline_height = 1.5\nmax_column_width = 800.0\ntheme = 'dark'\npalette = 'default'\nlocale = 'auto'\ntab_size = 4\n").unwrap();
    let loaded = Preferences::load_from_path(&config_path).expect("Load should succeed");
    assert_eq!(loaded.font_family, "Monaco");
    assert!(loaded.workspace_roots.is_empty());
}

#[test]
fn test_load_without_line_height_defaults_to_1_6() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("old_config.toml");
    std::fs::write(
        &config_path,
        "font_family = 'Monaco'\nfont_size = 14.0\nmax_column_width = 800.0\ntheme = 'dark'\npalette = 'default'\nlocale = 'auto'\ntab_size = 4\n",
    )
    .unwrap();
    let loaded = Preferences::load_from_path(&config_path).expect("Load should succeed");
    assert!((loaded.line_height - 1.6).abs() < f32::EPSILON);
}

#[test]
fn test_toml_deserialize_without_line_height() {
    let toml_str = "font_family = 'Monaco'\nfont_size = 14.0\nmax_column_width = 800.0\ntheme = 'dark'\npalette = 'default'\nlocale = 'auto'\ntab_size = 4\n";
    let pref: Preferences = toml::from_str(toml_str).expect("should deserialize");
    assert!((pref.line_height - 1.6).abs() < f32::EPSILON);
}

#[test]
fn test_load_nonexistent_returns_default() {
    let dir = tempdir().unwrap();
    let non_existent = dir.path().join("does_not_exist.toml");
    let pref = Preferences::load_or_default_from_path(&non_existent);
    assert_eq!(pref, Preferences::default());
}

#[test]
fn test_save_to_path_atomic_replacement_and_failure_resilience() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");

    // 1. Initial save succeeds
    let mut pref1 = Preferences::default();
    pref1.font_family = "Initial Font".to_string();
    pref1.save_to_path(&config_path).expect("Initial save succeeds");
    assert!(config_path.exists());
    let content1 = std::fs::read_to_string(&config_path).unwrap();
    assert!(content1.contains("Initial Font"));

    // 2. Atomic replacement with new content succeeds
    let mut pref2 = pref1.clone();
    pref2.font_family = "Updated Font".to_string();
    pref2.save_to_path(&config_path).expect("Atomic update succeeds");
    let content2 = std::fs::read_to_string(&config_path).unwrap();
    assert!(content2.contains("Updated Font"));

    // 3. Failure resilience: target path cannot be overwritten (e.g. replaced by directory)
    let conflict_dir = dir.path().join("conflict_dir");
    std::fs::create_dir(&conflict_dir).unwrap();
    let original_in_conflict = conflict_dir.join("inner_config.toml");
    pref2.save_to_path(&original_in_conflict).expect("Save inside conflict_dir succeeds");
    let saved_inner = std::fs::read_to_string(&original_in_conflict).unwrap();

    // Now try to overwrite a path that is an existing directory: rename over directory fails
    let mut bad_pref = Preferences::default();
    bad_pref.font_family = "Corrupted Font".to_string();
    let res = bad_pref.save_to_path(&conflict_dir);
    assert!(res.is_err(), "Saving to a directory path must fail");

    // Verify original file inside conflict_dir is intact
    let inner_after = std::fs::read_to_string(&original_in_conflict).unwrap();
    assert_eq!(saved_inner, inner_after);

    // Verify no leftover .tmp files
    let tmp_files_count = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
        .count();
    assert_eq!(tmp_files_count, 0);
}
