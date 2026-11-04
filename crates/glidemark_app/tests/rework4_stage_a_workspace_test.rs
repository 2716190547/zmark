use glidemark_app::state::{AppState, ViewMode};
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

    // User types in editor: buffer leads AST during debounce
    cx.update(|_window, cx| {
        view.update(cx, |ws, _cx| {
            ws.state.editor.insert_text("z");
            let buffer_text = ws.state.editor.lines.join("\n");
            ws.state.doc.set_buffer(buffer_text);
            assert!(ws.state.doc.is_parse_stale());
        });
    });

    // Immediate draw before debounce completes: no panic, buffer rendered gracefully
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    // Simulate debounce timer firing: AST updates to match buffer
    cx.update(|_window, cx| {
        view.update(cx, |ws, _cx| {
            ws.state.doc.reparse();
            assert!(!ws.state.doc.is_parse_stale());
        });
    });

    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();

    // Verify recovery after debounce
    cx.update(|_w, cx| {
        let ws = view.read(cx);
        assert!(!ws.syntax_cache.lock().unwrap().entries.is_empty());
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
