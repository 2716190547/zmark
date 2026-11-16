use std::io;
use std::path::{Path, PathBuf};

/// File types supported by the sidebar and external file drops.
pub fn is_supported_text(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_ascii_lowercase();
    if matches!(name.as_str(), "dockerfile" | "makefile" | "justfile" | ".gitignore" | ".gitattributes" | ".editorconfig" | ".env") {
        return true;
    }
    if path.extension().is_none() { return true; }
    matches!(path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str(),
        "md" | "markdown" | "txt" | "text" | "rst" | "adoc" | "properties" | "rs" | "py" | "pyi" | "js" | "jsx" | "ts" | "tsx" |
        "c" | "h" | "cpp" | "hpp" | "cc" | "cs" | "go" | "java" | "kt" | "kts" | "swift" |
        "rb" | "php" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "sql" | "lua" | "r" |
        "html" | "htm" | "css" | "scss" | "sass" | "less" | "vue" | "svelte" |
        "json" | "jsonc" | "toml" | "yaml" | "yml" | "xml" | "ini" | "conf" | "cfg" |
        "csv" | "tsv" | "log" | "env")
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e|
        e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}

pub fn validate_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_control())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid file name",
        ));
    }
    #[cfg(windows)]
    {
        let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'));
        if reserved || name.ends_with(['.', ' ']) || name.chars().any(|c| "<>:\"|?*".contains(c)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid Windows file name",
            ));
        }
    }
    Ok(())
}

pub fn markdown_name(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".md")
        || name.to_ascii_lowercase().ends_with(".markdown")
    {
        name.to_owned()
    } else {
        format!("{name}.md")
    }
}

pub fn remap_path(path: &Path, old: &Path, new: &Path) -> Option<PathBuf> {
    path.strip_prefix(old).ok().map(|suffix| {
        if suffix.as_os_str().is_empty() {
            new.to_path_buf()
        } else {
            new.join(suffix)
        }
    })
}

/// Write completely before replacing the destination; a failed write leaves the old file intact.
pub(crate) fn atomic_write(path: &Path, contents: &[u8], require_existing: bool) -> io::Result<()> {
    use std::io::Write;
    let target = match std::fs::symlink_metadata(path) {
        Ok(_) => path.canonicalize()?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => path.to_path_buf(),
        Err(error) => return Err(error),
    };
    let metadata = match std::fs::metadata(&target) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.permissions().readonly() {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Destination is not a writable file"));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if metadata.nlink() > 1 {
                    return Err(io::Error::new(io::ErrorKind::Unsupported, "Hard-linked files require Save As to a new path"));
                }
            }
            Some(metadata)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound && !require_existing => None,
        Err(error) => return Err(error),
    };
    let parent = target.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    if let Some(metadata) = &metadata { temporary.as_file().set_permissions(metadata.permissions())?; }
    temporary.write_all(contents)?;
    temporary.as_file().sync_all()?;
    match (&metadata, std::fs::metadata(&target)) {
        (Some(_expected), Ok(_current)) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if _current.dev() != _expected.dev() || _current.ino() != _expected.ino() || _current.nlink() > 1 {
                    return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Destination changed while saving"));
                }
            }
        }
        (Some(_), Err(error)) => return Err(error),
        (None, _) => {}
    }
    if metadata.is_some() {
        temporary.persist(&target).map_err(|error| error.error)?;
    } else {
        // A newly created destination must not overwrite another process's file.
        temporary.persist_noclobber(&target).map_err(|error| error.error)?;
    }
    Ok(())
}

/// The OS checks destination occupancy in the rename itself, not in a racy exists() check.
pub fn rename_noreplace(old: &Path, new: &Path) -> io::Result<()> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use std::os::unix::ffi::OsStrExt;
        let old = std::ffi::CString::new(old.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"))?;
        let new = std::ffi::CString::new(new.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"))?;
        #[cfg(target_os = "macos")]
        let result = {
            extern "C" {
                fn renamex_np(
                    old: *const std::ffi::c_char,
                    new: *const std::ffi::c_char,
                    flags: u32,
                ) -> i32;
            }
            unsafe { renamex_np(old.as_ptr(), new.as_ptr(), 0x4) } // RENAME_EXCL
        };
        #[cfg(target_os = "linux")]
        let result = {
            extern "C" {
                fn renameat2(
                    old_fd: i32,
                    old: *const std::ffi::c_char,
                    new_fd: i32,
                    new: *const std::ffi::c_char,
                    flags: u32,
                ) -> i32;
            }
            unsafe { renameat2(-100, old.as_ptr(), -100, new.as_ptr(), 1) } // AT_FDCWD, RENAME_NOREPLACE
        };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let old: Vec<u16> = old.as_os_str().encode_wide().chain(Some(0)).collect();
        let new: Vec<u16> = new.as_os_str().encode_wide().chain(Some(0)).collect();
        if old[..old.len() - 1].contains(&0) || new[..new.len() - 1].contains(&0) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "NUL in path"));
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn MoveFileExW(old: *const u16, new: *const u16, flags: u32) -> i32;
        }
        if unsafe { MoveFileExW(old.as_ptr(), new.as_ptr(), 0) } != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    {
        let _ = (old, new);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Exclusive rename unsupported on this platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_save_requires_a_destination_but_save_as_can_create_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.md");
        assert!(atomic_write(&path, b"local", true).is_err());
        assert!(!path.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        atomic_write(&path, b"local", false).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"local");
    }

    #[test]
    fn exclusive_rename_keeps_existing_files_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.md");
        let target = dir.path().join("target.md");
        std::fs::write(&source, "source").unwrap();
        std::fs::write(&target, "target").unwrap();
        assert!(rename_noreplace(&source, &target).is_err());
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "source");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "target");
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        assert!(rename_noreplace(&a, &b).is_err());
        assert!(a.is_dir() && b.is_dir());
        std::fs::remove_file(&target).unwrap();
        rename_noreplace(&source, &target).unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "source");
    }

    #[test]
    fn names_and_component_prefixes_are_validated() {
        for name in ["", ".", "..", "../escape", "a/b", "a\\b", "nul\0"] {
            assert!(validate_name(name).is_err(), "{name:?}");
        }
        validate_name("中文😀").unwrap();
        assert_eq!(markdown_name("Notes.MD"), "Notes.MD");
        assert_eq!(
            remap_path(Path::new("a/b/doc.md"), Path::new("a/b"), Path::new("a/c")),
            Some(PathBuf::from("a/c/doc.md"))
        );
        assert_eq!(
            remap_path(Path::new("a/b2/doc.md"), Path::new("a/b"), Path::new("a/c")),
            None
        );
        assert_eq!(
            remap_path(Path::new("a.md"), Path::new("a.md"), Path::new("b.md"))
                .unwrap()
                .as_os_str(),
            Path::new("b.md").as_os_str()
        );
        #[cfg(windows)]
        for name in ["CON", "NUL.md", "COM1.md", "LPT9", "a:", "a.", "a ", "a?b"] {
            assert!(validate_name(name).is_err(), "{name:?}");
        }
    }
}
