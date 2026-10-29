use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use glidemark_core::config::{Preferences, ThemeMode};
use glidemark_core::document::{Document, DocumentStatus};
use glidemark_core::export::export_html;
use glidemark_core::i18n::{get_strings, I18nStrings, SupportedLocale};

use crate::editor_engine::EditorEngine;
use crate::theme::ThemeColors;

/// Documents below this size parse 100ms after the last edit.
pub const PARSE_TIER_FAST_BYTES: usize = 2 * 1024 * 1024;
/// Documents up to this size parse 300ms after the last edit; larger ones pause parsing.
pub const PARSE_TIER_SLOW_BYTES: usize = 10 * 1024 * 1024;

/// Returns the debounce delay for a document size, or `None` when auto-parsing is paused.
pub fn parse_debounce(bytes: usize) -> Option<Duration> {
    if bytes < PARSE_TIER_FAST_BYTES {
        Some(Duration::from_millis(100))
    } else if bytes <= PARSE_TIER_SLOW_BYTES {
        Some(Duration::from_millis(300))
    } else {
        None
    }
}

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
    pub open_docs: Vec<Document>,
    pub active_doc_idx: usize,
    pub workspace_folder: Option<PathBuf>,
    pub editor: EditorEngine,
    pub view_mode: ViewMode,
    pub drawer_open: bool,
    pub drawer_tab: DrawerTab,
    pub settings_tab: SettingsTab,
    pub preferences: Preferences,
    pub theme: ThemeColors,
    pub strings: &'static I18nStrings,
    pub sibling_files: Vec<PathBuf>,
    pub is_system_dark: bool,
}

impl AppState {
    pub fn new(doc: Document) -> Self {
        let mut preferences = Preferences::load_or_default();
        if !crate::theme::ALL_PALETTES.iter().any(|p| p.id == preferences.palette) {
            preferences.palette = "default".to_string();
            let _ = preferences.save();
        }
        let is_dark = match preferences.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::Auto => false,
        };
        let theme = ThemeColors::resolve(&preferences.palette, is_dark);
        let strings = get_strings(preferences.locale);
        let editor = EditorEngine::new(&doc.buffer);

