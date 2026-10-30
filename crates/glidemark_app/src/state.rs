use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
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
    pub doc: Document,
    pub open_docs: Vec<OpenDoc>,
    pub active: Option<DocKey>,
    pub next_scratch_id: u64,
    pub pending_close: Option<PendingClose>,
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
}

impl AppState {
    pub fn new(doc: Document) -> Self {
        let preferences = Preferences::load_or_default();
        Self::new_with_preferences_at(doc, preferences, Preferences::default_config_path())
    }

    #[allow(dead_code)]
    pub fn new_isolated(doc: Document) -> Self {
        let tmp_dir = std::env::temp_dir();
        let file_name = format!(
            "glidemark_test_{}_{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        Self::new_with_preferences_at(doc, Preferences::default(), tmp_dir.join(file_name))
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
        if !crate::theme::ALL_PALETTES.iter().any(|p| p.id == preferences.palette) {
            preferences.palette = "default".to_string();
            let _ = preferences.save_to_path(&preferences_path);
        }
        let is_dark = match preferences.theme {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::Auto => false,
        };
        let theme = ThemeColors::resolve(&preferences.palette, is_dark);
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
            doc: doc.clone(),
        }];
        let active = Some(key);

        let mut state = Self {
            open_docs,
            active,
            next_scratch_id: 2,
            pending_close: None,
            workspace_roots,
            scan_budget: ScanBudget::default(),
            discarded_roots,
            preferences_path,
            doc,
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

    pub fn set_tab_size(&mut self, tab_size: usize) {
        self.preferences.tab_size = tab_size;
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

    pub fn set_locale(&mut self, locale: SupportedLocale) {
        self.preferences.locale = locale;
        let _ = self.preferences.save_to_path(&self.preferences_path);
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
        if self.active.is_none() {
            return false;
        }
        let new_text = self.editor.to_text();
        let changed = self.doc.set_buffer(new_text);
        if let Some(active_key) = &self.active {
            if let Some(od) = self.open_docs.iter_mut().find(|d| &d.key == active_key) {
                od.doc = self.doc.clone();
            }
        }
        changed
    }

    /// Parses the buffer now, ignoring the debounce tier.
    pub fn parse_now(&mut self) -> bool {
        self.doc.reparse()
    }

    pub fn parse_is_stale(&self) -> bool {
        self.doc.is_parse_stale()
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
                self.workspace_roots[idx].is_expanded = true;
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

        // Check nesting relationships (missing roots MUST participate in nesting check)
        for r in &self.workspace_roots {
            let r_cmp = resolve_cmp_path(&r.path);

            if canonical.starts_with(&r_cmp) {
                return Err(RootAddError::NestedInExisting(r.path.clone()));
            }
            if r_cmp.starts_with(&canonical) {
                return Err(RootAddError::ContainsExisting(r.path.clone()));
            }
        }

        let idx = self.workspace_roots.len();
        self.workspace_roots.push(WorkspaceRoot {
            path: canonical,
            tree: Vec::new(),
            is_expanded: true,
            is_missing: false,
            truncated: false,
            entries_used: 0,
            files_used: 0,
            deferred: false,
        });
        self.preferences.workspace_roots =
            self.workspace_roots.iter().map(|root| root.path.clone()).collect();
        if let Err(e) = self.preferences.save_to_path(&self.preferences_path) {
            // Atomic rollback
            self.workspace_roots.pop();
            self.preferences.workspace_roots =
                self.workspace_roots.iter().map(|root| root.path.clone()).collect();
            return Err(RootAddError::SaveFailed(e.to_string()));
        }
        self.refresh_root(idx);
        Ok(idx)
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
        self.scan_budget.entries_left =
            (self.scan_budget.entries_left.saturating_add(removed.entries_used))
                .min(ScanBudget::MAX_GLOBAL_ENTRIES);
        self.scan_budget.files_left =
            (self.scan_budget.files_left.saturating_add(removed.files_used))
                .min(ScanBudget::MAX_GLOBAL_FILES);
        Ok(Some(removed))
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
                doc: new_doc.clone(),
            };
        } else {
            self.open_docs.push(OpenDoc {
                key: target_key.clone(),
                doc: new_doc.clone(),
            });
        }

        self.active = Some(target_key);
        self.doc = new_doc;
        self.editor.reset_text(&self.doc.buffer);
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
            self.active = Some(key.clone());
            self.doc = self.open_docs[pos].doc.clone();
            self.editor.reset_text(&self.doc.buffer);
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
            doc: scratch.clone(),
        });
        self.active = Some(key);
        self.doc = scratch;
        self.editor.reset_text("");
    }

    pub fn plan_close(&self, key: &DocKey) -> Option<CloseDecision> {
        let od = self.open_docs.iter().find(|d| &d.key == key)?;
        let is_active = self.active.as_ref() == Some(key);
        let is_dirty = if is_active {
            self.doc.status == DocumentStatus::Modified
                || self.editor.to_text() != self.doc.buffer
        } else {
            od.doc.status == DocumentStatus::Modified
        };

        if is_dirty {
            Some(CloseDecision::PromptDirty {
                key: key.clone(),
                title: od.doc.display_name(),
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
            od.doc.status == DocumentStatus::Modified
                || (od.key.is_scratch() && !od.doc.buffer.trim().is_empty())
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
        if let Some(od) = self.open_docs.iter_mut().find(|d| &d.key == key) {
            if od.doc.path.is_some() {
                od.doc.save()?;
                if self.active.as_ref() == Some(key) {
                    self.doc = od.doc.clone();
                }
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
                self.doc = Document::new_scratch();
                self.editor.reset_text("");
            } else {
                let next_idx = if pos < self.open_docs.len() {
                    pos
                } else {
                    self.open_docs.len() - 1
                };
                let next_key = self.open_docs[next_idx].key.clone();
                let next_doc = self.open_docs[next_idx].doc.clone();
                self.active = Some(next_key.clone());
                self.doc = next_doc;
                self.editor.reset_text(&self.doc.buffer);
                if let DocKey::Path(ref p) = next_key {
                    self.expand_to_file(p);
                }
            }
        }
    }

    pub fn load_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        self.open_file(path)
    }

    pub fn rename_file(&mut self, new_name: &str) -> Result<(), std::io::Error> {
        if self.active.is_none() {
            return Ok(());
        }

        let old_path = match self.doc.path.clone() {
            Some(p) => p,
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Draft documents cannot be renamed directly; use save_doc_as",
                ));
            }
        };

        let clean_name = if new_name.ends_with(".md") || new_name.ends_with(".markdown") {
            new_name.to_string()
        } else {
            format!("{}.md", new_name)
        };

        let parent = old_path.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Parent directory not found")
        })?;
        let new_path = parent.join(&clean_name);

        if new_path == old_path {
            return Ok(());
        }

        // Prevent overwriting existing file on disk
        if new_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "A file with the target name already exists",
            ));
        }

        // Prevent collision with another open document key
        let old_key = DocKey::Path(old_path.clone());
        let new_key = DocKey::Path(new_path.clone());
        if self.open_docs.iter().any(|d| d.key != old_key && d.key == new_key) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "A document with target path is already open in workspace",
            ));
        }

        std::fs::rename(&old_path, &new_path)?;

        self.doc.path = Some(new_path.clone());
        if let Some(od) = self.open_docs.iter_mut().find(|d| d.key == old_key) {
            od.key = new_key.clone();
            od.doc = self.doc.clone();
        }
        self.active = Some(new_key);
        let old_root_idx = self.find_root_index_for_path(&old_path);
        let new_root_idx = self.find_root_index_for_path(&new_path);
        if let Some(idx) = old_root_idx {
            self.refresh_root(idx);
        }
        if let Some(idx) = new_root_idx {
            if Some(idx) != old_root_idx {
                self.refresh_root(idx);
            }
        }
        self.expand_to_file(&new_path);
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if self.active.is_none() {
            return Ok(());
        }
        self.sync_from_editor();
        self.doc.save()?;
        if let Some(active_key) = &self.active {
            if let Some(od) = self.open_docs.iter_mut().find(|d| &d.key == active_key) {
                od.doc = self.doc.clone();
            }
        }
        Ok(())
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

        // Prepare candidate document clone to attempt save atomically
        let mut candidate_doc = if self.active.as_ref() == Some(key) {
            self.doc.clone()
        } else if let Some(od) = self.open_docs.iter().find(|d| &d.key == key) {
            od.doc.clone()
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Document to save was not found",
            ));
        };

        candidate_doc.path = Some(path.clone());
        candidate_doc.save()?; // Atomic test: if write fails, self state is completely untouched!

        // Atomic commit upon save success
        if let Some(od) = self.open_docs.iter_mut().find(|d| &d.key == key) {
            od.key = new_key.clone();
            od.doc = candidate_doc.clone();
        }

        if self.active.as_ref() == Some(key) {
            self.active = Some(new_key);
            self.doc = candidate_doc;
        }

        if let Some(idx) = self.find_root_index_for_path(&path) {
            self.refresh_root(idx);
        }
        self.expand_to_file(&path);
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
        if self.active.is_none() {
            return false;
        }
        self.doc.status == DocumentStatus::Modified
    }

    pub fn has_conflict(&self) -> bool {
        if self.active.is_none() {
            return false;
        }
        self.doc.status == DocumentStatus::ConflictOnDisk
    }
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

#[allow(dead_code)]
pub fn count_tree_usage(nodes: &[FileTreeNode]) -> (usize, usize) {
    let mut entries = 0;
    let mut files = 0;
    for node in nodes {
        if node.is_dir {
            entries += 1;
            let (sub_e, sub_f) = count_tree_usage(&node.children);
            entries += sub_e;
            files += sub_f;
        } else {
            files += 1;
        }
    }
    (entries, files)
}

#[allow(dead_code)]
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
            });
        } else if file_type.is_file() {
            let is_markdown = p.extension()
                .and_then(|e| e.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
                .unwrap_or(false);

            if is_markdown {
                if !budget.consume_file() {
                    truncated = true;
                    break;
                }
                nodes.push(FileTreeNode {
                    path: p,
                    name,
                    is_dir: false,
                    is_expanded: false,
                    children: Vec::new(),
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
