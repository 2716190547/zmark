use glidemark_app::state::{AppState, DocKey, OpenDoc, ViewMode};
use glidemark_app::theme::ThemeColors;
use glidemark_app::views::workspace::{adaptive_header_padding, mode_tab_colors};
use glidemark_core::document::Document;
use glidemark_core::i18n::{get_strings, SupportedLocale};
use gpui::rgb;
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
        doc: scratch_doc.clone(),
    });
    state.active = Some(scratch_zero.clone());
    state.doc = scratch_doc;

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
fn test_header_view_mode_states_and_warm_pill_palette() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

    // Initial default view mode is Split
    assert_eq!(state.view_mode, ViewMode::Split);

    // Transition to Source mode
    state.set_view_mode(ViewMode::Source);
    assert_eq!(state.view_mode, ViewMode::Source);

    // Transition to Preview mode
    state.set_view_mode(ViewMode::Preview);
    assert_eq!(state.view_mode, ViewMode::Preview);

    // Transition back to Split mode
    state.set_view_mode(ViewMode::Split);
    assert_eq!(state.view_mode, ViewMode::Split);

    // ThemeColors warm stone pill palette verification
    let light_theme = ThemeColors::resolve("default", false);
    let light_bg = light_theme.warm_pill_bg();
    let light_text = light_theme.warm_pill_text();

    // Verify exact expected warm stone colors (#EFF1F4 light bg, #1F2328 light text)
    assert_eq!(light_bg, rgb(0xeff1f4).into());
    assert_eq!(light_text, rgb(0x1f2328).into());
    assert_eq!(light_bg.a, 1.0, "Light warm pill bg must be fully opaque");
    assert_eq!(light_text.a, 1.0, "Light warm pill text must be fully opaque");
    assert_ne!(
        light_bg, light_text,
        "Warm pill background and text must be distinguishable in light mode"
    );
    assert!(
        light_bg.l > light_text.l,
        "Light mode warm stone background must have higher lightness than text"
    );

    let dark_theme = ThemeColors::resolve("default", true);
    let dark_bg = dark_theme.warm_pill_bg();
    let dark_text = dark_theme.warm_pill_text();

    // Verify exact expected warm stone colors (#272B33 dark bg, #F0F2F5 dark text)
    assert_eq!(dark_bg, rgb(0x272b33).into());
    assert_eq!(dark_text, rgb(0xf0f2f5).into());
    assert_eq!(dark_bg.a, 1.0, "Dark warm pill bg must be fully opaque");
    assert_eq!(dark_text.a, 1.0, "Dark warm pill text must be fully opaque");
    assert_ne!(
        dark_bg, dark_text,
        "Warm pill background and text must be distinguishable in dark mode"
    );
    assert!(
        dark_bg.l < dark_text.l,
        "Dark mode warm stone background must have lower lightness than text"
    );

    // Cross-mode distinctness
    assert_ne!(light_bg, dark_bg, "Light and dark pill backgrounds must differ");
    assert_ne!(light_text, dark_text, "Light and dark pill texts must differ");

    // Verify consistency across all registered palettes
    for palette in glidemark_app::theme::ALL_PALETTES {
        let l_theme = ThemeColors::resolve(palette.id, false);
        let d_theme = ThemeColors::resolve(palette.id, true);

        assert_ne!(l_theme.warm_pill_bg(), l_theme.warm_pill_text());
        assert_ne!(d_theme.warm_pill_bg(), d_theme.warm_pill_text());
        assert_eq!(l_theme.warm_pill_bg().a, 1.0);
        assert_eq!(d_theme.warm_pill_bg().a, 1.0);
    }

    // Direct verification of mode_tab_colors production function (P1 requirement)
    for is_dark in [false, true] {
        let theme = ThemeColors::default_palette(is_dark);

        let (active_bg, active_text) = mode_tab_colors(true, &theme);
        let (_inactive_bg, inactive_text) = mode_tab_colors(false, &theme);

        // Active background MUST be warm_pill_bg, NOT theme.surface
        assert_eq!(
            active_bg,
            theme.warm_pill_bg(),
            "Active mode tab background must equal warm_pill_bg"
        );
        assert_ne!(
            active_bg,
            theme.surface,
            "Active mode tab background must NOT be theme.surface"
        );
        assert_eq!(
            active_text,
            theme.warm_pill_text(),
            "Active mode tab text must equal warm_pill_text"
        );
        assert_eq!(
            inactive_text,
            theme.text_muted,
            "Inactive mode tab text must equal text_muted"
        );
    }
}

