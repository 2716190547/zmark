use gpui::TestAppContext;
use zmark_app::state::{AppState, ViewMode};
use zmark_app::views::workspace::{SyntaxBlockState, WorkspaceView};
use zmark_core::document::Document;

#[gpui::test]
async fn editing_a_code_block_prunes_old_syntax_state(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(
            "```rust\nlet a = 1;\n```\n\n```rust\nlet b = 2;\n```\n".to_string(),
        );
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Source);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    let (stale, kept) = cx.update(|_window, cx| {
        let ws = view.read(cx);
        (
            ws.state.doc().parsed.code_blocks[0].key.clone(),
            ws.state.doc().parsed.code_blocks[1].key.clone(),
        )
    });

    cx.update(|_window, cx| {
        view.update(cx, |ws, _cx| {
            ws.syntax_attempts.insert(stale.clone(), (1, 1));
            ws.syntax_attempts.insert(kept.clone(), (1, 1));
            ws.syntax_block_states
                .insert(stale.clone(), SyntaxBlockState::Ready);
            ws.syntax_block_states
                .insert(kept.clone(), SyntaxBlockState::Ready);
            ws.syntax_invalid_positions.insert((stale.clone(), 1));
        });
    });

    cx.update(|_window, cx| {
        view.update(cx, |ws, _cx| {
            ws.state.editor.set_cursor(1, "let a = 1;".len());
            ws.state.editor.insert_text("x");
            ws.parse_now_and_refresh();
        });
    });

    cx.update(|_window, cx| {
        let ws = view.read(cx);
        assert!(
            !ws.syntax_attempts.contains_key(&stale),
            "old code-block attempts must be pruned"
        );
        assert!(
            !ws.syntax_block_states.contains_key(&stale),
            "old code-block state must be pruned"
        );
        assert!(
            !ws.syntax_invalid_positions
                .contains(&(stale.clone(), 1)),
            "old invalid position must be pruned"
        );
        assert!(
            ws.syntax_attempts.contains_key(&kept),
            "unchanged code block must keep its attempts"
        );
        assert!(
            ws.syntax_block_states.contains_key(&kept),
            "unchanged code block must keep its state"
        );
    });
}