        let mut state = Self {
            open_docs: vec![doc.clone()],
            active_doc_idx: 0,
            workspace_folder: None,
            doc,
            editor,
            view_mode: ViewMode::Split,
            drawer_open: false,
            drawer_tab: DrawerTab::Files,
            settings_tab: SettingsTab::Appearance,
            preferences,
            theme,
            strings,
            sibling_files: Vec::new(),
            is_system_dark: false,
        };
        state.refresh_sibling_files();
        state
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }

    #[allow(dead_code)]
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
        self.is_system_dark = is_dark;
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
            ThemeMode::Dark => true,
            ThemeMode::Auto => self.is_system_dark,
        };
        self.theme = ThemeColors::resolve(&self.preferences.palette, is_dark);
    }

    /// Mirrors the editor buffer into the document. Returns whether the text changed.
    pub fn sync_from_editor(&mut self) -> bool {
        let new_text = self.editor.to_text();
        self.doc.set_buffer(new_text)
    }

    /// Parses the buffer now, ignoring the debounce tier.
    pub fn parse_now(&mut self) -> bool {
        self.doc.reparse()
    }

    pub fn parse_is_stale(&self) -> bool {
        self.doc.is_parse_stale()
    }

    pub fn refresh_sibling_files(&mut self) {
        self.sibling_files.clear();
        let target_dir = if let Some(ref folder) = self.workspace_folder {
            Some(folder.clone())
        } else if let Some(ref path) = self.doc.path {
            path.parent().map(|p| p.to_path_buf())
        } else {
            None
        };

        if let Some(dir) = target_dir {
            let mut scanned = 0;
            scan_markdown_files(&dir, &mut self.sibling_files, 0, &mut scanned);
        }
        self.sibling_files.sort();
    }

    pub fn set_workspace_folder(&mut self, folder: PathBuf) {
        let dir = if folder.is_file() {
            folder.parent().map(|p| p.to_path_buf()).unwrap_or(folder)
        } else {
            folder
        };
        self.workspace_folder = Some(dir);
        self.refresh_sibling_files();
    }

    pub fn open_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        self.sync_from_editor();
        if self.active_doc_idx < self.open_docs.len() {
            self.open_docs[self.active_doc_idx] = self.doc.clone();
        }

        // Check if already open
        if let Some(pos) = self.open_docs.iter().position(|d| d.path.as_ref() == Some(&path)) {
            self.switch_tab(pos);
            return Ok(());
        }

        let new_doc = Document::from_file(path)?;
        // If single empty scratch document, replace it
        if self.open_docs.len() == 1
            && self.open_docs[0].path.is_none()
            && self.open_docs[0].buffer.trim().is_empty()
        {
            self.open_docs[0] = new_doc.clone();
            self.active_doc_idx = 0;
        } else {
            self.open_docs.push(new_doc.clone());
            self.active_doc_idx = self.open_docs.len() - 1;
        }

        self.doc = new_doc;
        self.editor.reset_text(&self.doc.buffer);
        self.refresh_sibling_files();
        Ok(())
    }

    pub fn switch_tab(&mut self, idx: usize) {
        if idx >= self.open_docs.len() || idx == self.active_doc_idx {
            return;
        }
        self.sync_from_editor();
        if self.active_doc_idx < self.open_docs.len() {
            self.open_docs[self.active_doc_idx] = self.doc.clone();
        }

        self.active_doc_idx = idx;
        self.doc = self.open_docs[idx].clone();
        self.editor.reset_text(&self.doc.buffer);
        self.refresh_sibling_files();
    }

    pub fn close_tab(&mut self, idx: usize) {
        if idx >= self.open_docs.len() {
            return;
        }

        if self.open_docs.len() == 1 {
            let scratch = Document::new_scratch();
            self.open_docs[0] = scratch.clone();
            self.doc = scratch;
            self.active_doc_idx = 0;
            self.editor.reset_text("");
            self.refresh_sibling_files();
            return;
        }

        self.open_docs.remove(idx);
        if self.active_doc_idx >= self.open_docs.len() {
            self.active_doc_idx = self.open_docs.len() - 1;
        } else if self.active_doc_idx > idx {
            self.active_doc_idx -= 1;
        }

        self.doc = self.open_docs[self.active_doc_idx].clone();
        self.editor.reset_text(&self.doc.buffer);
        self.refresh_sibling_files();
    }

    pub fn new_tab(&mut self) {
        self.sync_from_editor();
        if self.active_doc_idx < self.open_docs.len() {
            self.open_docs[self.active_doc_idx] = self.doc.clone();
        }

        let scratch = Document::new_scratch();
        self.open_docs.push(scratch.clone());
        self.active_doc_idx = self.open_docs.len() - 1;
        self.doc = scratch;
        self.editor.reset_text("");
        self.refresh_sibling_files();
    }

    pub fn load_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        self.open_file(path)
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
                    if self.active_doc_idx < self.open_docs.len() {
                        self.open_docs[self.active_doc_idx] = self.doc.clone();
                    }
                    self.refresh_sibling_files();
                }
            }
        } else {
            let target_path = std::env::current_dir()?.join(&clean_name);
            self.doc.path = Some(target_path);
            self.save()?;
            if self.active_doc_idx < self.open_docs.len() {
                self.open_docs[self.active_doc_idx] = self.doc.clone();
            }
            self.refresh_sibling_files();
        }
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        self.sync_from_editor();
        self.doc.save()?;
        if self.active_doc_idx < self.open_docs.len() {
            self.open_docs[self.active_doc_idx] = self.doc.clone();
        }
        Ok(())
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

fn scan_markdown_files(dir: &std::path::Path, list: &mut Vec<PathBuf>, depth: usize, scanned: &mut usize) {
    if depth > 2 || list.len() >= 100 || *scanned >= 1000 {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            *scanned += 1;
            if list.len() >= 100 || *scanned >= 1000 {
                break;
            }
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let p = entry.path();
            let name = p.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
            if name.starts_with('.')
                || name == "target"
                || name == "node_modules"
                || name == "build"
                || name == "dist"
                || name == "dev"
                || name == "proc"
                || name == "sys"
            {
                continue;
            }
            if file_type.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
                        list.push(p);
                    }
                }
            } else if file_type.is_dir() {
                scan_markdown_files(&p, list, depth + 1, scanned);
            }
        }
    }
}
