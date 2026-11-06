use zmark_app::state::AppState;
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;
use gpui::TestAppContext;

#[gpui::test]
async fn test_workspace_real_layout_visual_lines_and_navigation(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        // Line with 5 tabs + inline code `code` (6 chars)
        // In GPUI monospace (14px):
        // 5 tabs = 42.0px. `code` = 50.4px + 8.0px padding = 58.4px. Total = 100.4px.
        // Under column estimation (tab=4 cols): 5 tabs = 20 cols, `code` = 6 cols. Total = 26 cols.
        // At 14px (unit_w = 8.4px), 14 cols = 117.6px.
        // Column estimation would wrap 26 cols into 2 lines!
        // But real layout at 120.0px width fits 100.4px in 1 line!
        doc.update_buffer("\t\t\t\t\t`code`\nShort second line".to_string());
        let state = AppState::new(doc);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    cx.update(|window, app| {
        view.update(app, |workspace, _cx| {
            let vis_lines = workspace.compute_visual_lines(window);
            // Must have visual lines produced by wrap_document_to_visual_layout
            assert!(!vis_lines.is_empty());
            assert_eq!(vis_lines[0].physical_line_idx, 0);

            // Test navigation with move_down_visual_lines and move_up_visual_lines
            workspace.state.editor.set_cursor(0, 0);
            workspace.state.editor.move_down_visual_lines(&vis_lines);
            // Moves to line 1
            assert_eq!(workspace.state.editor.cursor().0, 1);
            workspace.state.editor.move_up_visual_lines(&vis_lines);
            assert_eq!(workspace.state.editor.cursor().0, 0);
        });
    });
}

#[gpui::test]
async fn test_workspace_search_single_input_pipeline(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let doc = Document::new_scratch();
        let state = AppState::new(doc);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    cx.update(|_window, app| {
        view.update(app, |workspace, cx| {
            // 1. Initial state
            assert!(!workspace.search_focused);
            assert_eq!(workspace.state.search_query, "");

            // 2. Focus search and type via SearchInput EntityInputHandler
            workspace.search_focused = true;
            let input = workspace.search_input.clone().expect("search_input should exist");

            // Type "ab" through SearchInput
            input.update(cx, |inp, _cx| {
                inp.handle_replace_text(None, "ab");
            });
            assert_eq!(input.read(cx).content.as_ref(), "ab");

            // 3. Move left twice in SearchInput (cursor at 0)
            input.update(cx, |inp, _cx| {
                inp.handle_left();
                inp.handle_left();
            });
            assert_eq!(input.read(cx).selected_range, 0..0);

            // 4. Type 'z' at cursor 0: should become "zab" with cursor at 1 (NOT jumping to tail "abz")
            input.update(cx, |inp, _cx| {
                inp.handle_replace_text(None, "z");
            });
            assert_eq!(input.read(cx).content.as_ref(), "zab");
            assert_eq!(input.read(cx).selected_range, 1..1);
        });
    });
}
