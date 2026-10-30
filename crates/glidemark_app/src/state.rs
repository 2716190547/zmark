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
pub struct VisibleTreeRow {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub depth: usize,
}

pub struct AppState {
    pub doc: Document,
    pub open_docs: Vec<OpenDoc>,
    pub active: Option<DocKey>,
    pub next_scratch_id: u64,
    pub pending_close: Option<PendingClose>,
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
    pub file_tree: Vec<FileTreeNode>,
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

        let workspace_folder = doc.path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf());
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
            workspace_folder,
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
            file_tree: Vec::new(),
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
        self.refresh_workspace_tree();
    }

    pub fn refresh_workspace_tree(&mut self) {
        let target_dir = if let Some(ref folder) = self.workspace_folder {
            Some(folder.clone())
        } else if let Some(ref path) = self.doc.path {
            path.parent().map(|p| p.to_path_buf())
        } else {
            None
        };

        if let Some(dir) = target_dir {
            let mut expanded_paths = std::collections::HashSet::new();
            collect_expanded_paths(&self.file_tree, &mut expanded_paths);
            let mut scanned = 0;
            self.file_tree = scan_tree_dir(&dir, 0, &mut scanned, &expanded_paths);
        } else {
            self.file_tree.clear();
        }
    }

    pub fn toggle_tree_folder(&mut self, target: &std::path::Path) {
        fn toggle_node(nodes: &mut [FileTreeNode], target: &std::path::Path) -> bool {
            for node in nodes.iter_mut() {
                if node.path == target {
                    if node.is_dir {
                        node.is_expanded = !node.is_expanded;
                        if node.is_expanded && node.children.is_empty() {
                            let mut scanned = 0;
                            let empty_set = std::collections::HashSet::new();
                            node.children = scan_tree_dir(&node.path, 0, &mut scanned, &empty_set);
                        }
                    }
                    return true;
                }
                if node.is_dir && toggle_node(&mut node.children, target) {
                    return true;
                }
            }
            false
        }
        toggle_node(&mut self.file_tree, target);
    }

    pub fn expand_to_file(&mut self, file_path: &std::path::Path) {
        fn expand_ancestors(nodes: &mut [FileTreeNode], target: &std::path::Path) -> bool {
            let mut found = false;
            for node in nodes.iter_mut() {
                if node.is_dir && target.starts_with(&node.path) {
                    node.is_expanded = true;
                    if node.children.is_empty() {
                        let mut scanned = 0;
                        let empty_set = std::collections::HashSet::new();
                        node.children = scan_tree_dir(&node.path, 0, &mut scanned, &empty_set);
                    }
                    expand_ancestors(&mut node.children, target);
                    found = true;
                }
            }
            found
        }
        expand_ancestors(&mut self.file_tree, file_path);
    }

    pub fn flatten_tree(&self) -> Vec<VisibleTreeRow> {
        let mut rows = Vec::new();
        fn collect(nodes: &[FileTreeNode], depth: usize, rows: &mut Vec<VisibleTreeRow>) {
            for node in nodes {
                rows.push(VisibleTreeRow {
                    path: node.path.clone(),
                    name: node.name.clone(),
                    is_dir: node.is_dir,
                    is_expanded: node.is_expanded,
                    depth,
                });
                if node.is_dir && node.is_expanded {
                    collect(&node.children, depth + 1, rows);
                }
            }
        }
        collect(&self.file_tree, 0, &mut rows);
        rows
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

        // Automatic workspace root fallback: if outside current workspace, set parent as root
        if let Some(parent) = path.parent() {
            let should_switch = match &self.workspace_folder {
                Some(folder) => !path.starts_with(folder),
                None => true,
            };
            if should_switch {
                self.workspace_folder = Some(parent.to_path_buf());
            }
        }

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
        self.refresh_sibling_files();
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
            self.refresh_sibling_files();
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
        self.refresh_sibling_files();
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

    /// Evaluates window exit strategy:
    /// - If any unpathed draft is modified, switch to it, trigger close prompt, and block exit.
    /// - Otherwise, auto-save all modified documents with paths and allow exit.
    pub fn handle_window_should_close(&mut self) -> bool {
        self.sync_from_editor();

        let dirty_unpathed_scratch = self.open_docs.iter().find(|d| {
            d.key.is_scratch()
                && (d.doc.status == DocumentStatus::Modified || !d.doc.buffer.trim().is_empty())
        });

        if let Some(scratch) = dirty_unpathed_scratch {
            let key = scratch.key.clone();
            self.request_close(&key);
            return false;
        }

        for od in self.open_docs.iter_mut() {
            if od.doc.path.is_some() && od.doc.status == DocumentStatus::Modified {
                let _ = od.doc.save();
            }
        }

        true
    }

    pub fn cancel_close(&mut self) {
        self.pending_close = None;
    }

    pub fn confirm_close_discard(&mut self, key: &DocKey) {
        self.pending_close = None;
        self.close_document_forced(key);
    }

    pub fn confirm_close_save(&mut self, key: &DocKey) -> Result<bool, std::io::Error> {
        self.pending_close = None;
        if let Some(od) = self.open_docs.iter_mut().find(|d| &d.key == key) {
            if od.doc.path.is_some() {
                od.doc.save()?;
                if self.active.as_ref() == Some(key) {
                    self.doc = od.doc.clone();
                }
                self.close_document_forced(key);
                return Ok(true);
            } else {
                // Scratch needs Save As path from UI
                return Ok(false);
            }
        }
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
        self.refresh_sibling_files();
    }

    pub fn load_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        self.open_file(path)
    }

    pub fn rename_file(&mut self, new_name: &str) -> Result<(), std::io::Error> {
        if self.active.is_none() {
            return Ok(());
        }
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
                    let old_key = DocKey::Path(old_path);
                    let new_key = DocKey::Path(new_path.clone());

                    self.doc.path = Some(new_path);
                    if let Some(od) = self.open_docs.iter_mut().find(|d| d.key == old_key) {
                        od.key = new_key.clone();
                        od.doc = self.doc.clone();
                    }
                    self.active = Some(new_key);
                    self.refresh_sibling_files();
                }
            }
        }
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

        if let Some(od) = self.open_docs.iter_mut().find(|d| &d.key == key) {
            od.doc.path = Some(path.clone());
            od.doc.save()?;
            od.key = new_key.clone();
        }

        if self.active.as_ref() == Some(key) {
            self.active = Some(new_key);
            self.doc.path = Some(path.clone());
            self.doc.save()?;
        }

        self.refresh_sibling_files();
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

fn scan_tree_dir(
    dir: &std::path::Path,
    depth: usize,
    scanned: &mut usize,
    expanded_set: &std::collections::HashSet<PathBuf>,
) -> Vec<FileTreeNode> {
    if depth > 4 || *scanned >= 1000 {
        return Vec::new();
    }
    let mut nodes = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            *scanned += 1;
            if *scanned >= 1000 {
                break;
            }
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let p = entry.path();
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
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

            if file_type.is_dir() {
                let is_expanded = expanded_set.contains(&p);
                let children = if is_expanded || depth < 1 {
                    scan_tree_dir(&p, depth + 1, scanned, expanded_set)
                } else {
                    Vec::new()
                };
                nodes.push(FileTreeNode {
                    path: p,
                    name,
                    is_dir: true,
                    is_expanded,
                    children,
                });
            } else if file_type.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") {
                        nodes.push(FileTreeNode {
                            path: p,
                            name,
                            is_dir: false,
                            is_expanded: false,
                            children: Vec::new(),
                        });
                    }
                }
            }
        }
    }

    nodes.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    nodes
}
