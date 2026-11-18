use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::i18n::SupportedLocale;

/// Current preference schema version.
pub const CURRENT_CONFIG_VERSION: u32 = 1;

/// Pre-rename config directory. Kept only so existing preferences migrate once.
pub const LEGACY_APP_DIR: &str = "GlideMark";

/// Palette identifier reserved for user-customized theme colors.
pub const CUSTOM_PALETTE_ID: &str = "custom";

/// Normalizes a 6-digit RGB hex color (`#RRGGBB` or `RRGGBB`) into lowercase `#rrggbb`.
pub fn normalize_hex_rgb(input: &str) -> Option<String> {
    let hex = input.trim().strip_prefix('#').unwrap_or_else(|| input.trim());
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("#{}", hex.to_ascii_lowercase()))
}

/// Identifies a customizable color slot inside a custom theme variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CustomColorSlot {
    Bg,
    Surface,
    Border,
    TextPrimary,
    TextMuted,
    Accent,
    CodeBg,
    SyntaxHeading,
    SyntaxEmphasis,
    SyntaxInlineCode,
    SyntaxLink,
    SyntaxBullet,
    SyntaxCode,
    SyntaxKeyword,
    SyntaxFunction,
    SyntaxAttribute,
    SyntaxString,
    SyntaxNumber,
    SyntaxComment,
}

impl CustomColorSlot {
    pub const CORE: [Self; 7] = [
        Self::Bg,
        Self::Surface,
        Self::Border,
        Self::TextPrimary,
        Self::TextMuted,
        Self::Accent,
        Self::CodeBg,
    ];

    pub const SYNTAX: [Self; 12] = [
        Self::SyntaxHeading,
        Self::SyntaxEmphasis,
        Self::SyntaxInlineCode,
        Self::SyntaxLink,
        Self::SyntaxBullet,
        Self::SyntaxCode,
        Self::SyntaxKeyword,
        Self::SyntaxFunction,
        Self::SyntaxAttribute,
        Self::SyntaxString,
        Self::SyntaxNumber,
        Self::SyntaxComment,
    ];

    pub const ALL: [Self; 19] = [
        Self::Bg,
        Self::Surface,
        Self::Border,
        Self::TextPrimary,
        Self::TextMuted,
        Self::Accent,
        Self::CodeBg,
        Self::SyntaxHeading,
        Self::SyntaxEmphasis,
        Self::SyntaxInlineCode,
        Self::SyntaxLink,
        Self::SyntaxBullet,
        Self::SyntaxCode,
        Self::SyntaxKeyword,
        Self::SyntaxFunction,
        Self::SyntaxAttribute,
        Self::SyntaxString,
        Self::SyntaxNumber,
        Self::SyntaxComment,
    ];
}

/// Customizable hex color overrides (`#rrggbb`) for one light or dark variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CustomThemePalette {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_primary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_muted: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_bg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_heading: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_emphasis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_inline_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_bullet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_keyword: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_function: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_attribute: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_string: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_comment: Option<String>,
}

impl CustomThemePalette {
    pub fn slot_ref(&self, slot: CustomColorSlot) -> &Option<String> {
        match slot {
            CustomColorSlot::Bg => &self.bg,
            CustomColorSlot::Surface => &self.surface,
            CustomColorSlot::Border => &self.border,
            CustomColorSlot::TextPrimary => &self.text_primary,
            CustomColorSlot::TextMuted => &self.text_muted,
            CustomColorSlot::Accent => &self.accent,
            CustomColorSlot::CodeBg => &self.code_bg,
            CustomColorSlot::SyntaxHeading => &self.syntax_heading,
            CustomColorSlot::SyntaxEmphasis => &self.syntax_emphasis,
            CustomColorSlot::SyntaxInlineCode => &self.syntax_inline_code,
            CustomColorSlot::SyntaxLink => &self.syntax_link,
            CustomColorSlot::SyntaxBullet => &self.syntax_bullet,
            CustomColorSlot::SyntaxCode => &self.syntax_code,
            CustomColorSlot::SyntaxKeyword => &self.syntax_keyword,
            CustomColorSlot::SyntaxFunction => &self.syntax_function,
            CustomColorSlot::SyntaxAttribute => &self.syntax_attribute,
            CustomColorSlot::SyntaxString => &self.syntax_string,
            CustomColorSlot::SyntaxNumber => &self.syntax_number,
            CustomColorSlot::SyntaxComment => &self.syntax_comment,
        }
    }

