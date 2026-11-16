use zmark_app::views::settings::shortcut_rows;
use zmark_core::i18n::{get_strings, SupportedLocale};

#[test]
fn source_session_keeps_context_across_scroll_and_does_not_reuse_other_documents() {
    use zmark_app::syntax::SourceSyntaxSession;
    use zmark_core::syntax::{CodeKey, ParseResult, SyntaxRole};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("context.rs");
    let source = format!("/*\n{}*/\nfn tail() {{}}", "inside comment\n".repeat(90));
    std::fs::write(&path, &source).unwrap();
    let doc = zmark_core::document::Document::from_file(path.clone()).unwrap();
    let blocks = &doc.parsed.code_blocks;
    let mut session = SourceSyntaxSession::default();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    for block in blocks.iter() {
        let result = session.parse_visible(blocks, &[block.key.clone()], &cancel);
        assert_eq!(result.len(), 1);
        let ParseResult::Ready(spans) = &result[0].1 else { panic!("visible chunk should highlight"); };
        assert_eq!(spans[0].role, SyntaxRole::Comment);
    }
    // Backward scrolling can rebuild grammar, while identical local bytes in another document cannot share cached tokens.
    let back = session.parse_visible(blocks, &[blocks[1].key.clone()], &cancel);
    assert!(back[0].1.is_ready());
    std::fs::write(&path, source.replacen("/*", "//", 1)).unwrap();
    let other = zmark_core::document::Document::from_file(path).unwrap();
    assert_ne!(blocks[1].key, other.parsed.code_blocks[1].key);
    let other_result = session.parse_visible(&other.parsed.code_blocks, &[other.parsed.code_blocks[1].key.clone()], &cancel);
    assert!(other_result[0].1.is_ready());
    assert_eq!(CodeKey::new(Some("Rust"), "x".into()), CodeKey::new(Some("Rust"), "x".into()));
}

#[test]
fn explicit_open_accepts_utf8_text_and_unknown_extensions_but_not_binary() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = zmark_app::state::AppState::new_isolated(zmark_core::document::Document::new_scratch());
    for name in ["notes.text", "LICENSE", "notes.custom"] {
        let path = dir.path().join(name);
        std::fs::write(&path, "中文🙂\r\ntext").unwrap();
        state.open_file(path.clone()).unwrap();
        assert_eq!(state.editor.to_text(), "中文🙂\r\ntext");
        state.editor.set_cursor(1, 4);
        state.editor.insert_text("!");
        state.save().unwrap();
        assert!(std::fs::read_to_string(path).unwrap().ends_with('!'));
    }
    for data in [b"binary\0data".as_slice(), &[255, 254, 253]] {
        let path = dir.path().join("bad.bin");
        std::fs::write(&path, data).unwrap();
        let before = state.editor.clone();
        assert!(state.open_file(path).is_err());
        assert_eq!(state.editor, before);
    }
}

#[gpui::test]
fn settings_click_persists_only_the_workspace_owned_preferences(cx: &mut gpui::TestAppContext) {
    use gpui::{AppContext, Modifiers};
    use zmark_app::{state::{AppState, SettingsTab}, views::{settings::SettingsWindowView, workspace::WorkspaceView}};
    use zmark_core::{config::{Preferences, ThemeMode}, document::Document};
    let mut fixture = None;
    let (_root, cx) = cx.add_window_view(|window, cx| {
        let workspace = cx.new(|cx| WorkspaceView::new(AppState::new_isolated(Document::new_scratch()), cx));
        let state = &workspace.read(cx).state;
        let preferences = state.preferences.clone();
        let theme = state.theme.clone();
        let strings = state.strings;
        let settings = cx.new(|cx| SettingsWindowView::new(workspace.downgrade(), SettingsTab::Appearance, preferences, theme, strings, cx));
        let root = gpui_component::Root::new(settings.clone(), window, cx);
        fixture = Some((workspace, settings, strings.theme_dark));
        root
    });
    let (workspace, settings, label) = fixture.unwrap();
    let path = workspace.read_with(cx, |ws, _| ws.state.preferences_path.clone());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let button = cx.debug_bounds(label).expect("real theme button").center();
    cx.simulate_click(button, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(workspace.read_with(cx, |ws, _| ws.state.preferences.theme), ThemeMode::Dark);
    assert_eq!(settings.read_with(cx, |settings, _| settings.preferences.theme), ThemeMode::Dark);
    assert_eq!(Preferences::load_or_default_from_path(&path).theme, ThemeMode::Dark);
}

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

#[cfg(target_os = "macos")]
#[test]
fn native_source_worker_keeps_multiline_state_across_calling_threads() {
    use std::sync::{Arc, atomic::AtomicBool};
    use zmark_app::syntax::SourceSyntaxWorker;
    use zmark_core::syntax::{ParseResult, SyntaxRole};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("worker.rs");
    std::fs::write(
        &path,
        format!("/*\n{}*/\nfn tail() {{}}", "inside comment\n".repeat(90)),
    )
    .unwrap();
    let doc = zmark_core::document::Document::from_file(path).unwrap();
    let worker = Arc::new(SourceSyntaxWorker::default());
    for block in doc.parsed.code_blocks.iter() {
        let worker = worker.clone();
        let blocks = doc.parsed.code_blocks.clone();
        let wanted = vec![block.key.clone()];
        let results = std::thread::spawn(move || {
            worker.parse_visible(blocks, wanted, Arc::new(AtomicBool::new(false)))
        })
        .join()
        .unwrap();
        let ParseResult::Ready(spans) = &results[0].1 else {
            panic!("native worker must return highlighted spans");
        };
        assert_eq!(spans[0].role, SyntaxRole::Comment);
    }
    let cancelled = worker.parse_visible(
        doc.parsed.code_blocks.clone(),
        vec![doc.parsed.code_blocks[0].key.clone()],
        Arc::new(AtomicBool::new(true)),
    );
    assert!(
        cancelled.is_empty(),
        "cancelled work must not produce stale tokens"
    );
}
