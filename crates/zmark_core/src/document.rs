use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::ast::{parse_markdown, ParsedDocument};

/// Document dirty and synchronization status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentStatus {
    Clean,
    Modified,
    ConflictOnDisk,
}

/// Represents an active document session holding text buffer, AST and path state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub path: Option<PathBuf>,
    scratch_name: Option<String>,
    pub buffer: String,
    pub status: DocumentStatus,
    pub parsed: Arc<ParsedDocument>,
    last_saved_content: String,
    parse_stale: bool,
}

impl Document {
    /// Creates an empty scratchpad document.
    pub fn new_scratch() -> Self {
        Self::new_scratch_with_text(String::new())
    }

    /// Creates a scratchpad with generated initial content that is already
    /// considered clean. The welcome page is onboarding copy, not an unsaved
    /// user edit, so closing it must not trigger the save prompt.
    pub fn new_scratch_with_text(buffer: String) -> Self {
        let parsed = Arc::new(parse_markdown(&buffer));
        Self {
            path: None,
            scratch_name: None,
            last_saved_content: buffer.clone(),
            buffer,
            status: DocumentStatus::Clean,
            parsed,
            parse_stale: false,
        }
    }

    /// Loads a document from the local filesystem.
    pub fn from_file(path: PathBuf) -> Result<Self, std::io::Error> {
        let buffer = fs::read_to_string(&path)?;
        let parsed = Arc::new(parse_markdown(&buffer));
        Ok(Self {
            path: Some(path),
            scratch_name: None,
            last_saved_content: buffer.clone(),
            buffer,
            status: DocumentStatus::Clean,
            parsed,
            parse_stale: false,
        })
    }

    /// User-friendly display name with custom localized fallback for untitled documents.
    pub fn display_name_with_fallback(&self, fallback: &str) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .or_else(|| self.scratch_name.clone())
            .unwrap_or_else(|| fallback.to_string())
    }

    /// Name a draft without creating a file; saving will choose its directory later.
    pub fn rename_scratch(&mut self, name: &str) -> std::io::Result<()> {
        let name = name.trim();
        if self.path.is_some()
            || name.is_empty()
            || name == "."
            || name == ".."
            || name.chars().any(|c| c == '/' || c == '\\' || c.is_control())
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid draft name",
            ));
        }
        let name = if name.ends_with(".md") || name.ends_with(".markdown") {
            name.to_owned()
        } else {
            format!("{name}.md")
        };
        self.scratch_name = Some(name);
        self.status = DocumentStatus::Modified;
        Ok(())
    }

    /// Updates the buffer from user editing, recalculating AST and dirty state.
    pub fn update_buffer(&mut self, new_text: String) {
        if self.buffer == new_text {
            return;
        }
        self.buffer = new_text;
        self.parsed = Arc::new(parse_markdown(&self.buffer));
        self.parse_stale = false;
        if self.status != DocumentStatus::ConflictOnDisk {
            if self.buffer == self.last_saved_content && self.scratch_name.is_none() {
                self.status = DocumentStatus::Clean;
            } else {
                self.status = DocumentStatus::Modified;
            }
        }
    }

    /// Updates the buffer without parsing; call [`Self::reparse`] to refresh the AST.
    pub fn set_buffer(&mut self, new_text: String) -> bool {
        if self.buffer == new_text {
            return false;
        }
        self.buffer = new_text;
        self.parse_stale = true;
        if self.status != DocumentStatus::ConflictOnDisk {
            if self.buffer == self.last_saved_content && self.scratch_name.is_none() {
                self.status = DocumentStatus::Clean;
            } else {
                self.status = DocumentStatus::Modified;
            }
        }
        true
    }

    /// Refreshes the AST if the buffer changed since the last parse.
    pub fn reparse(&mut self) -> bool {
        if !self.parse_stale {
            return false;
        }
        self.parsed = Arc::new(parse_markdown(&self.buffer));
        self.parse_stale = false;
        true
    }

    pub fn is_parse_stale(&self) -> bool {
        self.parse_stale
    }

    /// Releases the AST of a document that is not visible. The buffer and saved
    /// text stay intact; [`Self::reparse`] rebuilds the AST when it is shown again.
    pub fn unload_parsed(&mut self) {
        self.parsed = Arc::new(ParsedDocument::default());
        self.parse_stale = true;
    }

    /// Saves the current buffer to the existing file path.
    pub fn save(&mut self) -> Result<(), std::io::Error> {
        if let Some(ref path) = self.path {
            fs::write(path, &self.buffer)?;
            self.last_saved_content = self.buffer.clone();
            self.status = DocumentStatus::Clean;
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Cannot save untitled document without destination path",
            ))
        }
    }

    /// Saves the document to a specific path and tracks it.
    pub fn save_to(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        fs::write(&path, &self.buffer)?;
        self.path = Some(path);
        self.scratch_name = None;
        self.last_saved_content = self.buffer.clone();
        self.status = DocumentStatus::Clean;
        Ok(())
    }

    /// Handles external disk modification signals.
    pub fn handle_disk_change(&mut self, new_disk_content: String) {
        if self.buffer == new_disk_content {
            self.last_saved_content = new_disk_content;
            self.status = DocumentStatus::Clean;
            return;
        }

        match self.status {
            DocumentStatus::Clean => {
                self.buffer = new_disk_content.clone();
                self.last_saved_content = new_disk_content;
                self.parsed = Arc::new(parse_markdown(&self.buffer));
                self.parse_stale = false;
                self.status = DocumentStatus::Clean;
            }
            DocumentStatus::Modified | DocumentStatus::ConflictOnDisk => {
                self.status = DocumentStatus::ConflictOnDisk;
            }
        }
    }

    /// Resolves conflict by keeping local edits and marking state as Modified.
    pub fn resolve_conflict_keep_local(&mut self) {
        self.status = DocumentStatus::Modified;
    }

    /// Resolves conflict by reloading external disk content and discarding local edits.
    pub fn resolve_conflict_reload_disk(&mut self, disk_content: String) {
        self.buffer = disk_content.clone();
        self.last_saved_content = disk_content;
        self.parsed = Arc::new(parse_markdown(&self.buffer));
        self.parse_stale = false;
        self.status = DocumentStatus::Clean;
    }
}