    pub fn slot_mut(&mut self, slot: CustomColorSlot) -> &mut Option<String> {
        match slot {
            CustomColorSlot::Bg => &mut self.bg,
            CustomColorSlot::Surface => &mut self.surface,
            CustomColorSlot::Border => &mut self.border,
            CustomColorSlot::TextPrimary => &mut self.text_primary,
            CustomColorSlot::TextMuted => &mut self.text_muted,
            CustomColorSlot::Accent => &mut self.accent,
            CustomColorSlot::CodeBg => &mut self.code_bg,
            CustomColorSlot::SyntaxHeading => &mut self.syntax_heading,
            CustomColorSlot::SyntaxEmphasis => &mut self.syntax_emphasis,
            CustomColorSlot::SyntaxInlineCode => &mut self.syntax_inline_code,
            CustomColorSlot::SyntaxLink => &mut self.syntax_link,
            CustomColorSlot::SyntaxBullet => &mut self.syntax_bullet,
            CustomColorSlot::SyntaxCode => &mut self.syntax_code,
            CustomColorSlot::SyntaxKeyword => &mut self.syntax_keyword,
            CustomColorSlot::SyntaxFunction => &mut self.syntax_function,
            CustomColorSlot::SyntaxAttribute => &mut self.syntax_attribute,
            CustomColorSlot::SyntaxString => &mut self.syntax_string,
            CustomColorSlot::SyntaxNumber => &mut self.syntax_number,
            CustomColorSlot::SyntaxComment => &mut self.syntax_comment,
        }
    }

    pub fn get(&self, slot: CustomColorSlot) -> Option<&str> {
        self.slot_ref(slot).as_deref()
    }

    pub fn set(&mut self, slot: CustomColorSlot, value: Option<String>) {
        *self.slot_mut(slot) = value.and_then(|s| normalize_hex_rgb(&s));
    }

    pub fn is_empty(&self) -> bool {
        CustomColorSlot::ALL.iter().all(|&s| self.slot_ref(s).is_none())
    }
}

/// Separate light and dark custom theme color sets stored in `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CustomThemePreferences {
    #[serde(default, skip_serializing_if = "CustomThemePalette::is_empty")]
    pub light: CustomThemePalette,
    #[serde(default, skip_serializing_if = "CustomThemePalette::is_empty")]
    pub dark: CustomThemePalette,
}

impl CustomThemePreferences {
    pub fn variant(&self, is_dark: bool) -> &CustomThemePalette {
        if is_dark { &self.dark } else { &self.light }
    }

    pub fn variant_mut(&mut self, is_dark: bool) -> &mut CustomThemePalette {
        if is_dark { &mut self.dark } else { &mut self.light }
    }

    pub fn is_empty(&self) -> bool {
        self.light.is_empty() && self.dark.is_empty()
    }
}

fn legacy_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(LEGACY_APP_DIR)
}

/// Copy `old_root/config.toml` into `new_root` when the new config does not
/// exist yet. Never overwrites and never deletes the legacy file.
pub fn migrate_config_if_needed(old_root: &Path, new_root: &Path) -> bool {
    let new_path = new_root.join("config.toml");
    if new_path.exists() {
        return false;
    }
    let old_path = old_root.join("config.toml");
    if !old_path.exists() {
        return false;
    }
    if let Some(parent) = new_path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    fs::copy(&old_path, &new_path).is_ok()
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Auto,
    Dark,
    Light,
}

