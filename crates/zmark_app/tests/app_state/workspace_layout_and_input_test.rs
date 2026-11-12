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
    use gpui::Focusable;
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut state = AppState::new_isolated(Document::new_scratch());
        state.drawer_open = true;
        state.preferences.reduce_motion = true;
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();
    let input = cx.update(|window, cx| {
        let workspace = view.read(cx);
        assert!(!workspace.search_focused);
        assert_eq!(workspace.state.search_query, "");
        let input = workspace.search_input.clone().expect("native search input mounted");
        let focus = input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        input
    });
    cx.simulate_input("ab");
    cx.update(|_, cx| {
        assert_eq!(input.read(cx).value().as_ref(), "ab");
        assert_eq!(view.read(cx).state.search_query, "ab");
    });
    cx.simulate_keystrokes("left left");
    cx.update(|_, cx| assert_eq!(input.read(cx).selected_range(), 0..0));
    cx.simulate_input("z");
    cx.update(|_, cx| {
        assert_eq!(input.read(cx).value().as_ref(), "zab", "typing must not jump to the tail or double-insert");
        assert_eq!(input.read(cx).selected_range(), 1..1);
        assert_eq!(view.read(cx).state.search_query, "zab");
        assert_eq!(view.read(cx).state.editor.revision, 0);
    });
}
