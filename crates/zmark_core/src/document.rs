use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::ast::{parse_markdown, ParsedDocument};

/// Plain source files share the fenced-code highlighting pipeline without changing their text.
fn parse_document(buffer: &str, path: Option<&std::path::Path>) -> ParsedDocument {
    let Some(path) = path.filter(|path| !crate::file_ops::is_markdown(path)) else {
        return parse_markdown(buffer);
    };
    let set = crate::syntax::init_syntax_set();
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let syntax = set.find_syntax_by_extension(name)
        .or_else(|| path.extension().and_then(|s| s.to_str()).and_then(|ext| set.find_syntax_by_extension(&ext.to_ascii_lowercase())));
    let language = syntax.map(|syntax| syntax.name.as_str());
    let mut offset = 0;
    let mut blocks = Vec::new();
    let mut start = 0;
    let mut first_line = 0;
    let mut mappings = Vec::new();
    for (source_line_idx, line) in buffer.split_inclusive('\n').enumerate() {
        if !mappings.is_empty() && (offset - start + line.len() > 32 * 1024 || mappings.len() >= 32) {
            blocks.push(crate::ast::ParsedCodeBlock {
                key: crate::syntax::CodeKey::new(language, Arc::from(&buffer[start..offset])),
                raw_info: language.map(str::to_owned), fence_open_line: first_line,
                fence_close_line: None, lines: std::mem::take(&mut mappings),
                child_path: "source-file".into(), is_valid: true,
            });
            start = offset;
            first_line = source_line_idx;
        }
        let eol_len = if line.ends_with("\r\n") { 2 } else { usize::from(line.ends_with('\n')) };
        let local = offset - start;
        offset += line.len();
        mappings.push(crate::ast::CodeLineMapping {
            payload_range: local..offset - start, source_line_idx, prefix_len: 0,
            content_len: line.len() - eol_len, eol_len, is_synthetic_lf: false,
        });
    }
    blocks.push(crate::ast::ParsedCodeBlock {
        key: crate::syntax::CodeKey::new(language, Arc::from(&buffer[start..])),
        raw_info: language.map(str::to_owned), fence_open_line: first_line, fence_close_line: None,
        lines: mappings, child_path: "source-file".into(), is_valid: true,
    });
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    buffer.hash(&mut hash);
    let context = hash.finish();
    for block in &mut blocks { block.key.source_context = Some((context, block.fence_open_line)); }
    ParsedDocument { code_blocks: Arc::new(blocks), ..ParsedDocument::default() }
}

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
    scratch_name_dirty: bool,
    pub buffer: String,
    pub status: DocumentStatus,
    pub parsed: Arc<ParsedDocument>,
    last_saved_content: String,
    observed_disk_content: Option<String>,
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
            scratch_name_dirty: false,
            last_saved_content: buffer.clone(),
            observed_disk_content: None,
            buffer,
            status: DocumentStatus::Clean,
            parsed,
            parse_stale: false,
        }
    }

    /// Creates a named scratchpad (such as the welcome document) whose initial
    /// title and body start clean until edited or explicitly renamed.
    pub fn new_named_scratch_with_text(name: &str, buffer: String) -> Self {
        let mut doc = Self::new_scratch_with_text(buffer);
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            doc.scratch_name = Some(crate::file_ops::markdown_name(trimmed));
        }
        doc
    }

    /// Loads a document from the local filesystem.
    pub fn from_file(path: PathBuf) -> Result<Self, std::io::Error> {
        use std::io::Read;
        const MAX_TEXT_BYTES: u64 = 32 * 1024 * 1024;
        let file = fs::File::open(&path)?;
        if !file.metadata()?.is_file() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Text path must be a regular file"));
        }
        let mut buffer = String::new();
        file.take(MAX_TEXT_BYTES + 1).read_to_string(&mut buffer)?;
        if buffer.len() as u64 > MAX_TEXT_BYTES {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Text file exceeds 32MiB"));
        }
        if buffer.contains('\0') {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Binary files cannot be edited as text"));
        }
        let parsed = Arc::new(parse_document(&buffer, Some(&path)));
        Ok(Self {
            path: Some(path),
            scratch_name: None,
            scratch_name_dirty: false,
            last_saved_content: buffer.clone(),
            observed_disk_content: None,
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
        crate::file_ops::validate_name(name)?;
        if self.path.is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid draft name",
            ));
        }
        self.scratch_name = Some(crate::file_ops::markdown_name(name));
        self.scratch_name_dirty = true;
        self.status = DocumentStatus::Modified;
        Ok(())
    }

    /// Updates the buffer from user editing, recalculating AST and dirty state.
    pub fn update_buffer(&mut self, new_text: String) {
        if self.buffer == new_text {
            return;
        }
        self.buffer = new_text;
        self.parsed = Arc::new(parse_document(&self.buffer, self.path.as_deref()));
        self.parse_stale = false;
        if self.status != DocumentStatus::ConflictOnDisk {
            if self.buffer == self.last_saved_content && !self.scratch_name_dirty {
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
            if self.buffer == self.last_saved_content && !self.scratch_name_dirty {
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
        self.parsed = Arc::new(parse_document(&self.buffer, self.path.as_deref()));
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

    /// Checks the current disk version while keeping unsaved local edits intact.
    pub fn check_disk_change(&mut self) -> std::io::Result<bool> {
        let Some(path) = &self.path else { return Ok(false); };
        let disk = fs::read_to_string(path)?;
        if disk.contains('\0') {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Disk file is no longer text"));
        }
        let expected = self.observed_disk_content.as_ref().unwrap_or(&self.last_saved_content);
        if &disk == expected { return Ok(false); }
        let reload = self.status == DocumentStatus::Clean && disk != self.buffer;
        self.handle_disk_change(disk);
        Ok(reload)
    }

    /// Save must not replace or recreate an external version it did not validate.
    pub fn save(&mut self) -> Result<(), std::io::Error> {
        let path = self.path.clone().ok_or_else(|| std::io::Error::new(
            std::io::ErrorKind::NotFound, "Cannot save untitled document without destination path",
        ))?;
        if self.check_disk_change()? { return Ok(()); }
        if self.status == DocumentStatus::ConflictOnDisk {
            return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "File changed on disk; resolve the conflict before saving"));
        }
        // Save must never silently recreate a deleted file.
        crate::file_ops::atomic_write(&path.canonicalize()?, self.buffer.as_bytes(), true)?;
        self.last_saved_content.clone_from(&self.buffer);
        self.observed_disk_content = None;
        self.status = DocumentStatus::Clean;
        Ok(())
    }

    /// Updates draft identity only after the destination was written completely.
    pub fn save_to(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        if self.path.as_ref().is_some_and(|old| old == &path || old.canonicalize().ok().zip(path.canonicalize().ok()).is_some_and(|(a, b)| a == b)) {
            return self.save();
        }
        crate::file_ops::atomic_write(&path, self.buffer.as_bytes(), false)?;
        self.path = Some(path);
        self.scratch_name = None;
        self.scratch_name_dirty = false;
        self.last_saved_content.clone_from(&self.buffer);
        self.observed_disk_content = None;
        self.status = DocumentStatus::Clean;
        Ok(())
    }

    /// Handles external disk modification signals.
    pub fn handle_disk_change(&mut self, new_disk_content: String) {
        self.observed_disk_content = Some(new_disk_content.clone());
        if self.buffer == new_disk_content {
            self.last_saved_content = new_disk_content;
            self.observed_disk_content = None;
            self.status = DocumentStatus::Clean;
            return;
        }

        match self.status {
            DocumentStatus::Clean => {
                self.buffer = new_disk_content.clone();
                self.last_saved_content = new_disk_content;
                self.observed_disk_content = None;
                self.parsed = Arc::new(parse_document(&self.buffer, self.path.as_deref()));
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
        self.observed_disk_content = None;
        self.parsed = Arc::new(parse_document(&self.buffer, self.path.as_deref()));
        self.parse_stale = false;
        self.status = DocumentStatus::Clean;
    }
}