fn default_true() -> bool {
    true
}

fn default_auto_save_delay() -> u64 {
    1000
}

fn default_line_height() -> f32 {
    1.6
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    pub font_family: String,
    pub font_size: f32,
    #[serde(default = "default_line_height")]
    pub line_height: f32,
    pub max_column_width: f32,
    pub theme: ThemeMode,
    pub palette: String,
    pub locale: SupportedLocale,
    pub tab_size: usize,
    #[serde(default = "default_true")]
    pub auto_save: bool,
    #[serde(default = "default_auto_save_delay")]
    pub auto_save_delay_ms: u64,
    #[serde(default = "default_true")]
    pub auto_save_on_blur: bool,
    /// Skip interface transitions, pulses and other decorative motion.
    #[serde(default)]
    pub reduce_motion: bool,
    /// Show the editor gutter line numbers.
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,
    /// One-time preference migrations (0 = pre-0.2.1 config).
    #[serde(default)]
    pub config_version: u32,
    #[serde(default)]
    pub workspace_roots: Vec<PathBuf>,
    #[serde(default)]
    pub sidebar_hidden_paths: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "CustomThemePreferences::is_empty")]
    pub custom_theme: CustomThemePreferences,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            font_family: "system-ui".to_string(),
            font_size: 16.0,
            line_height: 1.6,
            max_column_width: 760.0,
            theme: ThemeMode::Auto,
            palette: "default".to_string(),
            locale: SupportedLocale::Auto,
            tab_size: 4,
            auto_save: true,
            auto_save_delay_ms: 1000,
            auto_save_on_blur: true,
            reduce_motion: false,
            show_line_numbers: true,
            config_version: CURRENT_CONFIG_VERSION,
            workspace_roots: Vec::new(),
            sidebar_hidden_paths: Vec::new(),
            custom_theme: CustomThemePreferences::default(),
        }
    }
}

impl Preferences {
    pub fn sanitize_line_height(val: f32) -> f32 {
        if !(1.3..=1.7).contains(&val) || val.is_nan() {
            1.6
        } else {
            val
        }
    }

