use zmark_app::views::settings::shortcut_rows;
use zmark_core::i18n::{get_strings, SupportedLocale};

#[test]
fn shortcut_guide_has_the_six_canonical_entries() {
    let strings = get_strings(SupportedLocale::En);
    let rows = shortcut_rows(strings);
    assert_eq!(rows.len(), 6);
    let expected = |key: &str| zmark_app::platform::shortcut_hint(key);
    assert_eq!(rows[0].0, expected("O"));
    assert_eq!(rows[2].0, format!("⇧{}", expected("E")));
    assert_eq!(rows[5].0, expected(","));
    if cfg!(target_os = "macos") {
        assert_eq!(rows[0].0, "⌘O");
    } else {
        assert_eq!(rows[0].0, "Ctrl+O");
    }
}
