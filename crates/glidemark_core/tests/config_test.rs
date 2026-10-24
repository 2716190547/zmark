use glidemark_core::config::{Preferences, ThemeMode};
use glidemark_core::i18n::SupportedLocale;
use tempfile::tempdir;

#[test]
fn test_default_preferences() {
    let pref = Preferences::default();
    assert_eq!(pref.font_size, 16.0);
    assert_eq!(pref.max_column_width, 820.0);
    assert_eq!(pref.theme, ThemeMode::Auto);
    assert_eq!(pref.palette, "miaoyan");
    assert_eq!(pref.locale, SupportedLocale::Auto);
    assert!(pref.auto_save_on_blur);
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

    custom.save_to_path(&config_path).expect("Save should succeed");
    assert!(config_path.exists());

    let loaded = Preferences::load_from_path(&config_path).expect("Load should succeed");
    assert_eq!(loaded.font_family, "JetBrains Mono");
    assert_eq!(loaded.font_size, 18.0);
    assert_eq!(loaded.theme, ThemeMode::Dark);
    assert_eq!(loaded.palette, "tokyo-night");
    assert_eq!(loaded.locale, SupportedLocale::ZhCn);
    assert_eq!(loaded.max_column_width, 750.0);
}

#[test]
fn test_load_nonexistent_returns_default() {
    let dir = tempdir().unwrap();
    let non_existent = dir.path().join("does_not_exist.toml");
    let pref = Preferences::load_or_default_from_path(&non_existent);
    assert_eq!(pref, Preferences::default());
}
