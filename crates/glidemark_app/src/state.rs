use std::fs;
use std::path::PathBuf;
use glidemark_core::anchor::SyncScrollMap;
use glidemark_core::config::{Preferences, ThemeMode};
use glidemark_core::document::{Document, DocumentStatus};
use glidemark_core::export::export_html;

use crate::theme::ThemeColors;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    Source,
    #[default]
    Split,
    Preview,
}

pub struct AppState {
    pub doc: Document,
    pub view_mode: ViewMode,
    pub drawer_open: bool,
    pub preferences: Preferences,
    pub theme: ThemeColors,
    pub sync_map: SyncScrollMap,
    pub sibling_files: Vec<PathBuf>,
}

impl AppState {
    pub fn new(doc: Document) -> Self {
        let preferences = Preferences::load_or_default();
        let theme = match preferences.theme {
            ThemeMode::Light => ThemeColors::light(),
            ThemeMode::Dark | ThemeMode::Auto => ThemeColors::dark(),
        };
        let sync_map = SyncScrollMap::new(doc.parsed.anchors.clone());
        let mut state = Self {
            doc,
            view_mode: ViewMode::Split,
            drawer_open: false,
            preferences,
            theme,
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

    #[allow(dead_code)]
    pub fn update_content(&mut self, new_text: String) {
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
        self.doc = doc;
        self.sync_map = SyncScrollMap::new(self.doc.parsed.anchors.clone());
        self.refresh_sibling_files();
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        self.doc.save()
    }

    pub fn export_standalone_html(&self) -> String {
        let title = self
            .doc
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("GlideMark Document");
        export_html(&self.doc.parsed, title)
    }

    pub fn is_dirty(&self) -> bool {
        self.doc.status == DocumentStatus::Modified
    }

    pub fn has_conflict(&self) -> bool {
        self.doc.status == DocumentStatus::ConflictOnDisk
    }
}
