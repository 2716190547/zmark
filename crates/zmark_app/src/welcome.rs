use std::path::PathBuf;
use zmark_core::document::Document;
use zmark_core::i18n::SupportedLocale;

pub fn initial_document(path: Option<PathBuf>, locale: SupportedLocale) -> Document {
    match path {
        Some(path) => Document::from_file(path).unwrap_or_else(|_| Document::new_scratch()),
        None => {
            let modifier = if cfg!(target_os = "macos") {
                "Cmd+"
            } else {
                "Ctrl+"
            };
            let title = zmark_core::i18n::get_strings(locale).welcome_doc_title;
            Document::new_named_scratch_with_text(title, text(locale).replace("Cmd+", modifier))
        }
    }
}

/// True for the startup welcome draft the user has not edited, which may be
/// replaced by the first opened file instead of staying as an extra tab.
pub fn is_pristine(doc: &Document, locale: SupportedLocale) -> bool {
    doc.path.is_none()
        && doc.status != zmark_core::document::DocumentStatus::Modified
        && doc.buffer == initial_document(None, locale).buffer
}

/// Welcome text is chosen once when a new startup scratch document is created.
pub fn text(locale: SupportedLocale) -> &'static str {
    match locale.resolve() {
        SupportedLocale::ZhCn => include_str!("../assets/welcome/zh-CN.md"),
        SupportedLocale::ZhTw => include_str!("../assets/welcome/zh-TW.md"),
        SupportedLocale::Ja => include_str!("../assets/welcome/ja.md"),
        SupportedLocale::Ko => include_str!("../assets/welcome/ko.md"),
        SupportedLocale::Fr => include_str!("../assets/welcome/fr.md"),
        SupportedLocale::De => include_str!("../assets/welcome/de.md"),
        SupportedLocale::Es => include_str!("../assets/welcome/es.md"),
        SupportedLocale::Ru => include_str!("../assets/welcome/ru.md"),
        SupportedLocale::Pt => include_str!("../assets/welcome/pt.md"),
        SupportedLocale::En | SupportedLocale::Auto => include_str!("../assets/welcome/en.md"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_locale_has_a_welcome_document() {
        for &(locale, tag, _) in SupportedLocale::all() {
            let document = text(locale);
            assert!(document.starts_with("# "), "{tag} has no title");
            assert!(
                document.contains("```mermaid"),
                "{tag} has no diagram example"
            );
            assert!(document.contains("Cmd+O"), "{tag} has no open shortcut");
        }
        assert_ne!(text(SupportedLocale::ZhCn), text(SupportedLocale::En));
        assert_eq!(
            text(SupportedLocale::Auto),
            text(SupportedLocale::Auto.resolve())
        );
        assert_eq!(
            text(SupportedLocale::from_tag("unknown")),
            text(SupportedLocale::En)
        );
    }

    #[test]
    fn startup_uses_welcome_only_without_a_file_argument() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("existing.md");
        std::fs::write(&path, "# Existing document").unwrap();
        let welcome = initial_document(None, SupportedLocale::ZhCn);
        let modifier = if cfg!(target_os = "macos") {
            "Cmd+"
        } else {
            "Ctrl+"
        };
        assert_eq!(
            welcome.buffer,
            text(SupportedLocale::ZhCn).replace("Cmd+", modifier)
        );
        assert_eq!(
            welcome.display_name_with_fallback("Untitled"),
            zmark_core::i18n::get_strings(SupportedLocale::ZhCn).welcome_doc_title
        );
        assert!(welcome.path.is_none());
        assert_eq!(
            welcome.status,
            zmark_core::document::DocumentStatus::Clean,
            "untouched welcome page must not look like an unsaved edit"
        );
        let mut edited = welcome;
        edited.update_buffer("# changed".to_string());
        assert_eq!(
            edited.status,
            zmark_core::document::DocumentStatus::Modified,
            "editing the welcome page must mark it dirty"
        );
        let opened = initial_document(Some(path.clone()), SupportedLocale::ZhCn);
        assert_eq!(opened.buffer, "# Existing document");
        assert_eq!(opened.path.as_ref(), Some(&path));
        let missing = initial_document(
            Some(directory.path().join("missing.md")),
            SupportedLocale::ZhCn,
        );
        assert!(missing.buffer.is_empty());
        assert!(missing.path.is_none());
    }
}
