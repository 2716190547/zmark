use std::fs;
use std::path::PathBuf;
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
    pub buffer: String,
    pub status: DocumentStatus,
    pub parsed: ParsedDocument,
    last_saved_content: String,
}

impl Document {
    /// Creates an empty scratchpad document.
    pub fn new_scratch() -> Self {
        let buffer = String::new();
        let parsed = parse_markdown(&buffer);
        Self {
            path: None,
            buffer,
            status: DocumentStatus::Clean,
            parsed,
            last_saved_content: String::new(),
        }
    }

    /// Loads a document from the local filesystem.
    pub fn from_file(path: PathBuf) -> Result<Self, std::io::Error> {
        let buffer = fs::read_to_string(&path)?;
        let parsed = parse_markdown(&buffer);
        Ok(Self {
            path: Some(path),
            last_saved_content: buffer.clone(),
            buffer,
            status: DocumentStatus::Clean,
            parsed,
        })
    }

    /// Updates the buffer from user editing, recalculating AST and dirty state.
    pub fn update_buffer(&mut self, new_text: String) {
        if self.buffer == new_text {
            return;
        }
        self.buffer = new_text;
        self.parsed = parse_markdown(&self.buffer);
        if self.status != DocumentStatus::ConflictOnDisk {
            if self.buffer == self.last_saved_content {
                self.status = DocumentStatus::Clean;
            } else {
                self.status = DocumentStatus::Modified;
            }
        }
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
                self.parsed = parse_markdown(&self.buffer);
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
        self.parsed = parse_markdown(&self.buffer);
        self.status = DocumentStatus::Clean;
    }
}
