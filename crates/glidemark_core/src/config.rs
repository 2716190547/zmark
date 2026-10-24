use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::i18n::SupportedLocale;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub max_column_width: f32,
    pub theme: ThemeMode,
    pub palette: String,
    pub locale: SupportedLocale,
    pub tab_size: usize,
    pub auto_save_on_blur: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            font_family: "system-ui".to_string(),
            font_size: 16.0,
            line_height: 1.6,
            max_column_width: 820.0,
            theme: ThemeMode::Auto,
            palette: "miaoyan".to_string(),
            locale: SupportedLocale::Auto,
            tab_size: 4,
            auto_save_on_blur: true,
        }
    }
}

impl Preferences {
    /// Standard OS application support or config directory.
    pub fn default_config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("GlideMark")
    }

    pub fn default_config_path() -> PathBuf {
        Self::default_config_dir().join("config.toml")
    }

    pub fn load_from_path(path: &Path) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        let pref: Preferences = toml::from_str(&content)?;
        Ok(pref)
    }

    pub fn load_or_default_from_path(path: &Path) -> Self {
        if path.exists() {
            Self::load_from_path(path).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn load_or_default() -> Self {
        Self::load_or_default_from_path(&Self::default_config_path())
    }

    pub fn save_to_path(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;
        fs::write(path, serialized)?;
        Ok(())
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        self.save_to_path(&Self::default_config_path())
    }
}