    /// Standard OS application support or config directory.
    pub fn default_config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ZMark")
    }

    pub fn default_config_path() -> PathBuf {
        Self::default_config_dir().join("config.toml")
    }

    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        let mut pref: Preferences = toml::from_str(&content)?;
        pref.line_height = Self::sanitize_line_height(pref.line_height);
        if pref.migrate_defaults() {
            // Best-effort: a read-only config directory must not block startup.
            if let Err(error) = pref.save_to_path(path) {
                eprintln!("zmark: could not persist config migration: {error}");
            }
        }
        Ok(pref)
    }

    /// Applies one-time defaults. `config_version` is bumped for every old
    /// config, so a later user choice of 820 is never rewritten again.
    fn migrate_defaults(&mut self) -> bool {
        if self.config_version >= CURRENT_CONFIG_VERSION {
            return false;
        }
        self.config_version = CURRENT_CONFIG_VERSION;
        if (self.max_column_width - 820.0).abs() < 0.01 {
            self.max_column_width = 760.0;
        }
        true
    }

    pub fn load_or_default_from_path(path: &Path) -> Self {
        if path.exists() {
            Self::load_from_path(path).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn load_or_default() -> Self {
        let new_dir = Self::default_config_dir();
        migrate_config_if_needed(&legacy_config_dir(), &new_dir);
        Self::load_or_default_from_path(&new_dir.join("config.toml"))
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;

        let parent_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("config");
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("toml");
        let pid = std::process::id();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_path = parent_dir.join(format!("{}.tmp.{}_{}.{}", file_stem, pid, nanos, ext));

        // Never overwrite another writer's temporary file or unlink the old config first.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;
        if let Err(err) = file
            .write_all(serialized.as_bytes())
            .and_then(|_| file.sync_all())
        {
            drop(file);
            let _ = fs::remove_file(&tmp_path);
            return Err(ConfigError::Io(err));
        }
        drop(file);

        // std::fs::rename replaces existing files on Windows too (MoveFileExW).
        if let Err(err) = fs::rename(&tmp_path, path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(ConfigError::Io(err));
        }

        Ok(())
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        self.save_to_path(&Self::default_config_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_config_preserves_old_data_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut preferences = Preferences::default();
        preferences.save_to_path(&path).unwrap();
        preferences.tab_size = 8;
        preferences.save_to_path(&path).unwrap();
        assert_eq!(Preferences::load_from_path(&path).unwrap().tab_size, 8);
        let blocked = dir.path().join("directory.toml");
        fs::create_dir(&blocked).unwrap();
        fs::write(blocked.join("keep"), "original").unwrap();
        assert!(preferences.save_to_path(&blocked).is_err());
        assert_eq!(
            fs::read_to_string(blocked.join("keep")).unwrap(),
            "original"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_shared_destination_is_never_unlinked_on_replacement_failure() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut preferences = Preferences::default();
        preferences.save_to_path(&path).unwrap();
        let original = fs::read(&path).unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(4)
            .open(&path)
            .unwrap();
        preferences.tab_size = 8;
        let result = preferences.save_to_path(&path);
        drop(lock);
        assert!(path.exists());
        if result.is_err() {
            assert_eq!(fs::read(&path).unwrap(), original);
        } else {
            assert_eq!(Preferences::load_from_path(&path).unwrap().tab_size, 8);
        }
    }

    #[test]
    fn test_sanitize_line_height() {
        for &valid in &[1.3f32, 1.4f32, 1.5f32, 1.6f32, 1.7f32] {
            assert_eq!(Preferences::sanitize_line_height(valid), valid);
        }
        for &underflow in &[0.5f32, 1.0f32, 1.29f32, 0.0f32, -1.0f32, -100.0f32] {
            assert_eq!(Preferences::sanitize_line_height(underflow), 1.6);
        }
        for &overflow in &[1.71f32, 2.0f32, 3.0f32, 10.0f32, 99.0f32] {
            assert_eq!(Preferences::sanitize_line_height(overflow), 1.6);
        }
        assert_eq!(Preferences::sanitize_line_height(f32::NAN), 1.6);
        assert_eq!(Preferences::sanitize_line_height(f32::INFINITY), 1.6);
        assert_eq!(Preferences::sanitize_line_height(f32::NEG_INFINITY), 1.6);
    }

    #[test]
    fn max_width_migrates_once_and_line_numbers_default_on() {
        let root = std::env::temp_dir().join(format!("zmark_cfg_width_{}", std::process::id()));
        let path = root.join("config.toml");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        // Old 820 config without a schema version -> 760 and version 1.
        let mut old = Preferences::default();
        old.max_column_width = 820.0;
        old.config_version = 0;
        fs::write(&path, toml::to_string(&old).unwrap()).unwrap();
        let loaded = Preferences::load_from_path(&path).unwrap();
        assert_eq!(loaded.max_column_width, 760.0);
        assert_eq!(loaded.config_version, CURRENT_CONFIG_VERSION);
        assert!(
            loaded.show_line_numbers,
            "old configs default to visible line numbers"
        );

        // A second load is stable.
        assert_eq!(
            Preferences::load_from_path(&path).unwrap().max_column_width,
            760.0
        );

        // A user choice of 820 on the current schema is preserved.
        let mut user = loaded;
        user.max_column_width = 820.0;
        fs::write(&path, toml::to_string(&user).unwrap()).unwrap();
        assert_eq!(
            Preferences::load_from_path(&path).unwrap().max_column_width,
            820.0
        );

        // An old non-820 value is kept but still marked migrated, so a later
        // 820 choice is not rewritten.
        let mut old750 = Preferences::default();
        old750.max_column_width = 750.0;
        old750.config_version = 0;
        fs::write(&path, toml::to_string(&old750).unwrap()).unwrap();
        let loaded = Preferences::load_from_path(&path).unwrap();
        assert_eq!(loaded.max_column_width, 750.0);
        assert_eq!(loaded.config_version, CURRENT_CONFIG_VERSION);

        // Missing config_version behaves like version 0.
        let mut missing = toml::to_string(&old750).unwrap();
        missing = missing
            .lines()
            .filter(|line| !line.starts_with("config_version"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, missing).unwrap();
        assert_eq!(
            Preferences::load_from_path(&path).unwrap().config_version,
            CURRENT_CONFIG_VERSION
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn reduce_motion_defaults_off_and_round_trips() {
        let default = Preferences::default();
        assert!(!default.reduce_motion);
        let serialized = toml::to_string(&default).unwrap();
        let parsed: Preferences = toml::from_str(&serialized).unwrap();
        assert!(!parsed.reduce_motion);

        let mut enabled = default;
        enabled.reduce_motion = true;
        let parsed: Preferences = toml::from_str(&toml::to_string(&enabled).unwrap()).unwrap();
        assert!(parsed.reduce_motion);
    }

    #[test]
    fn config_migrates_from_glidemark_once() {
        let root = std::env::temp_dir().join(format!("zmark_cfg_migrate_{}", std::process::id()));
        let old = root.join("GlideMark");
        let new = root.join("ZMark");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("config.toml"), "font_size = 18.0\n").unwrap();

        assert!(migrate_config_if_needed(&old, &new));
        let migrated = fs::read_to_string(new.join("config.toml")).unwrap();
        assert!(migrated.contains("font_size = 18.0"));
        assert!(
            old.join("config.toml").exists(),
            "legacy config must not be deleted"
        );

        // A second call must not overwrite the migrated copy.
        fs::write(new.join("config.toml"), "font_size = 20.0\n").unwrap();
        assert!(!migrate_config_if_needed(&old, &new));
        assert!(fs::read_to_string(new.join("config.toml"))
            .unwrap()
            .contains("font_size = 20.0"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn config_migration_noop_without_legacy_file() {
        let root = std::env::temp_dir().join(format!("zmark_cfg_noop_{}", std::process::id()));
        let old = root.join("GlideMark");
        let new = root.join("ZMark");
        let _ = fs::remove_dir_all(&root);
        assert!(!migrate_config_if_needed(&old, &new));
        assert!(!new.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn custom_theme_normalizes_hex_and_round_trips() {
        assert_eq!(normalize_hex_rgb(" #AbCdEf "), Some("#abcdef".to_string()));
        assert_eq!(normalize_hex_rgb("123456"), Some("#123456".to_string()));
        assert_eq!(normalize_hex_rgb("#12345"), None);
        assert_eq!(normalize_hex_rgb("zzzzzz"), None);

        let mut prefs = Preferences::default();
        assert!(!toml::to_string(&prefs).unwrap().contains("custom_theme"));

        prefs.palette = CUSTOM_PALETTE_ID.to_string();
        prefs
            .custom_theme
            .variant_mut(false)
            .set(CustomColorSlot::Accent, Some("FF8800".to_string()));
        prefs
            .custom_theme
            .variant_mut(true)
            .set(CustomColorSlot::Bg, Some("invalid".to_string()));

        let parsed: Preferences = toml::from_str(&toml::to_string(&prefs).unwrap()).unwrap();
        assert_eq!(parsed.palette, CUSTOM_PALETTE_ID);
        assert_eq!(
            parsed.custom_theme.variant(false).get(CustomColorSlot::Accent),
            Some("#ff8800")
        );
        assert!(parsed.custom_theme.variant(true).is_empty());
    }
}