#[test]
fn test_document_title_display_name_and_i18n_fallback() {
    let dir = tempdir().unwrap();

    // 1. Unnamed scratch document: localized untitled_file fallback
    let scratch_doc = Document::new_scratch();

    // Chinese (Simplified)
    let strings_zh_cn = get_strings(SupportedLocale::ZhCn);
    assert_eq!(strings_zh_cn.untitled_file, "未命名");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_zh_cn.untitled_file),
        "未命名",
        "Chinese locale must return '未命名' for scratch document"
    );

    // Chinese (Traditional)
    let strings_zh_tw = get_strings(SupportedLocale::ZhTw);
    assert_eq!(strings_zh_tw.untitled_file, "未命名");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_zh_tw.untitled_file),
        "未命名"
    );

    // English
    let strings_en = get_strings(SupportedLocale::En);
    assert_eq!(strings_en.untitled_file, "Untitled");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_en.untitled_file),
        "Untitled",
        "English locale must return 'Untitled' for scratch document"
    );

    // Japanese
    let strings_ja = get_strings(SupportedLocale::Ja);
    assert_eq!(strings_ja.untitled_file, "名称未設定");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_ja.untitled_file),
        "名称未設定"
    );

    // German
    let strings_de = get_strings(SupportedLocale::De);
    assert_eq!(strings_de.untitled_file, "Ohne Titel");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_de.untitled_file),
        "Ohne Titel"
    );

    // French
    let strings_fr = get_strings(SupportedLocale::Fr);
    assert_eq!(strings_fr.untitled_file, "Sans titre");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_fr.untitled_file),
        "Sans titre"
    );

    // Spanish
    let strings_es = get_strings(SupportedLocale::Es);
    assert_eq!(strings_es.untitled_file, "Sin título");
    assert_eq!(
        scratch_doc.display_name_with_fallback(strings_es.untitled_file),
        "Sin título"
    );

    // 2. Document with concrete file path: returns file_name and NOT fallback
    let doc_file = dir.path().join("Architecture_Spec.md");
    fs::write(&doc_file, "# Architecture Spec").unwrap();

    let path_doc = Document::from_file(doc_file).unwrap();
    assert_eq!(
        path_doc.display_name_with_fallback(strings_zh_cn.untitled_file),
        "Architecture_Spec.md",
        "Concrete file path must return file_name, never masked by Chinese fallback"
    );
    assert_eq!(
        path_doc.display_name_with_fallback(strings_en.untitled_file),
        "Architecture_Spec.md",
        "Concrete file path must return file_name, never masked by English fallback"
    );
    assert_eq!(
        path_doc.display_name_with_fallback(strings_ja.untitled_file),
        "Architecture_Spec.md",
        "Concrete file path must return file_name, never masked by Japanese fallback"
    );

    // Deep nested file path
    let nested_file = dir.path().join("nested").join("subfolder").join("custom_design.markdown");
    fs::create_dir_all(nested_file.parent().unwrap()).unwrap();
    fs::write(&nested_file, "# Design").unwrap();

    let nested_doc = Document::from_file(nested_file).unwrap();
    assert_eq!(
        nested_doc.display_name_with_fallback(strings_en.untitled_file),
        "custom_design.markdown",
        "Deeply nested file path must preserve its exact file_name"
    );
}

#[test]
fn test_fullscreen_padding_adaptive_calculation() {
    // Direct assertion on production function: adaptive_header_padding(is_macos, is_fullscreen, drawer_open)

    // 1. Drawer Header (always at leading edge of window, drawer_open = false):
    // macOS non-fullscreen normal window requires 76px to leave safe room for native traffic lights
    assert_eq!(
        adaptive_header_padding(true, false, false),
        76.0,
        "macOS non-fullscreen window leading edge must have 76px left padding"
    );
    // macOS fullscreen window smoothly adapts to 12px
    assert_eq!(
        adaptive_header_padding(true, true, false),
        12.0,
        "macOS fullscreen window leading edge must adapt to 12px left padding"
    );
    // Non-macOS window leading edge always 8px
    assert_eq!(
        adaptive_header_padding(false, false, false),
        8.0,
        "Non-macOS window leading edge must have 8px left padding in normal mode"
    );
    assert_eq!(
        adaptive_header_padding(false, true, false),
        8.0,
        "Non-macOS window leading edge must have 8px left padding in fullscreen mode"
    );

    // 2. Canvas Header (contextual based on drawer open/collapsed state):
    // When drawer is open on macOS, canvas header receives standard 8px padding
    assert_eq!(
        adaptive_header_padding(true, false, true),
        8.0,
        "When drawer is open on macOS, canvas header needs only standard 8px padding"
    );
    assert_eq!(
        adaptive_header_padding(true, true, true),
        8.0,
        "When drawer is open in fullscreen on macOS, canvas header needs only standard 8px padding"
    );
    // When drawer is collapsed on macOS, canvas header absorbs the traffic light safe padding
    assert_eq!(
        adaptive_header_padding(true, false, false),
        76.0,
        "When drawer is collapsed on macOS, canvas header needs 76px padding"
    );
    assert_eq!(
        adaptive_header_padding(true, true, false),
        12.0,
        "When drawer is collapsed in fullscreen on macOS, canvas header needs 12px padding"
    );

    // On non-macOS (Windows / Linux), drawer open or collapsed always yields 8px for canvas header
    assert_eq!(adaptive_header_padding(false, false, false), 8.0);
    assert_eq!(adaptive_header_padding(false, true, false), 8.0);
    assert_eq!(adaptive_header_padding(false, false, true), 8.0);
    assert_eq!(adaptive_header_padding(false, true, true), 8.0);
}
