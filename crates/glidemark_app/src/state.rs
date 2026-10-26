use std::fs;
use std::path::PathBuf;
use glidemark_core::anchor::SyncScrollMap;
use glidemark_core::config::{Preferences, ThemeMode};
use glidemark_core::document::{Document, DocumentStatus};
use glidemark_core::export::export_html;
use glidemark_core::i18n::{get_strings, I18nStrings, SupportedLocale};

use crate::editor_engine::EditorEngine;
use crate::theme::ThemeColors;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    Source,
    #[default]
    Split,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrawerTab {
    #[default]
    Files,
    Outline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Appearance,
    EditorSave,
    Language,
    About,
}

pub struct AppState {
    pub doc: Document,
    pub editor: EditorEngine,
    pub view_mode: ViewMode,
    pub drawer_open: bool,
    pub drawer_tab: DrawerTab,
    pub settings_open: bool,
    pub settings_tab: SettingsTab,
    pub preferences: Preferences,
    pub theme: ThemeColors,
    pub strings: &'static I18nStrings,
    pub sync_map: SyncScrollMap,
    pub sibling_files: Vec<PathBuf>,
}

impl AppState {
    pub fn new(doc: Document) -> Self {
        let preferences = Preferences::load_or_default();
        let is_dark = match preferences.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark | ThemeMode::Auto => true,
        };
        let theme = ThemeColors::resolve(&preferences.palette, is_dark);
        let strings = get_strings(preferences.locale);
        let sync_map = SyncScrollMap::new(doc.parsed.anchors.clone());
        let editor = EditorEngine::new(&doc.buffer);

        let mut state = Self {
            doc,
            editor,
            view_mode: ViewMode::Split,
            drawer_open: false,
            drawer_tab: DrawerTab::Files,
            settings_open: false,
            settings_tab: SettingsTab::Appearance,
            preferences,
            theme,
            strings,
            sync_map,
            sibling_files: Vec::new(),
        };
        state.refresh_sibling_files();
        state
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }

    pub fn toggle_drawer(&mut self) {
        self.drawer_open = !self.drawer_open;
        if self.drawer_open {
            self.refresh_sibling_files();
        }
    }

    pub fn set_drawer_tab(&mut self, tab: DrawerTab) {
        self.drawer_tab = tab;
    }

    #[allow(dead_code)]
    pub fn toggle_settings(&mut self) {
        self.settings_open = !self.settings_open;
    }

    pub fn set_settings_tab(&mut self, tab: SettingsTab) {
        self.settings_tab = tab;
    }

    pub fn set_auto_save(&mut self, enabled: bool) {
        self.preferences.auto_save = enabled;
        let _ = self.preferences.save();
    }

    pub fn set_auto_save_on_blur(&mut self, enabled: bool) {
        self.preferences.auto_save_on_blur = enabled;
        let _ = self.preferences.save();
    }

    pub fn set_tab_size(&mut self, tab_size: usize) {
        self.preferences.tab_size = tab_size;
        let _ = self.preferences.save();
    }

    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        self.preferences.theme = mode;
        let _ = self.preferences.save();
        self.recompute_theme();
    }

    pub fn set_palette(&mut self, palette_id: &str) {
        self.preferences.palette = palette_id.to_string();
        let _ = self.preferences.save();
        self.recompute_theme();
    }

    pub fn set_locale(&mut self, locale: SupportedLocale) {
        self.preferences.locale = locale;
        let _ = self.preferences.save();
        self.strings = get_strings(locale);
    }

    pub fn update_appearance(&mut self, is_dark: bool) {
        let effective_dark = match self.preferences.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::Auto => is_dark,
        };
        if self.theme.is_dark != effective_dark {
            self.theme = ThemeColors::resolve(&self.preferences.palette, effective_dark);
        }
    }

    pub fn recompute_theme(&mut self) {
        let is_dark = match self.preferences.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark | ThemeMode::Auto => true,
        };
        self.theme = ThemeColors::resolve(&self.preferences.palette, is_dark);
    }

    pub fn sync_from_editor(&mut self) {
        let new_text = self.editor.to_text();
        self.doc.update_buffer(new_text);
        self.sync_map = SyncScrollMap::new(self.doc.parsed.anchors.clone());
    }

    #[allow(dead_code)]
    pub fn update_content(&mut self, new_text: String) {
        self.editor.reset_text(&new_text);
        self.doc.update_buffer(new_text);
        self.sync_map = SyncScrollMap::new(self.doc.parsed.anchors.clone());
    }

    pub fn refresh_sibling_files(&mut self) {
        self.sibling_files.clear();
        if let Some(ref path) = self.doc.path {
            if let Some(parent) = path.parent() {
                if let Ok(entries) = fs::read_dir(parent) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            if let Some(ext) = p.extension() {
                                if ext == "md" || ext == "markdown" {
                                    self.sibling_files.push(p);
                                }
                            }
                        }
                    }
                }
            }
        }
        self.sibling_files.sort();
    }

    pub fn load_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        let doc = Document::from_file(path)?;
        self.editor.reset_text(&doc.buffer);
        self.doc = doc;
        self.sync_map = SyncScrollMap::new(self.doc.parsed.anchors.clone());
        self.refresh_sibling_files();
        Ok(())
    }

    pub fn rename_file(&mut self, new_name: &str) -> Result<(), std::io::Error> {
        let clean_name = if new_name.ends_with(".md") || new_name.ends_with(".markdown") {
            new_name.to_string()
        } else {
            format!("{}.md", new_name)
        };

        if let Some(old_path) = self.doc.path.clone() {
            if let Some(parent) = old_path.parent() {
                let new_path = parent.join(&clean_name);
                if new_path != old_path {
                    std::fs::rename(&old_path, &new_path)?;
                    self.doc.path = Some(new_path);
                    self.refresh_sibling_files();
                }
            }
        } else {
            let target_path = std::env::current_dir()?.join(&clean_name);
            self.doc.path = Some(target_path);
            self.save()?;
            self.refresh_sibling_files();
        }
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        self.sync_from_editor();
        self.doc.save()
    }

    pub fn export_standalone_html(&self) -> String {
        let title = self
            .doc
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(self.strings.untitled);
        export_html(&self.doc.parsed, title)
    }

    pub fn is_dirty(&self) -> bool {
        self.doc.status == DocumentStatus::Modified
    }

    pub fn has_conflict(&self) -> bool {
        self.doc.status == DocumentStatus::ConflictOnDisk
    }
}
