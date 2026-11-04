use glidemark_app::state::{AppState, ViewMode};
#[allow(unused_imports)]
use glidemark_app::views::editor::MarkdownTokenKind;
#[allow(unused_imports)]
use glidemark_app::views::preview::obs;
use glidemark_app::views::workspace::WorkspaceView;
use glidemark_core::document::Document;
use glidemark_core::syntax::{CodeKey, SyntaxRole};
use gpui::TestAppContext;
use std::sync::Arc;

#[gpui::test]
async fn test_workspace_single_cache_entry_shared_across_editor_and_preview(cx: &mut TestAppContext) {
    // Document with CRLF and list-nested code block
    let md = "# Title\r\n\r\n- item\r\n\r\n  ```rust\r\n  fn a() {}\r\n  ```\r\n";

    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(md.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });

    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();

    cx.update(|_w, cx| {
        let ws = view.read(cx);
        let cache = ws.syntax_cache.lock().unwrap();

        // Must have exactly ONE entry in cache (shared between editor and preview)
        assert_eq!(
            cache.entries.len(),
            1,
            "Editor and preview must share exactly ONE cache entry for the same block"
        );

        let entry = &cache.entries[0];
        assert_eq!(
            entry.key.language,
            Some("Rust"),
            "Canonical language must be Rust"
        );
        assert_eq!(
            entry.key.payload.as_ref(),
            "fn a() {}\n",
            "Canonical payload must be LF-normalized without container indentation"
        );

        // Spans must be highlighted
        assert!(
            entry.spans.iter().any(|s| s.role == SyntaxRole::Keyword),
            "Must have keyword span for 'fn'"
        );
    });
}

#[gpui::test]
async fn test_workspace_bash_block_highlighted_not_plain(cx: &mut TestAppContext) {
    let md = "# Script\n\n```bash\n#!/bin/bash\necho \"hello world\"\n```\n";

    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(md.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });

    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();

    cx.update(|_w, cx| {
        let ws = view.read(cx);
        let cache = ws.syntax_cache.lock().unwrap();

        assert_eq!(cache.entries.len(), 1);
        let entry = &cache.entries[0];
        assert_eq!(entry.key.language, Some("Bourne Again Shell (bash)"));

        // Must have non-plain spans (comment, string, keyword)
        let has_non_plain = entry.spans.iter().any(|s| s.role != SyntaxRole::Plain);
        assert!(
            has_non_plain,
            "Bash code block must have syntax highlighting (not plain fallback), spans: {:?}",
            entry.spans
        );
    });
}

#[gpui::test]
async fn test_workspace_edit_then_draw_buffer_leads_ast(cx: &mut TestAppContext) {
    let md = "# Title\n\n```rust\nfn a() {}\n```\n";

    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(md.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Source);
        WorkspaceView::new(state, cx)
    });

    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();

    // Verify initially highlighted
    cx.update(|_w, cx| {
        let ws = view.read(cx);
        assert_eq!(ws.syntax_cache.lock().unwrap().entries.len(), 1);
    });

    // In cx.update view.update:
    // Simulate typing above the code block so line indices shift
    cx.update(|_window, cx| {
        view.update(cx, |ws, cx| {
            ws.state.editor.set_cursor(0, 0);
            ws.state.editor.insert_text("// comment\n");
            ws.state.sync_from_editor();
            ws.schedule_parse(cx);
            assert!(ws.state.doc.is_parse_stale(), "buffer must lead AST during debounce");
        });
    });

    // Draw immediately BEFORE run_until_parked
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    #[cfg(feature = "test-observation")]
    {
        // Read the tokens of line 4 (which is where `fn a() {}` has shifted) via `obs::editor_tokens(4)`
        let tokens = obs::editor_tokens(4).expect("tokens for line 4 must be Some");
        assert!(!tokens.is_empty(), "tokens for line 4 must not be empty");
        assert!(
            tokens.iter().all(|t| t.kind == MarkdownTokenKind::CodeText || t.kind == MarkdownTokenKind::Plain),
            "All tokens must be CodeText (or Plain), got: {:?}",
            tokens
        );
        assert!(
            !tokens.iter().any(|t| matches!(t.kind, MarkdownTokenKind::Keyword | MarkdownTokenKind::Number | MarkdownTokenKind::CodeDelimiter)),
            "No token should be Keyword, Number, or CodeDelimiter during stale debounce, got: {:?}",
            tokens
        );
        let combined_text: String = tokens.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(combined_text, "fn a() {}");
    }

    // Now let the debounce parse land:
    cx.executor().advance_clock(std::time::Duration::from_millis(150));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();

    #[cfg(feature = "test-observation")]
    {
        // Read obs::editor_tokens(4) again:
        let tokens_after = obs::editor_tokens(4).expect("tokens for line 4 after parse must be Some");
        assert!(
            tokens_after.iter().any(|t| t.kind == MarkdownTokenKind::Keyword && t.text == "fn"),
            "Tokens must contain Keyword for 'fn', got: {:?}",
            tokens_after
        );
    }
    cx.update(|_window, cx| {
        let ws = view.read(cx);
        assert!(!ws.state.doc.is_parse_stale(), "AST must no longer be stale");
    });
}

#[gpui::test]
async fn test_workspace_preview_only_highlight_and_single_entry(cx: &mut TestAppContext) {
    let md = "```rust\nfn a() { let v = 1; }\n```\n";

    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(md.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });

    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();

    cx.update(|_w, cx| {
        let ws = view.read(cx);
        let cache = ws.syntax_cache.lock().unwrap();

        assert_eq!(cache.entries.len(), 1, "Preview-only must have exactly 1 cache entry");
        let entry = &cache.entries[0];
        assert_eq!(entry.key.language, Some("Rust"));
        assert_eq!(entry.key.payload.as_ref(), "fn a() { let v = 1; }\n");

        assert!(
            entry.spans.iter().any(|s| s.role == SyntaxRole::Keyword || s.role == SyntaxRole::Number),
            "Entry spans must contain Keyword or Number, got: {:?}",
            entry.spans
        );

        #[cfg(feature = "test-observation")]
        {
            let last_highlights = obs::last_code_highlights().expect("obs::last_code_highlights must be Some");
            assert!(
                last_highlights.iter().any(|(_, h)| h.color == Some(ws.state.theme.syntax_keyword)),
                "Highlights must contain a highlight with syntax_keyword color"
            );
        }
    });
}

#[gpui::test]
async fn test_workspace_cache_isolation_and_no_global_leak(cx: &mut TestAppContext) {
    let md = "```rust\nfn unique_doc_code() {}\n```\n";

    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(md.to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Source);
        WorkspaceView::new(state, cx)
    });

    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();

    // Clear workspace cache (simulating document switch / reset_preview_list)
    cx.update(|_w, cx| {
        let ws = view.read(cx);
        ws.syntax_cache.lock().unwrap().clear();
    });

    // Verify cache is empty and cannot recover from any global static cache
    cx.update(|_w, cx| {
        let ws = view.read(cx);
        let mut cache = ws.syntax_cache.lock().unwrap();
        let key = CodeKey::from_raw(Some("rust"), Arc::from("fn unique_doc_code() {}\n"));
        assert!(
            cache.get(&key).is_none(),
            "Empty workspace cache must NOT recover spans from any global cache"
        );
    });
}
