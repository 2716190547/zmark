use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use zmark_core::config::{Preferences, ThemeMode};
use zmark_core::document::{Document, DocumentStatus};
use zmark_core::export::export_html;
use zmark_core::i18n::{get_strings, I18nStrings, SupportedLocale};

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
    Shortcuts,
    About,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DocKey {
    Path(PathBuf),
    Scratch(u64),
}

impl DocKey {
    pub fn is_scratch(&self) -> bool {
        matches!(self, DocKey::Scratch(_))
    }

    #[allow(dead_code)]
    pub fn as_path(&self) -> Option<&PathBuf> {
        match self {
            DocKey::Path(p) => Some(p),
            DocKey::Scratch(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenDoc {
    pub key: DocKey,
    pub doc: Document,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseDecision {
    CloseImmediately,
    PromptDirty {
        key: DocKey,
        title: String,
        has_path: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingClose {
    pub key: DocKey,
    pub title: String,
    pub has_path: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub children: Vec<FileTreeNode>,
    pub mtime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRoot {
    pub path: PathBuf,
    pub tree: Vec<FileTreeNode>,
    pub is_expanded: bool,
    pub is_missing: bool,
    pub truncated: bool,
    pub entries_used: usize,
    pub files_used: usize,
    pub deferred: bool,
}

impl Default for WorkspaceRoot {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            tree: Vec::new(),
            is_expanded: false,
            is_missing: false,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        }
    }
}

impl WorkspaceRoot {
    pub fn expanded_folders(&self) -> HashSet<PathBuf> {
        let mut set = HashSet::new();
        collect_expanded_paths(&self.tree, &mut set);
        set
    }

    pub fn set_expanded_folders(&mut self, set: &HashSet<PathBuf>) {
        apply_expanded_paths(&mut self.tree, set);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanBudget {
    pub entries_left: usize,
    pub files_left: usize,
}

impl Default for ScanBudget {
    fn default() -> Self {
        Self {
            entries_left: Self::MAX_GLOBAL_ENTRIES,
            files_left: Self::MAX_GLOBAL_FILES,
        }
    }
}

impl ScanBudget {
    pub const MAX_GLOBAL_ENTRIES: usize = 2000;
    pub const MAX_GLOBAL_FILES: usize = 400;
    pub const PER_ROOT_ENTRY_CAP: usize = 800;
    pub const PER_ROOT_FILE_CAP: usize = 150;
    pub const FIRST_LAYER_FLOOR_ENTRIES: usize = 100;
    pub const FIRST_LAYER_FLOOR_FILES: usize = 30;

    pub fn new(entries: usize, files: usize) -> Self {
        Self {
            entries_left: entries,
            files_left: files,
        }
    }

    #[allow(dead_code)]
    pub fn can_scan_entry(&self) -> bool {
        self.entries_left > 0
    }

    pub fn consume_entry(&mut self) -> bool {
        if self.entries_left > 0 {
            self.entries_left -= 1;
            true
        } else {
            false
        }
    }

    pub fn consume_file(&mut self) -> bool {
        if self.files_left > 0 {
            self.files_left -= 1;
            true
        } else {
            false
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootAddError {
    NotFound,
    NotADirectory,
    AlreadyExists,
    CanonicalizeFailed(PathBuf, String),
    SaveFailed(String),
    NestedInExisting(PathBuf),
    ContainsExisting(PathBuf),
}

impl std::fmt::Display for RootAddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RootAddError::NotFound => write!(f, "Directory not found"),
            RootAddError::NotADirectory => write!(f, "Path is not a directory"),
            RootAddError::AlreadyExists => write!(f, "Workspace root already exists"),
            RootAddError::CanonicalizeFailed(p, msg) => {
                write!(f, "Failed to canonicalize path {}: {}", p.display(), msg)
            }
            RootAddError::SaveFailed(msg) => write!(f, "Failed to save preferences: {}", msg),
            RootAddError::NestedInExisting(p) => {
                write!(f, "Directory is nested within existing root: {}", p.display())
            }
            RootAddError::ContainsExisting(p) => {
                write!(f, "Directory contains existing root: {}", p.display())
            }
        }
    }
}

impl std::error::Error for RootAddError {}

/// Helper to resolve a path for lexical nesting comparison.
/// Walks up to the nearest existing ancestor to canonicalize symlinks on platforms like macOS.
fn resolve_cmp_path(path: &std::path::Path) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(path) {
        return c;
    }
    let mut suffix = Vec::new();
    let mut curr = path;
    while let Some(parent) = curr.parent() {
        if let Some(name) = curr.file_name() {
            suffix.push(name);
        }
        if parent.exists() {
            if let Ok(mut base) = std::fs::canonicalize(parent) {
                while let Some(comp) = suffix.pop() {
                    base.push(comp);
                }
                return base;
            }
        }
        curr = parent;
    }
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedRoots {
    pub roots: Vec<WorkspaceRoot>,
    pub discarded: Vec<(PathBuf, String)>,
}

/// Normalizes a list of configured workspace roots:
/// - Deduplicates paths;
/// - Discards nested subpaths;
/// - Canonicalizes physically existing directories (is_missing = false, is_expanded = true);
/// - Marks nonexistent paths as is_missing = true, is_expanded = false.
#[allow(dead_code)]
pub fn normalize_loaded_roots(paths: &[PathBuf]) -> NormalizedRoots {
    let mut roots: Vec<WorkspaceRoot> = Vec::new();
    let mut discarded: Vec<(PathBuf, String)> = Vec::new();

    for raw_path in paths {
        let (path, is_missing) = if raw_path.exists() && raw_path.is_dir() {
            match std::fs::canonicalize(raw_path) {
                Ok(c) => (c, false),
                Err(_) => (raw_path.clone(), true),
            }
        } else {
            (raw_path.clone(), true)
        };

        let path_cmp = resolve_cmp_path(&path);

        // Deduplication
        if roots.iter().any(|r| {
            r.path == path || &r.path == raw_path || resolve_cmp_path(&r.path) == path_cmp
        }) {
            discarded.push((raw_path.clone(), "Duplicate path".to_string()));
            continue;
        }

        // Nesting check: discard nested subpaths
        let mut is_nested = false;
        let mut nested_reason = String::new();
        let mut i = 0;
        while i < roots.len() {
            let r_cmp = resolve_cmp_path(&roots[i].path);
            if path_cmp.starts_with(&r_cmp) {
                is_nested = true;
                nested_reason = format!("Nested within existing root: {}", roots[i].path.display());
                break;
            } else if r_cmp.starts_with(&path_cmp) {
                let removed = roots.remove(i);
                discarded.push((
                    removed.path.clone(),
                    format!("Nested within new parent root: {}", path.display()),
                ));
            } else {
                i += 1;
            }
        }

        if is_nested {
            discarded.push((raw_path.clone(), nested_reason));
            continue;
        }

        roots.push(WorkspaceRoot {
            path,
            tree: Vec::new(),
            is_expanded: !is_missing,
            is_missing,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        });
    }

    NormalizedRoots { roots, discarded }
}

/// Derives a disambiguated display label for a workspace root.
/// Returns the folder name if unique among valid roots;
/// traces parent directory name if duplicated (`docs — projA`);
/// falls back to canonicalized full path if parent is also duplicated or absent.
#[allow(dead_code)]
pub fn root_label(roots: &[WorkspaceRoot], idx: usize) -> String {
    if idx >= roots.len() {
        return String::new();
    }
    let target = &roots[idx];
    let folder_name = match target.path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.to_string(),
        None => target.path.to_string_lossy().to_string(),
    };

    let has_duplicate = if target.is_missing {
        roots.iter().enumerate().any(|(i, r)| {
            i != idx && r.path.file_name().and_then(|n| n.to_str()).unwrap_or("/") == folder_name
        })
    } else {
        roots.iter().enumerate().any(|(i, r)| {
            i != idx && !r.is_missing && r.path.file_name().and_then(|n| n.to_str()).unwrap_or("/") == folder_name
        })
    };

    if !has_duplicate {
        return folder_name;
    }

    if let Some(parent) = target.path.parent() {
        if let Some(parent_name) = parent.file_name().and_then(|n| n.to_str()) {
            let candidate = format!("{} \u{2014} {}", folder_name, parent_name);
            let candidate_duplicate = roots.iter().enumerate().any(|(i, r)| {
                if i == idx {
                    return false;
                }
                if !target.is_missing && r.is_missing {
                    return false;
                }
                let r_folder = r.path.file_name().and_then(|n| n.to_str()).unwrap_or("/");
                if r_folder != folder_name {
                    return false;
                }
                let r_parent = r
                    .path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                r_parent == parent_name
            });

            if !candidate_duplicate {
                return candidate;
            }
        }
    }

    target.path.to_string_lossy().to_string()
}

pub struct AppState {
    empty_doc: Document,
    _isolated_preferences_dir: Option<tempfile::TempDir>,
    pub(crate) open_docs: Vec<OpenDoc>,
    pub(crate) active: Option<DocKey>,
    pub next_scratch_id: u64,
    pub pending_close: Option<PendingClose>,
    pub pending_remove_root: Option<usize>,
    #[allow(dead_code)]
    pub workspace_roots: Vec<WorkspaceRoot>,
    #[allow(dead_code)]
    pub scan_budget: ScanBudget,
    #[allow(dead_code)]
    pub discarded_roots: Vec<(PathBuf, String)>,
    #[allow(dead_code)]
    pub preferences_path: PathBuf,
    pub editor: EditorEngine,
    pub view_mode: ViewMode,
    pub drawer_open: bool,
    pub drawer_tab: DrawerTab,
    pub settings_tab: SettingsTab,
    pub preferences: Preferences,
    pub theme: ThemeColors,
    pub strings: &'static I18nStrings,
    pub is_system_dark: bool,
    pub is_quitting: bool,
    pub error_message: Option<String>,
    /// Set on every editor mutation so the dirty dot updates without joining
    /// the whole buffer on each keystroke. Cleared when the buffer is synced.
    editor_dirty: bool,
    #[allow(dead_code)]
    pub search_query: String,
    #[allow(dead_code)]
    pub search_original_expanded: Option<std::collections::HashMap<std::path::PathBuf, std::collections::HashSet<std::path::PathBuf>>>,
    #[allow(dead_code)]
    pub search_generation: u64,
    #[allow(dead_code)]
    pub search_in_progress: bool,
    #[allow(dead_code)]
    pub search_truncated: bool,
}

impl AppState {
    pub fn open_documents(&self) -> &[OpenDoc] { &self.open_docs }
    pub fn doc(&self) -> &Document {
        match &self.active {
            Some(key) => &self.open_docs.iter().find(|d| &d.key == key).expect("active document missing").doc,
            None => &self.empty_doc,
        }
    }
    pub(crate) fn doc_mut(&mut self) -> &mut Document {
        match &self.active {
            Some(key) => &mut self.open_docs.iter_mut().find(|d| &d.key == key).expect("active document missing").doc,
            None => &mut self.empty_doc,
        }
    }
    pub fn new(doc: Document) -> Self {
        let preferences = Preferences::load_or_default();
        Self::new_with_preferences_at(doc, preferences, Preferences::default_config_path())
    }

    #[allow(dead_code)]
    pub fn new_isolated(doc: Document) -> Self {
        let directory = tempfile::tempdir().expect("create isolated preferences directory");
        let mut state = Self::new_with_preferences_at(
            doc, Preferences::default(), directory.path().join("config.toml"),
        );
        state._isolated_preferences_dir = Some(directory);
        state
    }

    #[allow(dead_code)]
    pub fn new_with_preferences(doc: Document, preferences: Preferences) -> Self {
        Self::new_with_preferences_at(doc, preferences, Preferences::default_config_path())
    }

    #[allow(dead_code)]
    pub fn new_with_preferences_at(
        doc: Document,
        mut preferences: Preferences,
        preferences_path: PathBuf,
    ) -> Self {
        if !crate::theme::ThemeColors::is_valid_palette_id(&preferences.palette) {
            preferences.palette = "default".to_string();
            let _ = preferences.save_to_path(&preferences_path);
        }
        let is_dark = match preferences.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::Auto => false,
        };
        let theme = ThemeColors::resolve_preferences(&preferences, is_dark);
        let strings = get_strings(preferences.locale);
        let editor = EditorEngine::new(&doc.buffer);

        let normalized = normalize_loaded_roots(&preferences.workspace_roots);
        let workspace_roots = normalized.roots;
        let discarded_roots = normalized.discarded;

        let key = if let Some(ref p) = doc.path {
            DocKey::Path(p.clone())
        } else {
            DocKey::Scratch(1)
        };
        let open_docs = vec![OpenDoc {
            key: key.clone(),
            doc,
        }];
        let active = Some(key);

        let mut state = Self {
            open_docs,
            active,
            next_scratch_id: 2,
            pending_close: None,
            pending_remove_root: None,
            workspace_roots,
            scan_budget: ScanBudget::default(),
            discarded_roots,
            preferences_path,
            empty_doc: Document::new_scratch(),
            _isolated_preferences_dir: None,
            editor,
            view_mode: ViewMode::Split,
            drawer_open: false,
            drawer_tab: DrawerTab::Files,
            settings_tab: SettingsTab::Appearance,
            preferences,
            theme,
            strings,
            is_system_dark: false,
            is_quitting: false,
            error_message: None,
            editor_dirty: false,
            search_query: String::new(),
            search_original_expanded: None,
            search_generation: 0,
            search_in_progress: false,
            search_truncated: false,
        };
        state.refresh_all_roots();
        state
    }

    pub fn set_view_mode(&mut self, mode: ViewMode) {
        self.view_mode = mode;
    }

    #[allow(dead_code)]
    pub fn toggle_drawer(&mut self) {
        self.drawer_open = !self.drawer_open;
    }

    pub fn set_drawer_tab(&mut self, tab: DrawerTab) {
        self.drawer_tab = tab;
    }

    #[allow(dead_code)]
    pub fn set_error(&mut self, err: impl Into<String>) {
        self.error_message = Some(err.into());
    }

    #[allow(dead_code)]
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    #[allow(dead_code)]
    pub fn set_settings_tab(&mut self, tab: SettingsTab) {
        self.settings_tab = tab;
    }

    pub fn set_auto_save(&mut self, enabled: bool) {
        self.preferences.auto_save = enabled;
        let _ = self.preferences.save_to_path(&self.preferences_path);
    }

    pub fn set_auto_save_on_blur(&mut self, enabled: bool) {
        self.preferences.auto_save_on_blur = enabled;
        let _ = self.preferences.save_to_path(&self.preferences_path);
    }

    pub fn set_reduce_motion(&mut self, enabled: bool) {
        self.preferences.reduce_motion = enabled;
        let _ = self.preferences.save_to_path(&self.preferences_path);
    }

    pub fn set_show_line_numbers(&mut self, enabled: bool) {
        self.preferences.show_line_numbers = enabled;
        let _ = self.preferences.save_to_path(&self.preferences_path);
    }

    pub fn set_tab_size(&mut self, tab_size: usize) {
        self.preferences.tab_size = tab_size;
        let _ = self.preferences.save_to_path(&self.preferences_path);
    }

    pub fn set_line_height(&mut self, val: f32) {
        let clamped = Preferences::sanitize_line_height(val);
        self.preferences.line_height = clamped;
        let _ = self.preferences.save_to_path(&self.preferences_path);
    }

    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        self.preferences.theme = mode;
        let _ = self.preferences.save_to_path(&self.preferences_path);
        self.recompute_theme();
    }

    pub fn set_palette(&mut self, palette_id: &str) {
        self.preferences.palette = palette_id.to_string();
        let _ = self.preferences.save_to_path(&self.preferences_path);
        self.recompute_theme();
    }

    /// Updates one customizable color slot for the given light or dark variant.
    pub fn set_custom_theme_color(
        &mut self,
        is_dark: bool,
        slot: zmark_core::config::CustomColorSlot,
        hex: &str,
    ) {
        self.preferences
            .custom_theme
            .variant_mut(is_dark)
            .set(slot, Some(hex.to_string()));
        let _ = self.preferences.save_to_path(&self.preferences_path);
        self.recompute_theme();
    }

    /// Copies all customizable colors from a built-in preset into a custom variant.
    pub fn import_palette_into_custom(&mut self, is_dark: bool, source_palette_id: &str) {
        let imported = ThemeColors::resolve(source_palette_id, is_dark).to_custom_palette();
        *self.preferences.custom_theme.variant_mut(is_dark) = imported;
        self.preferences.palette = zmark_core::config::CUSTOM_PALETTE_ID.to_string();
        let _ = self.preferences.save_to_path(&self.preferences_path);
        self.recompute_theme();
    }

    /// Clears custom color overrides for the given light or dark variant back to Default.
    pub fn reset_custom_theme(&mut self, is_dark: bool) {
        *self.preferences.custom_theme.variant_mut(is_dark) = Default::default();
        let _ = self.preferences.save_to_path(&self.preferences_path);
        self.recompute_theme();
    }

    pub fn set_locale(&mut self, locale: SupportedLocale) {
        self.preferences.locale = locale;
        let _ = self.preferences.save_to_path(&self.preferences_path);
        self.strings = get_strings(locale);
    }

    pub fn set_shortcut(
        &mut self,
        action: zmark_core::config::ShortcutActionId,
        spec: Option<String>,
    ) {
        self.preferences.set_shortcut(action, spec);
        let _ = self.preferences.save_to_path(&self.preferences_path);
    }

    pub fn reset_shortcut(&mut self, action: zmark_core::config::ShortcutActionId) {
        self.preferences.reset_shortcut(action);
        let _ = self.preferences.save_to_path(&self.preferences_path);
    }

    pub fn reset_all_shortcuts(&mut self) {
        self.preferences.reset_all_shortcuts();
        let _ = self.preferences.save_to_path(&self.preferences_path);
    }

    pub fn update_appearance(&mut self, is_dark: bool) {
        self.is_system_dark = is_dark;
        let effective_dark = match self.preferences.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::Auto => is_dark,
        };
        if self.theme.is_dark != effective_dark {
            self.theme = ThemeColors::resolve_preferences(&self.preferences, effective_dark);
        }
    }

    pub fn recompute_theme(&mut self) {
        let is_dark = match self.preferences.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::Auto => self.is_system_dark,
        };
        self.theme = ThemeColors::resolve_preferences(&self.preferences, is_dark);
    }

    /// Marks the active document dirty without joining every line into a new
    /// String. The full buffer is materialized by [`Self::sync_from_editor`]
    /// when parsing, saving, switching, or auto-saving.
    pub fn mark_editor_dirty(&mut self) {
        if self.active.is_some() {
            self.editor_dirty = true;
        }
    }

    /// Mirrors the editor buffer into the document. Returns whether the text changed.
    pub fn sync_from_editor(&mut self) -> bool {
        if self.active.is_none() {
            self.editor_dirty = false;
            return false;
        }
        let new_text = self.editor.to_text();
        let changed = self.doc_mut().set_buffer(new_text);
        self.editor_dirty = false;
        changed
    }

    pub fn reload_active_document(&mut self) -> std::io::Result<()> {
        let path = self.doc().path.as_ref().ok_or_else(|| std::io::Error::new(
            std::io::ErrorKind::NotFound, "Draft has no disk file",
        ))?;
        let text = fs::read_to_string(path)?;
        if text.contains('\0') {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Disk file is no longer text"));
        }
        self.doc_mut().resolve_conflict_reload_disk(text);
        let buffer = &self.open_docs.iter().find(|open| self.active.as_ref() == Some(&open.key)).unwrap().doc.buffer;
        self.editor.reset_text(buffer);
        self.editor_dirty = false;
        Ok(())
    }

    /// Called only at activation/save boundaries; inactive documents keep their AST unloaded.
    pub fn check_disk_changes(&mut self) -> std::io::Result<bool> {
        self.sync_from_editor();
        let mut active_reloaded = false;
        let mut failure = None;
        for open in &mut self.open_docs {
            match open.doc.check_disk_change() {
                Ok(true) if self.active.as_ref() == Some(&open.key) => active_reloaded = true,
                Ok(true) => open.doc.unload_parsed(),
                Ok(false) => {}
                Err(error) => { failure.get_or_insert(error); }
            }
        }
        if active_reloaded {
            let buffer = &self.open_docs.iter().find(|open| self.active.as_ref() == Some(&open.key)).unwrap().doc.buffer;
            self.editor.reset_text(buffer);
        }
        if let Some(error) = failure { return Err(error); }
        Ok(active_reloaded)
    }

    /// Parses the buffer now, ignoring the debounce tier.
    pub fn parse_now(&mut self) -> bool {
        self.doc_mut().reparse()
    }

    pub fn parse_is_stale(&self) -> bool {
        self.doc().is_parse_stale()
    }

    #[allow(dead_code)]
    pub fn find_root_index_for_path(&self, path: &Path) -> Option<usize> {
        let cmp_path = resolve_cmp_path(path);
        for (idx, r) in self.workspace_roots.iter().enumerate() {
            if r.is_missing {
                continue;
            }
            let r_cmp = resolve_cmp_path(&r.path);
            if cmp_path.starts_with(&r_cmp) || path.starts_with(&r.path) {
                return Some(idx);
            }
        }
        None
    }

    #[allow(dead_code)]
    pub fn loose_docs(&self) -> Vec<DocKey> {
        self.open_docs
            .iter()
            .filter_map(|od| match &od.key {
                DocKey::Scratch(_) => None,
                DocKey::Path(p) => {
                    if self.find_root_index_for_path(p).is_none() {
                        Some(od.key.clone())
                    } else {
                        None
                    }
                }
            })
            .collect()
    }

    pub fn expand_to_file(&mut self, file_path: &std::path::Path) {
        if let Some(root_idx) = self.find_root_index_for_path(file_path) {
            if self.workspace_roots[root_idx].deferred && self.workspace_roots[root_idx].tree.is_empty() {
                self.refresh_root(root_idx);
            }
            self.workspace_roots[root_idx].is_expanded = true;
            let target_cmp = resolve_cmp_path(file_path);

            fn expand_in_nodes(
                nodes: &mut [FileTreeNode],
                target: &std::path::Path,
                target_cmp: &std::path::Path,
                depth: usize,
                budget: &mut ScanBudget,
                entries_used: &mut usize,
                files_used: &mut usize,
                is_truncated: &mut bool,
                avail_entries: usize,
                avail_files: usize,
            ) -> bool {
                let mut found = false;
                for node in nodes.iter_mut() {
                    let node_cmp = resolve_cmp_path(&node.path);
                    if node.is_dir && (target.starts_with(&node.path) || target_cmp.starts_with(&node_cmp)) {
                        node.is_expanded = true;
                        if node.children.is_empty() {
                            if avail_entries == 0 || avail_files == 0 {
                                *is_truncated = true;
                            }
                            let empty_set = HashSet::new();
                            let prev_e = budget.entries_left;
                            let prev_f = budget.files_left;
                            let (children, child_trunc) = scan_root_layer(&node.path, depth + 1, budget, &empty_set, 4);
                            *entries_used += prev_e.saturating_sub(budget.entries_left);
                            *files_used += prev_f.saturating_sub(budget.files_left);
                            node.children = children;
                            if child_trunc {
                                *is_truncated = true;
                            }
                        }
                        expand_in_nodes(
                            &mut node.children,
                            target,
                            target_cmp,
                            depth + 1,
                            budget,
                            entries_used,
                            files_used,
                            is_truncated,
                            avail_entries,
                            avail_files,
                        );
                        found = true;
                    }
                }
                found
            }

            let root_rem_entries =
                ScanBudget::PER_ROOT_ENTRY_CAP.saturating_sub(self.workspace_roots[root_idx].entries_used);
            let root_rem_files =
                ScanBudget::PER_ROOT_FILE_CAP.saturating_sub(self.workspace_roots[root_idx].files_used);
            let avail_entries = root_rem_entries.min(self.scan_budget.entries_left);
            let avail_files = root_rem_files.min(self.scan_budget.files_left);
            let mut budget = ScanBudget::new(avail_entries, avail_files);
            let mut e_used = 0;
            let mut f_used = 0;
            let mut is_truncated = self.workspace_roots[root_idx].truncated;

            expand_in_nodes(
                &mut self.workspace_roots[root_idx].tree,
                file_path,
                &target_cmp,
                0,
                &mut budget,
                &mut e_used,
                &mut f_used,
                &mut is_truncated,
                avail_entries,
                avail_files,
            );

            self.workspace_roots[root_idx].truncated = is_truncated;
            self.workspace_roots[root_idx].entries_used += e_used;
            self.workspace_roots[root_idx].files_used += f_used;
            self.scan_budget.entries_left = self.scan_budget.entries_left.saturating_sub(e_used);
            self.scan_budget.files_left = self.scan_budget.files_left.saturating_sub(f_used);
        }
    }

    #[allow(dead_code)]
    pub fn add_workspace_root(&mut self, path: PathBuf) -> Result<usize, RootAddError> {
        if !path.exists() {
            return Err(RootAddError::NotFound);
        }
        if !path.is_dir() {
            return Err(RootAddError::NotADirectory);
        }
        let canonical = match std::fs::canonicalize(&path) {
            Ok(c) => c,
            Err(e) => return Err(RootAddError::CanonicalizeFailed(path, e.to_string())),
        };

        // Check if already in workspace_roots
        let existing_match = self.workspace_roots.iter().enumerate().find_map(|(idx, r)| {
            let matches = r.path == canonical
                || r.path == path
                || std::fs::canonicalize(&r.path).map(|c| c == canonical).unwrap_or(false);
            if matches {
                Some((idx, r.is_missing))
            } else {
                None
            }
        });

        if let Some((idx, is_missing)) = existing_match {
            if is_missing {
                let old_path = self.workspace_roots[idx].path.clone();
                self.workspace_roots[idx].path = canonical.clone();
                self.workspace_roots[idx].is_missing = false;
                self.workspace_roots[idx].is_expanded = false;
                self.preferences.workspace_roots =
                    self.workspace_roots.iter().map(|root| root.path.clone()).collect();
                if let Err(e) = self.preferences.save_to_path(&self.preferences_path) {
                    // Atomic rollback
                    self.workspace_roots[idx].path = old_path;
                    self.workspace_roots[idx].is_missing = true;
                    self.workspace_roots[idx].is_expanded = false;
                    self.preferences.workspace_roots =
                        self.workspace_roots.iter().map(|root| root.path.clone()).collect();
                    return Err(RootAddError::SaveFailed(e.to_string()));
                }
                self.refresh_root(idx);
                return Ok(idx);
            } else {
                return Err(RootAddError::AlreadyExists);
            }
        }

        // Check nesting relationships (missing roots MUST participate in nesting check).
        // Nested children inside an existing root are still rejected; adding a parent
        // directory absorbs any existing child roots it contains.
        let mut absorb_indices = Vec::new();
        for (idx, r) in self.workspace_roots.iter().enumerate() {
            let r_cmp = resolve_cmp_path(&r.path);

            if canonical.starts_with(&r_cmp) {
                return Err(RootAddError::NestedInExisting(r.path.clone()));
            }
            if r_cmp.starts_with(&canonical) {
                absorb_indices.push(idx);
            }
        }

        let rollback_snapshot = (!absorb_indices.is_empty()).then(|| {
            (
                self.workspace_roots.clone(),
                self.preferences.workspace_roots.clone(),
                self.scan_budget.clone(),
                self.search_original_expanded.clone(),
            )
        });

        let mut absorbed_expanded_paths = Vec::new();
        let insert_idx = if let Some(&first_idx) = absorb_indices.first() {
            for &idx in absorb_indices.iter().rev() {
                let removed = self.workspace_roots.remove(idx);
                if !removed.is_missing && removed.is_expanded {
                    absorbed_expanded_paths.push(removed.path.clone());
                    absorbed_expanded_paths.extend(removed.expanded_folders());
                }
                if let Some(ref mut saved) = self.search_original_expanded {
                    saved.remove(&removed.path);
                }
                self.scan_budget.entries_left = self
                    .scan_budget
                    .entries_left
                    .saturating_add(removed.entries_used)
                    .min(ScanBudget::MAX_GLOBAL_ENTRIES);
                self.scan_budget.files_left = self
                    .scan_budget
                    .files_left
                    .saturating_add(removed.files_used)
                    .min(ScanBudget::MAX_GLOBAL_FILES);
            }
            first_idx
        } else {
            self.workspace_roots.len()
        };

        self.workspace_roots.insert(
            insert_idx,
            WorkspaceRoot {
                path: canonical,
                tree: Vec::new(),
                is_expanded: false,
                is_missing: false,
                truncated: false,
                entries_used: 0,
                files_used: 0,
                deferred: false,
            },
        );
        self.preferences.workspace_roots =
            self.workspace_roots.iter().map(|root| root.path.clone()).collect();
        if let Err(e) = self.preferences.save_to_path(&self.preferences_path) {
            // Atomic rollback
            if let Some((roots, pref_roots, budget, search_expanded)) = rollback_snapshot {
                self.workspace_roots = roots;
                self.preferences.workspace_roots = pref_roots;
                self.scan_budget = budget;
                self.search_original_expanded = search_expanded;
            } else {
                self.workspace_roots.remove(insert_idx);
                self.preferences.workspace_roots =
                    self.workspace_roots.iter().map(|root| root.path.clone()).collect();
            }
            return Err(RootAddError::SaveFailed(e.to_string()));
        }
        self.refresh_root(insert_idx);
        absorbed_expanded_paths.sort();
        for expanded_path in absorbed_expanded_paths {
            self.expand_to_file(&expanded_path);
        }
        Ok(insert_idx)
    }

    #[allow(dead_code)]
    pub fn refresh_root(&mut self, root_idx: usize) {
        if root_idx >= self.workspace_roots.len() {
            return;
        }
        if self.workspace_roots[root_idx].is_missing {
            return;
        }

        let refund_e = self.workspace_roots[root_idx].entries_used;
        let refund_f = self.workspace_roots[root_idx].files_used;

        let total_e = (self.scan_budget.entries_left + refund_e).min(ScanBudget::MAX_GLOBAL_ENTRIES);
        let total_f = (self.scan_budget.files_left + refund_f).min(ScanBudget::MAX_GLOBAL_FILES);

        if total_f == 0 {
            self.workspace_roots[root_idx].deferred = true;
            self.workspace_roots[root_idx].truncated = false;
            return; // 预算为 0 时保留旧 tree，不粗暴 clear()
        }

        // 成功退还配额给全局预算
        self.scan_budget.entries_left = total_e;
        self.scan_budget.files_left = total_f;
        self.workspace_roots[root_idx].entries_used = 0;
        self.workspace_roots[root_idx].files_used = 0;
        self.workspace_roots[root_idx].deferred = false;

        let root_entries = self.scan_budget.entries_left.min(ScanBudget::PER_ROOT_ENTRY_CAP);
        let root_files = self.scan_budget.files_left.min(ScanBudget::PER_ROOT_FILE_CAP);

        let mut root_budget = ScanBudget::new(root_entries, root_files);
        let mut expanded_paths = HashSet::new();
        collect_expanded_paths(&self.workspace_roots[root_idx].tree, &mut expanded_paths);

        let root_path = self.workspace_roots[root_idx].path.clone();
        let (tree, truncated) = scan_root_layer(&root_path, 0, &mut root_budget, &expanded_paths, 4);

        let consumed_entries = root_entries.saturating_sub(root_budget.entries_left);
        let consumed_files = root_files.saturating_sub(root_budget.files_left);

        self.workspace_roots[root_idx].entries_used = consumed_entries;
        self.workspace_roots[root_idx].files_used = consumed_files;
        self.scan_budget.entries_left = self.scan_budget.entries_left.saturating_sub(consumed_entries);
        self.scan_budget.files_left = self.scan_budget.files_left.saturating_sub(consumed_files);

        self.workspace_roots[root_idx].deferred = false;
        self.workspace_roots[root_idx].truncated = truncated;
        self.workspace_roots[root_idx].tree = tree;
    }

    pub fn refresh_all_roots(&mut self) {
        self.refresh_all_roots_with_budget(ScanBudget::default());
    }

    #[allow(dead_code)]
    pub fn refresh_all_roots_with_budget(&mut self, mut global_budget: ScanBudget) {
        let non_missing_count = self.workspace_roots.iter().filter(|r| !r.is_missing).count();
        let mut non_missing_seen = 0;

        for idx in 0..self.workspace_roots.len() {
            if self.workspace_roots[idx].is_missing {
                continue;
            }
            let slots = non_missing_count.saturating_sub(non_missing_seen);
            if slots == 0 {
                break;
            }
            let floor_entries =
                ScanBudget::FIRST_LAYER_FLOOR_ENTRIES.min(global_budget.entries_left / slots);
            let floor_files =
                ScanBudget::FIRST_LAYER_FLOOR_FILES.min(global_budget.files_left / slots);
            let reserved_entries = floor_entries * (slots - 1);
            let reserved_files = floor_files * (slots - 1);
            let available_entries = global_budget.entries_left.saturating_sub(reserved_entries);
            let available_files = global_budget.files_left.saturating_sub(reserved_files);
            let root_entries = available_entries.min(ScanBudget::PER_ROOT_ENTRY_CAP);
            let root_files = available_files.min(ScanBudget::PER_ROOT_FILE_CAP);

            non_missing_seen += 1;

            if root_files == 0 {
                self.workspace_roots[idx].deferred = true;
                self.workspace_roots[idx].truncated = false;
                self.workspace_roots[idx].entries_used = 0;
                self.workspace_roots[idx].files_used = 0;
                continue;
            }

            let mut root_budget = ScanBudget::new(root_entries, root_files);

            let mut expanded_paths = HashSet::new();
            collect_expanded_paths(&self.workspace_roots[idx].tree, &mut expanded_paths);

            let root_path = self.workspace_roots[idx].path.clone();
            let (tree, truncated) =
                scan_root_layer(&root_path, 0, &mut root_budget, &expanded_paths, 4);

            let consumed_entries = root_entries.saturating_sub(root_budget.entries_left);
            let consumed_files = root_files.saturating_sub(root_budget.files_left);

            self.workspace_roots[idx].entries_used = consumed_entries;
            self.workspace_roots[idx].files_used = consumed_files;
            self.workspace_roots[idx].deferred = false;
            self.workspace_roots[idx].tree = tree;
            self.workspace_roots[idx].truncated = truncated;

            global_budget.entries_left = global_budget.entries_left.saturating_sub(consumed_entries);
            global_budget.files_left = global_budget.files_left.saturating_sub(consumed_files);
        }
        self.scan_budget = global_budget;
    }

    pub fn folder_expanded(&self, path: &Path) -> Option<bool> {
        fn find(nodes: &[FileTreeNode], path: &Path) -> Option<bool> {
            nodes.iter().find_map(|n| {
                if n.path == path && n.is_dir { Some(n.is_expanded) } else { find(&n.children, path) }
            })
        }
        self.workspace_roots.iter().find_map(|r| {
            if r.path == path { Some(r.is_expanded) } else { find(&r.tree, path) }
        })
    }

    #[allow(dead_code)]
    pub fn toggle_root_folder(&mut self, root_idx: usize, target: &Path) -> bool {
        if root_idx >= self.workspace_roots.len() {
            return false;
        }
        if self.workspace_roots[root_idx].is_missing {
            return false;
        }

        if self.workspace_roots[root_idx].path == target {
            self.workspace_roots[root_idx].is_expanded = !self.workspace_roots[root_idx].is_expanded;
            return true;
        }

        let root_rem_entries =
            ScanBudget::PER_ROOT_ENTRY_CAP.saturating_sub(self.workspace_roots[root_idx].entries_used);
        let root_rem_files =
            ScanBudget::PER_ROOT_FILE_CAP.saturating_sub(self.workspace_roots[root_idx].files_used);
        let avail_entries = root_rem_entries.min(self.scan_budget.entries_left);
        let avail_files = root_rem_files.min(self.scan_budget.files_left);

        let mut budget = ScanBudget::new(avail_entries, avail_files);
        let mut is_truncated = self.workspace_roots[root_idx].truncated;
        let mut did_expand = false;

        fn toggle_in_nodes(
            nodes: &mut [FileTreeNode],
            target: &Path,
            depth: usize,
            budget: &mut ScanBudget,
            is_truncated: &mut bool,
            did_expand: &mut bool,
            avail_entries: usize,
            avail_files: usize,
        ) -> bool {
            for node in nodes.iter_mut() {
                if node.path == target {
                    if node.is_dir {
                        node.is_expanded = !node.is_expanded;
                        if node.is_expanded && node.children.is_empty() {
                            *did_expand = true;
                            if avail_entries == 0 || avail_files == 0 {
                                *is_truncated = true;
                            }
                            let empty_set = HashSet::new();
                            let (children, child_trunc) =
                                scan_root_layer(&node.path, depth + 1, budget, &empty_set, 4);
                            node.children = children;
                            if child_trunc {
                                *is_truncated = true;
                            }
                        }
                    }
                    return true;
                }
                if node.is_dir
                    && toggle_in_nodes(
                        &mut node.children,
                        target,
                        depth + 1,
                        budget,
                        is_truncated,
                        did_expand,
                        avail_entries,
                        avail_files,
                    )
                {
                    return true;
                }
            }
            false
        }

        let found = toggle_in_nodes(
            &mut self.workspace_roots[root_idx].tree,
            target,
            0,
            &mut budget,
            &mut is_truncated,
            &mut did_expand,
            avail_entries,
            avail_files,
        );

        if found {
            self.workspace_roots[root_idx].truncated = is_truncated;
            if did_expand {
                let consumed_entries = avail_entries.saturating_sub(budget.entries_left);
                let consumed_files = avail_files.saturating_sub(budget.files_left);
                self.workspace_roots[root_idx].entries_used =
                    self.workspace_roots[root_idx].entries_used.saturating_add(consumed_entries);
                self.workspace_roots[root_idx].files_used =
                    self.workspace_roots[root_idx].files_used.saturating_add(consumed_files);
                self.scan_budget.entries_left = self.scan_budget.entries_left.saturating_sub(consumed_entries);
                self.scan_budget.files_left = self.scan_budget.files_left.saturating_sub(consumed_files);
            }
        }

        found
    }

    #[allow(dead_code)]
    pub fn remove_workspace_root(&mut self, idx: usize) -> Result<Option<WorkspaceRoot>, String> {
        if idx >= self.workspace_roots.len() {
            return Ok(None);
        }
        let removed = self.workspace_roots.remove(idx);
        self.preferences.workspace_roots =
            self.workspace_roots.iter().map(|r| r.path.clone()).collect();
        if let Err(e) = self.preferences.save_to_path(&self.preferences_path) {
            // 原子回滚
            self.workspace_roots.insert(idx, removed);
            self.preferences.workspace_roots =
                self.workspace_roots.iter().map(|r| r.path.clone()).collect();
            return Err(format!("Failed to save preferences: {}", e));
        }
        if let Some(ref mut saved) = self.search_original_expanded {
            saved.remove(&removed.path);
        }
        self.scan_budget.entries_left =
            (self.scan_budget.entries_left.saturating_add(removed.entries_used))
                .min(ScanBudget::MAX_GLOBAL_ENTRIES);
        self.scan_budget.files_left =
            (self.scan_budget.files_left.saturating_add(removed.files_used))
                .min(ScanBudget::MAX_GLOBAL_FILES);
        Ok(Some(removed))
    }

    pub fn request_remove_workspace_root(&mut self, idx: usize) {
        if idx < self.workspace_roots.len() {
            self.pending_remove_root = Some(idx);
        }
    }

    pub fn cancel_remove_workspace_root(&mut self) {
        self.pending_remove_root = None;
    }

    pub fn confirm_remove_workspace_root(&mut self) -> Result<Option<WorkspaceRoot>, String> {
        if let Some(idx) = self.pending_remove_root.take() {
            self.remove_workspace_root(idx)
        } else {
            Ok(None)
        }
    }

    #[allow(dead_code)]
    pub fn active_key(&self) -> Option<&DocKey> {
        self.active.as_ref()
    }

    #[allow(dead_code)]
    pub fn active_open_doc(&self) -> Option<&OpenDoc> {
        let key = self.active.as_ref()?;
        self.open_docs.iter().find(|d| &d.key == key)
    }

    pub fn open_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        let path = path.canonicalize()?;
        self.sync_from_editor();

        let target_key = DocKey::Path(path.clone());

        // Check if already open
        if self.open_docs.iter().any(|d| d.key == target_key) {
            self.switch_document(&target_key);
            self.expand_to_file(&path);
            return Ok(());
        }

        let new_doc = Document::from_file(path.clone())?;
        // If single empty clean scratch document, replace it
        if self.open_docs.len() == 1
            && self.open_docs[0].key.is_scratch()
            && self.open_docs[0].doc.buffer.trim().is_empty()
            && self.open_docs[0].doc.status != DocumentStatus::Modified
        {
            self.open_docs[0] = OpenDoc {
                key: target_key.clone(),
                doc: new_doc,
            };
        } else {
            self.open_docs.push(OpenDoc {
                key: target_key.clone(),
                doc: new_doc,
            });
        }

        let previous_key = self.active.clone();
        self.active = Some(target_key.clone());
        if let Some(previous_key) = previous_key.filter(|old| old != &target_key) {
            if let Some(old_pos) = self.open_docs.iter().position(|d| d.key == previous_key) {
                self.open_docs[old_pos].doc.unload_parsed();
            }
        }
        let buffer = &self.open_docs.iter().find(|d| self.active.as_ref() == Some(&d.key)).unwrap().doc.buffer;
        self.editor.reset_text(buffer);
        if let Some(idx) = self.find_root_index_for_path(&path) {
            self.refresh_root(idx);
        }
        self.expand_to_file(&path);
        Ok(())
    }

    pub fn switch_document(&mut self, key: &DocKey) {
        if self.active.as_ref() == Some(key) {
            return;
        }
        self.sync_from_editor();

        if let Some(pos) = self.open_docs.iter().position(|d| &d.key == key) {
            let old_key = self.active.clone();
            if let Some(old_key) = old_key.filter(|old| old != key) {
                if let Some(old_pos) = self.open_docs.iter().position(|d| d.key == old_key) {
                    self.open_docs[old_pos].doc.unload_parsed();
                }
            }
            self.active = Some(key.clone());
            self.open_docs[pos].doc.reparse();
            self.editor.reset_text(&self.open_docs[pos].doc.buffer);
            if let DocKey::Path(ref p) = key {
                self.expand_to_file(p);
            }
        }
    }

    /// Focuses the existing draft slot, or creates a new draft if none exists.
    pub fn focus_or_create_draft(&mut self) {
        self.sync_from_editor();

        if let Some(existing) = self.open_docs.iter().find(|d| d.key.is_scratch()) {
            let key = existing.key.clone();
            self.switch_document(&key);
            return;
        }

        let scratch = Document::new_scratch();
        let key = DocKey::Scratch(self.next_scratch_id);
        self.next_scratch_id += 1;

        self.open_docs.push(OpenDoc {
            key: key.clone(),
            doc: scratch,
        });
        self.active = Some(key);
        self.editor.reset_text("");
    }

    pub fn plan_close(&self, key: &DocKey) -> Option<CloseDecision> {
        let od = self.open_docs.iter().find(|d| &d.key == key)?;
        let is_active = self.active.as_ref() == Some(key);
        let is_dirty = if is_active {
            self.doc().status != DocumentStatus::Clean
                || self.editor.to_text() != self.doc().buffer
        } else {
            od.doc.status != DocumentStatus::Clean
        };

        if is_dirty {
            Some(CloseDecision::PromptDirty {
                key: key.clone(),
                title: od.doc.display_name_with_fallback(self.strings.untitled_file),
                has_path: od.doc.path.is_some(),
            })
        } else {
            Some(CloseDecision::CloseImmediately)
        }
    }

    pub fn request_close(&mut self, key: &DocKey) -> CloseDecision {
        self.sync_from_editor();
        let decision = match self.plan_close(key) {
            Some(d) => d,
            None => return CloseDecision::CloseImmediately,
        };

        match &decision {
            CloseDecision::PromptDirty { key, title, has_path } => {
                // If closing a background dirty document, switch to it first so user sees what is being closed
                if self.active.as_ref() != Some(key) {
                    self.switch_document(key);
                }
                self.pending_close = Some(PendingClose {
                    key: key.clone(),
                    title: title.clone(),
                    has_path: *has_path,
                });
                decision
            }
            CloseDecision::CloseImmediately => {
                self.close_document_forced(key);
                decision
            }
        }
    }

    pub fn next_dirty_doc_key(&mut self) -> Option<DocKey> {
        self.sync_from_editor();
        if self.is_dirty() {
            return self.active.clone();
        }
        self.open_docs.iter().find(|od| {
            od.doc.status != DocumentStatus::Clean
        }).map(|od| od.key.clone())
    }

    /// Evaluates window exit strategy:
    /// - If any document is modified, mark is_quitting = true, switch to it, trigger close prompt, and return false.
    /// - Otherwise, allow immediate exit (returns true).
    pub fn handle_window_should_close(&mut self) -> bool {
        self.sync_from_editor();

        if let Some(key) = self.next_dirty_doc_key() {
            self.is_quitting = true;
            self.request_close(&key);
            false
        } else {
            self.is_quitting = false;
            true
        }
    }

    pub fn cancel_close(&mut self) {
        self.is_quitting = false;
        self.pending_close = None;
    }

    pub fn confirm_close_discard(&mut self, key: &DocKey) {
        self.pending_close = None;
        self.close_document_forced(key);
    }

    pub fn confirm_close_save(&mut self, key: &DocKey) -> Result<bool, std::io::Error> {
        if self.active.as_ref() == Some(key) { self.sync_from_editor(); }
        if let Some(od) = self.open_docs.iter_mut().find(|d| &d.key == key) {
            if od.doc.path.is_some() {
                od.doc.save()?;
                self.pending_close = None;
                self.close_document_forced(key);
                return Ok(true);
            } else {
                // Scratch needs Save As path from UI; keep pending_close until path chosen
                return Ok(false);
            }
        }
        self.pending_close = None;
        Ok(true)
    }

    pub fn close_document_forced(&mut self, key: &DocKey) {
        let pos = match self.open_docs.iter().position(|d| &d.key == key) {
            Some(p) => p,
            None => return,
        };

        self.open_docs.remove(pos);

        if self.active.as_ref() == Some(key) {
            if self.open_docs.is_empty() {
                self.active = None;
                self.empty_doc = Document::new_scratch();
                self.editor.reset_text("");
            } else {
                let next_idx = if pos < self.open_docs.len() {
                    pos
                } else {
                    self.open_docs.len() - 1
                };
                let next_key = self.open_docs[next_idx].key.clone();
                self.active = Some(next_key.clone());
                self.open_docs[next_idx].doc.reparse();
                self.editor.reset_text(&self.open_docs[next_idx].doc.buffer);
                if let DocKey::Path(ref p) = next_key {
                    self.expand_to_file(p);
                }
            }
        }
    }

    pub fn load_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        self.open_file(path)
    }

    pub fn create_file_in_dir(&mut self, dir: &std::path::Path) -> Result<std::path::PathBuf, std::io::Error> {
        if !dir.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Target directory does not exist",
            ));
        }

        let mut candidate = dir.join("Untitled.md");
        let mut counter = 1;
        loop {
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
                Ok(_) => break,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    candidate = dir.join(format!("Untitled {}.md", counter));
                    counter += 1;
                }
                Err(error) => return Err(error),
            }
        }
        self.open_file(candidate.clone())?;
        Ok(candidate)
    }

    pub fn rename_file(&mut self, new_name: &str) -> Result<(), std::io::Error> {
        let Some(key) = self.active.clone() else { return Ok(()); };
        self.rename_document(&key, new_name).map(|_| ())
    }

    pub fn rename_document(&mut self, key: &DocKey, name: &str) -> std::io::Result<Option<String>> {
        if let DocKey::Path(path) = key {
            return self.rename_path(path, name);
        }
        let doc = self.open_docs.iter_mut().find(|d| &d.key == key)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Document is no longer open"))?;
        doc.doc.rename_scratch(name)?;
        Ok(None)
    }

    /// Migrates metadata only after disk commit; never reloads an editor buffer.
    pub fn rename_path(&mut self, old: &Path, name: &str) -> std::io::Result<Option<String>> {
        use zmark_core::file_ops::{markdown_name, remap_path, rename_noreplace, validate_name};
        validate_name(name)?;
        let metadata = fs::symlink_metadata(old)?;
        let is_dir = metadata.is_dir();
        let parent = old.parent().filter(|_| old.file_name().is_some())
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Cannot rename a filesystem root"))?;
        let new = parent.join(if is_dir || !zmark_core::file_ops::is_markdown(old) {
            name.to_owned()
        } else { markdown_name(name) });
        if new == old { return Ok(None); }
        let map = |p: &Path| remap_path(p, old, &new).unwrap_or_else(|| p.to_path_buf());
        let map_key = |key: &DocKey| match key {
            DocKey::Path(p) => DocKey::Path(map(p)),
            key => key.clone(),
        };
        let mut keys = HashSet::new();
        for doc in &self.open_docs {
            if !keys.insert(map_key(&doc.key)) {
                return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Target document is already open"));
            }
        }
        let root_paths: Vec<PathBuf> = self.workspace_roots.iter().map(|r| map(&r.path)).collect();
        for (idx, path) in root_paths.iter().enumerate() {
            let path = resolve_cmp_path(path);
            if root_paths[..idx].iter().any(|other| {
                let other = resolve_cmp_path(other);
                path.starts_with(&other) || other.starts_with(&path)
            }) {
                return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Target overlaps another workspace root"));
            }
        }
        self.sync_from_editor();
        rename_noreplace(old, &new)?;
        let mut warning = None;
        let preferences_path = map(&self.preferences_path);
        let hidden_paths: Vec<_> = self.preferences.sidebar_hidden_paths.iter().map(|p| map(p)).collect();
        let changes_config = hidden_paths != self.preferences.sidebar_hidden_paths || root_paths.iter().zip(&self.workspace_roots).any(|(p, r)| p != &r.path)
            || preferences_path != self.preferences_path;
        if changes_config {
            let mut preferences = self.preferences.clone();
            preferences.workspace_roots = root_paths.clone();
            preferences.sidebar_hidden_paths = hidden_paths;
            if let Err(error) = preferences.save_to_path(&preferences_path) {
                if let Err(rollback) = rename_noreplace(&new, old) {
                    warning = Some(format!("已更名到 {}，但配置未保存且回滚失败：{error}；{rollback}", new.display()));
                } else {
                    return Err(std::io::Error::other(format!("配置保存失败，已回滚更名：{error}")));
                }
            }
            self.preferences = preferences;
            self.preferences_path = preferences_path;
        }
        for doc in &mut self.open_docs {
            doc.key = map_key(&doc.key);
            if let Some(path) = &mut doc.doc.path {
                if let Some(mapped) = remap_path(path, old, &new) {
                    *path = mapped;
                    if !path.is_file() { doc.doc.status = DocumentStatus::ConflictOnDisk; }
                }
            }
        }
        self.active = self.active.as_ref().map(map_key);
        if let Some(pending) = &mut self.pending_close {
            pending.key = map_key(&pending.key);
            if let DocKey::Path(p) = &pending.key {
                pending.title = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            }
        }
        fn migrate_tree(nodes: &mut [FileTreeNode], old: &Path, new: &Path) {
            for node in nodes {
                if let Some(path) = zmark_core::file_ops::remap_path(&node.path, old, new) {
                    node.path = path;
                    node.name = node.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                }
                migrate_tree(&mut node.children, old, new);
            }
        }
        for root in &mut self.workspace_roots {
            if let Some(path) = remap_path(&root.path, old, &new) {
                root.path = path;
                root.is_missing = !root.path.is_dir();
            }
            migrate_tree(&mut root.tree, old, &new);
        }
        if let Some(saved) = self.search_original_expanded.take() {
            self.search_original_expanded = Some(saved.into_iter().map(|(root, paths)| {
                (map(&root), paths.into_iter().map(|p| map(&p)).collect())
            }).collect());
        }
        self.search_generation = self.search_generation.wrapping_add(1);
        self.search_in_progress = false;
        Ok(warning)
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if self.active.is_none() {
            return Ok(());
        }
        self.sync_from_editor();
        let result = self.doc_mut().save();
        if result.is_ok() && self.editor.to_text() != self.doc().buffer {
            let buffer = &self.open_docs.iter().find(|open| self.active.as_ref() == Some(&open.key)).unwrap().doc.buffer;
            self.editor.reset_text(buffer);
        }
        result
    }

    pub fn save_doc(&mut self, key: &DocKey) -> Result<(), std::io::Error> {
        if self.active.as_ref() == Some(key) {
            return self.save();
        }
        let Some(doc) = self.open_docs.iter_mut().find(|doc| &doc.key == key) else {
            return Ok(());
        };
        doc.doc.save()
    }

    pub fn save_doc_as(&mut self, key: &DocKey, path: PathBuf) -> Result<(), std::io::Error> {
        self.sync_from_editor();
        let new_key = DocKey::Path(path.clone());

        // Check if destination path is already open in another document
        if self.open_docs.iter().any(|d| d.key != *key && d.key == new_key) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "A document with this path is already open in workspace",
            ));
        }

        // `Document::save_to` writes first and only then updates the in-memory
        // path/status, so a failed write leaves the document untouched without
        // cloning the whole buffer first.
        let doc = if self.active.as_ref() == Some(key) {
            self.doc_mut()
        } else if let Some(od) = self.open_docs.iter_mut().find(|d| &d.key == key) {
            &mut od.doc
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Document to save was not found",
            ));
        };
        doc.save_to(path.clone())?;

        if let Some(od) = self.open_docs.iter_mut().find(|d| &d.key == key) {
            od.key = new_key.clone();
        }

        if self.active.as_ref() == Some(key) {
            self.active = Some(new_key);
        }

        if let Some(idx) = self.find_root_index_for_path(&path) {
            self.refresh_root(idx);
        }
        self.expand_to_file(&path);
        Ok(())
    }

    pub fn export_standalone_html(&self) -> String {
        let title = self
            .doc()
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(self.strings.untitled);
        export_html(&self.doc().parsed, title)
    }

    pub fn is_dirty(&self) -> bool {
        if self.active.is_none() {
            return false;
        }
        self.editor_dirty || self.doc().status != DocumentStatus::Clean
    }

    pub fn has_conflict(&self) -> bool {
        if self.active.is_none() {
            return false;
        }
        self.doc().status == DocumentStatus::ConflictOnDisk
    }

    #[allow(dead_code)]
    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.to_string();
        self.search_generation = self.search_generation.wrapping_add(1);

        if !query.is_empty() {
            // Save current expanded state on first keystroke
            if self.search_original_expanded.is_none() {
                let mut saved = HashMap::new();
                for r in &self.workspace_roots {
                    let mut set = r.expanded_folders();
                    if r.is_expanded {
                        set.insert(r.path.clone());
                    }
                    saved.insert(r.path.clone(), set);
                }
                self.search_original_expanded = Some(saved);
            } else if let Some(ref saved) = self.search_original_expanded {
                // If query changes while searching, restore each root to original state before re-expanding
                for r in self.workspace_roots.iter_mut() {
                    if let Some(orig) = saved.get(&r.path) {
                        r.is_expanded = orig.contains(&r.path);
                        r.set_expanded_folders(orig);
                    }
                }
            }
            // Filesystem traversal must NOT run synchronously inside set_search_query on the UI thread
            self.search_in_progress = true;
        } else {
            // Restore original expanded folders
            if let Some(saved) = self.search_original_expanded.take() {
                for r in self.workspace_roots.iter_mut() {
                    if let Some(orig) = saved.get(&r.path) {
                        r.is_expanded = orig.contains(&r.path);
                        r.set_expanded_folders(orig);
                    }
                }
            }
            self.search_in_progress = false;
            self.search_truncated = false;
        }
    }

    pub fn prepare_search_job(&self) -> Option<SearchJob> {
        if self.search_query.is_empty() {
            return None;
        }
        let roots = self
            .workspace_roots
            .iter()
            .map(SearchRootSpec::from)
            .collect();

        Some(SearchJob {
            generation: self.search_generation,
            query: self.search_query.clone(),
            roots,
            scan_budget: self.scan_budget.clone(),
        })
    }

    pub fn apply_search_result(&mut self, result: SearchJobResult) -> bool {
        if result.generation != self.search_generation {
            if result.generation >= self.search_generation || self.search_query.is_empty() {
                self.search_in_progress = false;
            }
            return false;
        }

        if result.query != self.search_query {
            self.search_in_progress = false;
            return false;
        }

        let query_lower = self.search_query.to_lowercase();

        for res_root in result.roots {
            if let Some(existing) = self.workspace_roots.iter_mut().find(|r| r.path == res_root.path) {
                if existing.tree.is_empty() {
                    existing.tree = res_root.tree;
                    existing.deferred = res_root.deferred;
                    existing.is_expanded = res_root.is_expanded;
                    apply_expanded_paths(&mut existing.tree, &res_root.matching_expanded_paths);
                } else {
                    merge_node_trees(&mut existing.tree, res_root.tree, &res_root.matching_expanded_paths);
                    if res_root.is_expanded {
                        existing.is_expanded = true;
                    }
                    if !res_root.deferred {
                        existing.deferred = false;
                    }
                }

                if expand_ancestors_for_query(&mut existing.tree, &query_lower) {
                    existing.is_expanded = true;
                }

                existing.truncated = existing.truncated || res_root.truncated;
                existing.entries_used = existing.entries_used.max(res_root.entries_used);
                existing.files_used = existing.files_used.max(res_root.files_used);
            }
        }

        self.scan_budget = result.scan_budget;
        self.search_truncated = result.search_truncated || self.workspace_roots.iter().any(|r| r.truncated);
        self.search_in_progress = false;
        true
    }

    #[allow(dead_code)]
    pub fn execute_search(&mut self) -> bool {
        if self.search_query.is_empty() {
            self.search_in_progress = false;
            return true;
        }
        if let Some(job) = self.prepare_search_job() {
            let result = job.execute();
            let applied = self.apply_search_result(result);
            if !applied {
                self.search_in_progress = false;
            }
            applied
        } else {
            self.search_in_progress = false;
            false
        }
    }

    pub fn clear_search(&mut self) {
        self.set_search_query("");
    }

    #[allow(dead_code)]
    pub fn file_matches_search(&self, filename: &str) -> bool {
        if self.search_query.is_empty() {
            return true;
        }
        filename.to_lowercase().contains(&self.search_query.to_lowercase())
    }

    pub fn has_search_matches(&self) -> bool {
        if self.search_query.is_empty() {
            return true;
        }
        let q = self.search_query.to_lowercase();
        for root in &self.workspace_roots {
            if node_tree_has_match(&root.tree, &q) {
                return true;
            }
        }
        for key in &self.loose_docs() {
            if let DocKey::Path(p) = key {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if name.to_lowercase().contains(&q) {
                        return true;
                    }
                }
            }
        }
        for open_doc in &self.open_docs {
            if open_doc.key.is_scratch() {
                let title = open_doc.doc.display_name_with_fallback(self.strings.untitled_file);
                if title.to_lowercase().contains(&q) {
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRootSpec {
    pub path: PathBuf,
    pub is_missing: bool,
    pub truncated: bool,
    pub entries_used: usize,
    pub files_used: usize,
    pub deferred: bool,
}

impl From<&WorkspaceRoot> for SearchRootSpec {
    fn from(r: &WorkspaceRoot) -> Self {
        Self {
            path: r.path.clone(),
            is_missing: r.is_missing,
            truncated: r.truncated,
            entries_used: r.entries_used,
            files_used: r.files_used,
            deferred: r.deferred,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchRootResult {
    pub path: PathBuf,
    pub tree: Vec<FileTreeNode>,
    pub is_expanded: bool,
    pub truncated: bool,
    pub entries_used: usize,
    pub files_used: usize,
    pub deferred: bool,
    pub matching_expanded_paths: HashSet<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct SearchJob {
    pub generation: u64,
    pub query: String,
    pub roots: Vec<SearchRootSpec>,
    pub scan_budget: ScanBudget,
}

#[derive(Debug, Clone)]
pub struct SearchJobResult {
    pub generation: u64,
    pub query: String,
    pub roots: Vec<SearchRootResult>,
    pub scan_budget: ScanBudget,
    pub search_truncated: bool,
}

impl SearchJob {
    pub fn execute(mut self) -> SearchJobResult {
        let query_lower = self.query.to_lowercase();
        let mut any_truncated = false;
        let mut result_roots = Vec::with_capacity(self.roots.len());

        for root in self.roots {
            if root.is_missing {
                result_roots.push(SearchRootResult {
                    path: root.path,
                    tree: Vec::new(),
                    is_expanded: false,
                    truncated: root.truncated,
                    entries_used: root.entries_used,
                    files_used: root.files_used,
                    deferred: root.deferred,
                    matching_expanded_paths: HashSet::new(),
                });
                continue;
            }

            if root.truncated {
                any_truncated = true;
            }

            let root_rem_entries = ScanBudget::PER_ROOT_ENTRY_CAP.saturating_sub(root.entries_used);
            let root_rem_files = ScanBudget::PER_ROOT_FILE_CAP.saturating_sub(root.files_used);
            let avail_entries = root_rem_entries.min(self.scan_budget.entries_left);
            let avail_files = root_rem_files.min(self.scan_budget.files_left);

            let mut budget = ScanBudget::new(avail_entries, avail_files);
            let mut is_truncated = root.truncated;
            let mut entries_used = 0;
            let mut files_used = 0;
            let mut matching_expanded_paths = HashSet::new();

            let (tree, root_matched) = scan_dir_for_search(
                &root.path,
                0,
                4,
                &query_lower,
                &mut budget,
                &mut entries_used,
                &mut files_used,
                &mut is_truncated,
                &mut matching_expanded_paths,
            );

            let total_entries = root.entries_used + entries_used;
            let total_files = root.files_used + files_used;
            self.scan_budget.entries_left = self.scan_budget.entries_left.saturating_sub(entries_used);
            self.scan_budget.files_left = self.scan_budget.files_left.saturating_sub(files_used);

            if is_truncated {
                any_truncated = true;
            }

            result_roots.push(SearchRootResult {
                path: root.path,
                tree,
                is_expanded: root_matched,
                truncated: is_truncated,
                entries_used: total_entries,
                files_used: total_files,
                deferred: false,
                matching_expanded_paths,
            });
        }

        SearchJobResult {
            generation: self.generation,
            query: self.query,
            roots: result_roots,
            scan_budget: self.scan_budget,
            search_truncated: any_truncated,
        }
    }
}

pub fn scan_dir_for_search(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    query_lower: &str,
    budget: &mut ScanBudget,
    entries_used: &mut usize,
    files_used: &mut usize,
    is_truncated: &mut bool,
    expanded_paths: &mut HashSet<PathBuf>,
) -> (Vec<FileTreeNode>, bool) {
    if depth > max_depth {
        return (Vec::new(), false);
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return (Vec::new(), false),
    };

    let mut dir_entries = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
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
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        dir_entries.push((p, name, file_type));
    }

    dir_entries.sort_by(|a, b| {
        match (a.2.is_dir(), b.2.is_dir()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.1.to_lowercase().cmp(&b.1.to_lowercase()),
        }
    });

    let mut nodes = Vec::new();
    let mut dir_has_match = false;

    for (p, name, file_type) in dir_entries {
        if file_type.is_dir() {
            if !budget.consume_entry() {
                *is_truncated = true;
                break;
            }
            *entries_used += 1;

            let name_matches = name.to_lowercase().contains(query_lower);
            let (children, child_has_match) = if depth < max_depth {
                scan_dir_for_search(
                    &p,
                    depth + 1,
                    max_depth,
                    query_lower,
                    budget,
                    entries_used,
                    files_used,
                    is_truncated,
                    expanded_paths,
                )
            } else {
                (Vec::new(), false)
            };

            let is_matched = name_matches || child_has_match;
            if is_matched {
                dir_has_match = true;
                expanded_paths.insert(p.clone());
            }

            nodes.push(FileTreeNode {
                path: p,
                name,
                is_dir: true,
                is_expanded: is_matched,
                children,
                mtime: None,
            });
        } else if file_type.is_file() {
            if zmark_core::file_ops::is_supported_text(&p) {
                if !budget.consume_file() {
                    *is_truncated = true;
                    break;
                }
                *files_used += 1;

                let name_matches = name.to_lowercase().contains(query_lower);
                if name_matches {
                    dir_has_match = true;
                }

                let raw_mtime = fs::metadata(&p).ok().and_then(|m| m.modified().ok());
                let mtime = format_mtime(raw_mtime, std::time::SystemTime::now());
                nodes.push(FileTreeNode {
                    path: p,
                    name,
                    is_dir: false,
                    is_expanded: false,
                    children: Vec::new(),
                    mtime,
                });
            } else {
                if !budget.consume_entry() {
                    *is_truncated = true;
                    break;
                }
                *entries_used += 1;
            }
        }
    }

    (nodes, dir_has_match)
}

fn merge_node_trees(
    existing: &mut Vec<FileTreeNode>,
    scanned: Vec<FileTreeNode>,
    expanded_paths: &HashSet<PathBuf>,
) {
    for scanned_node in scanned {
        if let Some(existing_node) = existing.iter_mut().find(|n| n.path == scanned_node.path) {
            if scanned_node.is_dir {
                if expanded_paths.contains(&existing_node.path) || scanned_node.is_expanded {
                    existing_node.is_expanded = true;
                }
                if existing_node.children.is_empty() {
                    let mut children = scanned_node.children;
                    for child in children.iter_mut() {
                        if child.is_dir && expanded_paths.contains(&child.path) {
                            child.is_expanded = true;
                        }
                    }
                    existing_node.children = children;
                } else if !scanned_node.children.is_empty() {
                    merge_node_trees(&mut existing_node.children, scanned_node.children, expanded_paths);
                }
            }
        } else {
            let parent_exists = scanned_node.path.parent().map(|p| p.exists()).unwrap_or(false);
            if scanned_node.path.exists() || !parent_exists {
                let mut new_node = scanned_node;
                if new_node.is_dir && expanded_paths.contains(&new_node.path) {
                    new_node.is_expanded = true;
                }
                existing.push(new_node);
            }
        }
    }

    existing.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
}

fn expand_ancestors_for_query(nodes: &mut [FileTreeNode], query_lower: &str) -> bool {
    let mut any_match = false;
    for node in nodes.iter_mut() {
        if node.is_dir {
            let child_match = expand_ancestors_for_query(&mut node.children, query_lower);
            let name_match = node.name.to_lowercase().contains(query_lower);
            if child_match || name_match {
                node.is_expanded = true;
                any_match = true;
            }
        } else if node.name.to_lowercase().contains(query_lower) {
            any_match = true;
        }
    }
    any_match
}

pub fn apply_expanded_paths(nodes: &mut [FileTreeNode], set: &HashSet<PathBuf>) {
    for n in nodes.iter_mut() {
        if n.is_dir {
            n.is_expanded = set.contains(&n.path);
            apply_expanded_paths(&mut n.children, set);
        }
    }
}

pub fn node_tree_has_match(nodes: &[FileTreeNode], q: &str) -> bool {
    for node in nodes {
        if node.is_dir {
            if node.name.to_lowercase().contains(q) || node_tree_has_match(&node.children, q) {
                return true;
            }
        } else if node.name.to_lowercase().contains(q) {
            return true;
        }
    }
    false
}

fn collect_expanded_paths(nodes: &[FileTreeNode], set: &mut std::collections::HashSet<PathBuf>) {
    for n in nodes {
        if n.is_dir {
            if n.is_expanded {
                set.insert(n.path.clone());
            }
            collect_expanded_paths(&n.children, set);
        }
    }
}

/// Formats an optional file modification time as `YYYY/MM/DD HH:mm`.
///
/// Pure function taking `mtime: Option<SystemTime>` and `now: SystemTime`:
/// - Returns `None` if `mtime` is `None` or precedes `UNIX_EPOCH` (pre-epoch fallback).
/// - Clamps future timestamps (`time > now`) to `now` to gracefully handle clock skew.
/// - Uses native POSIX `localtime_r` on Unix with a deterministic civil calendar algorithm fallback.
pub fn format_mtime(mtime: Option<std::time::SystemTime>, now: std::time::SystemTime) -> Option<String> {
    use std::time::UNIX_EPOCH;
    let time = mtime?;
    let effective_time = if time > now { now } else { time };
    let dur = effective_time.duration_since(UNIX_EPOCH).ok()?;
    let secs = dur.as_secs() as i64;

    #[cfg(unix)]
    {
        #[repr(C)]
        struct Tm {
            tm_sec: i32,
            tm_min: i32,
            tm_hour: i32,
            tm_mday: i32,
            tm_mon: i32,
            tm_year: i32,
            tm_wday: i32,
            tm_yday: i32,
            tm_isdst: i32,
            tm_gmtoff: i64,
            tm_zone: *const std::os::raw::c_char,
        }

        extern "C" {
            fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
        }

        let mut tm = std::mem::MaybeUninit::<Tm>::zeroed();
        let res = unsafe { localtime_r(&secs, tm.as_mut_ptr()) };
        if !res.is_null() {
            let tm = unsafe { tm.assume_init() };
            return Some(format!(
                "{:04}/{:02}/{:02} {:02}:{:02}",
                tm.tm_year + 1900,
                tm.tm_mon + 1,
                tm.tm_mday,
                tm.tm_hour,
                tm.tm_min
            ));
        }
    }

    let s = secs.max(0) as u64;
    let min = ((s / 60) % 60) as u32;
    let hour = ((s / 3600) % 24) as u32;
    let days = (s / 86400) as i64;

    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    Some(format!(
        "{:04}/{:02}/{:02} {:02}:{:02}",
        y, m, d, hour, min
    ))
}

pub fn scan_root_layer(
    dir: &Path,
    depth: usize,
    budget: &mut ScanBudget,
    expanded_paths: &HashSet<PathBuf>,
    max_depth: usize,
) -> (Vec<FileTreeNode>, bool) {
    if depth > max_depth {
        return (Vec::new(), false);
    }
    let read_res = fs::read_dir(dir);
    let entries = match read_res {
        Ok(e) => e,
        Err(_) => return (Vec::new(), false),
    };

    let mut dir_entries = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
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
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        dir_entries.push((p, name, file_type));
    }

    dir_entries.sort_by(|a, b| {
        match (a.2.is_dir(), b.2.is_dir()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.1.to_lowercase().cmp(&b.1.to_lowercase()),
        }
    });

    let mut nodes = Vec::new();
    let mut truncated = false;

    for (p, name, file_type) in dir_entries {
        if file_type.is_dir() {
            if !budget.consume_entry() {
                truncated = true;
                break;
            }
            let is_expanded = expanded_paths.contains(&p);
            let (children, child_truncated) = if is_expanded && depth < max_depth {
                scan_root_layer(&p, depth + 1, budget, expanded_paths, max_depth)
            } else {
                (Vec::new(), false)
            };
            if child_truncated {
                truncated = true;
            }
            nodes.push(FileTreeNode {
                path: p,
                name,
                is_dir: true,
                is_expanded,
                children,
                mtime: None,
            });
        } else if file_type.is_file() {
            if zmark_core::file_ops::is_supported_text(&p) {
                if !budget.consume_file() {
                    truncated = true;
                    break;
                }
                let raw_mtime = fs::metadata(&p)
                    .ok()
                    .and_then(|m| m.modified().ok());
                let mtime = format_mtime(raw_mtime, std::time::SystemTime::now());
                nodes.push(FileTreeNode {
                    path: p,
                    name,
                    is_dir: false,
                    is_expanded: false,
                    children: Vec::new(),
                    mtime,
                });
            } else {
                // Ordinary non-markdown file: consumes entry quota
                if !budget.consume_entry() {
                    truncated = true;
                    break;
                }
            }
        }
    }

    (nodes, truncated)
}

#[cfg(test)]
mod background_search_decoupling_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_prepare_search_job_lightweight_no_tree_clone() {
        let doc = Document::new_scratch();
        let mut state = AppState::new_isolated(doc);
        state.workspace_roots.push(WorkspaceRoot {
            path: PathBuf::from("/mock/root"),
            tree: vec![FileTreeNode {
                path: PathBuf::from("/mock/root/huge_file.md"),
                name: "huge_file.md".to_string(),
                is_dir: false,
                is_expanded: false,
                children: Vec::new(),
                mtime: None,
            }],
            is_expanded: true,
            is_missing: false,
            truncated: false,
            entries_used: 1,
            files_used: 1,
            deferred: false,
        });

        state.set_search_query("huge");
        let job = state.prepare_search_job().expect("job should be prepared");
        assert_eq!(job.roots.len(), 1);
        assert_eq!(job.roots[0].path, PathBuf::from("/mock/root"));
        assert_eq!(job.roots[0].entries_used, 1);
        assert_eq!(job.roots[0].files_used, 1);
    }

    #[test]
    fn test_apply_search_result_preserves_concurrent_user_mutations() {
        let tmp = tempdir().unwrap();
        let root_dir = tmp.path().join("workspace");
        fs::create_dir_all(&root_dir).unwrap();
        let initial_file = root_dir.join("initial.md");
        fs::write(&initial_file, "# Initial").unwrap();

        let doc = Document::new_scratch();
        let mut state = AppState::new_isolated(doc);
        state.add_workspace_root(root_dir.clone()).unwrap();

        // 1. User starts search
        state.set_search_query("initial");
        let job = state.prepare_search_job().unwrap();

        // 2. Background search executes
        let handle = std::thread::spawn(move || job.execute());
        let result = handle.join().unwrap();

        // 3. Concurrently while search was executing, user added a new file to the workspace tree
        let user_new_path = root_dir.join("user_concurrent.md");
        state.workspace_roots[0].tree.push(FileTreeNode {
            path: user_new_path.clone(),
            name: "user_concurrent.md".to_string(),
            is_dir: false,
            is_expanded: false,
            children: Vec::new(),
            mtime: None,
        });

        // 4. Result arrives and is applied
        let applied = state.apply_search_result(result);
        assert!(applied);
        assert!(!state.search_in_progress);

        // 5. Verify user_concurrent.md was NOT overwritten or lost!
        let has_user_file = state.workspace_roots[0]
            .tree
            .iter()
            .any(|n| n.name == "user_concurrent.md");
        assert!(has_user_file, "Concurrent user file must NOT be lost when search result is applied");
    }

    #[test]
    fn test_apply_search_result_does_not_resurrect_renamed_file() {
        let tmp = tempdir().unwrap();
        let root_dir = tmp.path().join("workspace");
        fs::create_dir_all(&root_dir).unwrap();
        let file_a = root_dir.join("file_a.md");
        fs::write(&file_a, "# File A").unwrap();

        let doc = Document::new_scratch();
        let mut state = AppState::new_isolated(doc);
        state.add_workspace_root(root_dir.clone()).unwrap();

        state.set_search_query("file");
        let job = state.prepare_search_job().unwrap();
        let result = job.execute();

        // Simulate rename on disk and in state tree while search was running
        let file_b = root_dir.join("file_b.md");
        fs::rename(&file_a, &file_b).unwrap();
        state.workspace_roots[0].tree.retain(|n| n.name != "file_a.md");
        state.workspace_roots[0].tree.push(FileTreeNode {
            path: file_b.clone(),
            name: "file_b.md".to_string(),
            is_dir: false,
            is_expanded: false,
            children: Vec::new(),
            mtime: None,
        });

        let applied = state.apply_search_result(result);
        assert!(applied);

        // file_a should NOT be resurrected because it no longer exists on disk
        let has_file_a = state.workspace_roots[0].tree.iter().any(|n| n.name == "file_a.md");
        let has_file_b = state.workspace_roots[0].tree.iter().any(|n| n.name == "file_b.md");
        assert!(!has_file_a, "file_a must not be resurrected after rename");
        assert!(has_file_b, "file_b must remain in workspace tree");
    }

    #[test]
    fn test_generation_mismatch_resets_in_progress_when_no_active_query() {
        let doc = Document::new_scratch();
        let mut state = AppState::new_isolated(doc);
        let stale_result = SearchJobResult {
            generation: 999,
            query: "query".to_string(),
            roots: Vec::new(),
            scan_budget: ScanBudget::default(),
            search_truncated: false,
        };
        state.search_in_progress = true;
        let applied = state.apply_search_result(stale_result);
        assert!(!applied);
        assert!(!state.search_in_progress);
    }
}

#[cfg(test)]
mod document_store_fixture_tests {
    use super::{AppState, DocKey, OpenDoc};
    use zmark_core::document::{Document, DocumentStatus};
    use std::fs;
    use tempfile::tempdir;

#[test]
fn test_sidebar_draft_decentralization_lifecycle() {
    let dir = tempdir().unwrap();
    let initial_file = dir.path().join("initial_doc.md");
    fs::write(&initial_file, "# Initial Document\n\nExisting content.").unwrap();

    let doc = Document::from_file(initial_file.clone()).unwrap();
    let mut state = AppState::new_isolated(doc);

    // 1. Initial clean state: when user has only opened a file, open_docs has no DocKey::Scratch
    assert_eq!(
        state.open_docs.iter().any(|d| d.key.is_scratch()),
        false,
        "Initial state with opened file document must contain zero Scratch documents"
    );

    // 2. Draft creation with explicit DocKey::Scratch(0)
    let scratch_doc = Document::new_scratch();
    let scratch_zero = DocKey::Scratch(0);
    state.open_docs.push(OpenDoc {
        key: scratch_zero.clone(),
        doc: scratch_doc,
    });
    state.active = Some(scratch_zero.clone());

    // Assert scratch item is detected
    assert!(
        state.open_docs.iter().any(|d| d.key.is_scratch()),
        "Scratch item must be detected in open_docs after draft creation"
    );
    assert!(
        state.open_docs.iter().any(|d| d.key == scratch_zero),
        "DocKey::Scratch(0) must exist in open_docs"
    );

    // 3. Save as file: state.save_doc_as(&DocKey::Scratch(0), target_path)
    let target_path = dir.path().join("promoted_draft.md");
    state
        .save_doc_as(&scratch_zero, target_path.clone())
        .expect("save_doc_as should succeed for scratch document");

    // Assert DocKey::Scratch(0) is completely removed from open_docs
    assert!(
        !state.open_docs.iter().any(|d| d.key == scratch_zero),
        "DocKey::Scratch(0) must be completely removed from open_docs after save_doc_as"
    );

    // Target document exists with DocKey::Path
    assert!(
        state
            .open_docs
            .iter()
            .any(|d| d.key == DocKey::Path(target_path.clone())),
        "Target document with DocKey::Path must exist in open_docs"
    );

    // Assert draft no longer appears in draft items list
    assert_eq!(
        state.open_docs.iter().any(|d| d.key.is_scratch()),
        false,
        "No scratch documents should remain in open_docs after saving as file"
    );

    // 4. Draft closing: state.close_document_forced(&DocKey::Scratch(0))
    // Create another draft with Scratch(0) to verify forced closure
    state.open_docs.push(OpenDoc {
        key: DocKey::Scratch(0),
        doc: Document::new_scratch(),
    });
    assert!(state.open_docs.iter().any(|d| d.key == DocKey::Scratch(0)));

    state.close_document_forced(&DocKey::Scratch(0));
    assert!(
        !state.open_docs.iter().any(|d| d.key == DocKey::Scratch(0)),
        "DocKey::Scratch(0) must be removed after close_document_forced"
    );
    assert_eq!(
        state.open_docs.iter().any(|d| d.key.is_scratch()),
        false,
        "Zero-draft state must be fully restored after closing draft"
    );

    // 5. Dynamic draft lifecycle via state.focus_or_create_draft()
    state.focus_or_create_draft();
    assert!(
        state.open_docs.iter().any(|d| d.key.is_scratch()),
        "focus_or_create_draft must instantiate a scratch document"
    );
    let dynamic_scratch_key = state
        .open_docs
        .iter()
        .find(|d| d.key.is_scratch())
        .map(|d| d.key.clone())
        .expect("Dynamic scratch key must exist");

    // Re-focusing does not create duplicates (singleton draft slot)
    state.focus_or_create_draft();
    assert_eq!(
        state.open_docs.iter().filter(|d| d.key.is_scratch()).count(),
        1,
        "Focusing draft again must not spawn duplicate draft items"
    );

    // Closing the dynamic draft recovers zero-draft state
    state.close_document_forced(&dynamic_scratch_key);
    assert_eq!(
        state.open_docs.iter().any(|d| d.key.is_scratch()),
        false,
        "Closing dynamic draft must recover zero-draft state"
    );
}

#[test]
fn test_matching_across_roots_loose_and_drafts() {
    let tmp = tempdir().unwrap();
    let doc_path = tmp.path().join("main.md");
    fs::write(&doc_path, "# Main Document").unwrap();

    let doc = Document::from_file(doc_path.clone()).unwrap();
    let mut state = AppState::new_isolated(doc);

    // 1. Workspace root matching
    let root_dir = tmp.path().join("workspace");
    fs::create_dir_all(&root_dir).unwrap();
    let ws_file = root_dir.join("workspace_notes.md");
    fs::write(&ws_file, "# Notes").unwrap();
    state.add_workspace_root(root_dir).unwrap();

    state.set_search_query("workspace_notes");
    assert!(state.has_search_matches(), "Should match file in workspace root");

    // 2. Loose docs matching (external file opened)
    let ext_path = tmp.path().join("external_loose.md");
    fs::write(&ext_path, "# External").unwrap();
    let ext_doc = Document::from_file(ext_path.clone()).unwrap();
    state.open_docs.push(OpenDoc {
        key: DocKey::Path(ext_path),
        doc: ext_doc,
    });

    state.set_search_query("external_loose");
    assert!(state.has_search_matches(), "Should match loose doc");

    // 3. Draft matching: match filenames only; do NOT match draft content's first heading
    let mut scratch_doc = Document::new_scratch();
    scratch_doc.update_buffer("# My Daily Scratch\nDraft content".to_string());
    state.open_docs.push(OpenDoc {
        key: DocKey::Scratch(99),
        doc: scratch_doc,
    });

    state.set_search_query("Daily");
    assert!(
        !state.has_search_matches(),
        "Filename search must NOT match scratch draft first heading"
    );

    // A named draft is searchable by filename, never by its first heading.
    state.focus_or_create_draft();
    let draft = state.active_key().cloned().unwrap();
    state.rename_document(&draft, "Daily notes").unwrap();
    state.set_search_query("Daily notes");
    assert!(state.has_search_matches(), "Should match the draft filename");

    // 4. Non-matching query
    state.set_search_query("completely_nonexistent_token_98765");
    assert!(!state.has_search_matches(), "Should not match nonexistent file");
}

#[test]
fn background_disk_conflict_survives_switching_without_losing_local_text() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.md");
    let second = dir.path().join("second.md");
    fs::write(&first, "original").unwrap();
    fs::write(&second, "second").unwrap();
    let mut state = AppState::new_isolated(Document::from_file(first.clone()).unwrap());
    state.editor.insert_text(" local");
    state.sync_from_editor();
    let local = state.editor.to_text();
    let key = DocKey::Path(first.clone());
    state.open_file(second).unwrap();
    fs::write(&first, "external change").unwrap();
    let doc = &mut state
        .open_docs
        .iter_mut()
        .find(|doc| doc.key == key)
        .unwrap()
        .doc;
    doc.handle_disk_change(fs::read_to_string(&first).unwrap());
    assert_eq!(doc.status, DocumentStatus::ConflictOnDisk);
    state.switch_document(&key);
    assert!(state.has_conflict());
    assert_eq!(state.editor.to_text(), local);
    assert_eq!(fs::read_to_string(first).unwrap(), "external change");
}
}
