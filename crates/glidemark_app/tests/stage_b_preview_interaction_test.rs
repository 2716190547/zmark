use glidemark_app::state::{AppState, ViewMode};
use glidemark_app::views::editor::DocumentLayoutCache;
use glidemark_app::views::preview::{
    calculate_text_click_char_offset, serialize_preview_selection,
    PreviewCharPosition, PreviewSelection,
};
use glidemark_app::views::workspace::WorkspaceView;
use glidemark_core::ast::parse_markdown;
use glidemark_core::document::Document;
use gpui::{div, px, Bounds, IntoElement, Point, Render, TestAppContext, Window};
use std::time::Instant;

struct EmptyTestView;

impl Render for EmptyTestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

// The non-zero-origin production entry is covered by phase_h_b1_real_geometry.
// Keep the distinct soft-wrap boundary check against the helper production uses.
#[gpui::test]
async fn test_preview_wrapped_text_hit_testing(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| EmptyTestView);
    cx.update(|window, _| {
        let hit = |y| calculate_text_click_char_offset(window.text_system(),
            "LineOneWords LineTwoWords LineThreeWords", 5., y,
            "ui-sans-serif, system-ui", 16., 1.75, Some(120.));
        assert_eq!(hit(10.), 1);
        assert!(hit(40.) > 10);
        assert!(hit(70.) > hit(40.));
    });
}

#[test]
fn test_b2_nested_list_and_blockquote_offset_mapping() {
    // 1. Nested list offset serialization and alignment matching review report
    let md_list = "- Parent\n  - Child A\n  - Child B";
    let parsed_list = parse_markdown(md_list);
    assert_eq!(parsed_list.blocks.len(), 1);

    let list_block = &parsed_list.blocks[0];
    let plain_text = list_block.to_plain_text();
    // Plain text layout:
    // - Parent (0..8)
    // \n  - Child A (8..20: \n is 8..9, '  ' is 9..11, '- ' is 11..13, 'Child A' is 13..20)
    assert!(plain_text.contains("- Parent"));
    assert!(plain_text.contains("Child A"));
    assert!(plain_text.contains("Child B"));

    let child_a_start = plain_text.find("Child A").expect("Child A in plain text");
    let child_a_end = child_a_start + "Child A".len();
    assert_eq!(child_a_start, 13);
    assert_eq!(child_a_end, 20);

    let sel = PreviewSelection::new(
        PreviewCharPosition::new(0, child_a_start),
        PreviewCharPosition::new(0, child_a_end),
    );
    let serialized = serialize_preview_selection(&parsed_list, sel);
    assert_eq!(
        serialized, "Child A",
        "Serializing 13..20 must produce exact substring 'Child A'"
    );

    // 2. BlockQuote multi-element mapping
    let md_quote = "> First line of quote\n>\n> Second line of quote";
    let parsed_quote = parse_markdown(md_quote);
    assert_eq!(parsed_quote.blocks.len(), 1);

    let quote_text = parsed_quote.blocks[0].to_plain_text();
    let second_quote_start = quote_text.find("Second line").expect("Second line in quote");
    let second_quote_end = second_quote_start + "Second line of quote".len();

    let sel_quote = PreviewSelection::new(
        PreviewCharPosition::new(0, second_quote_start),
        PreviewCharPosition::new(0, second_quote_end),
    );
    let serialized_quote = serialize_preview_selection(&parsed_quote, sel_quote);
    assert_eq!(
        serialized_quote, "Second line of quote",
        "Serializing second block of blockquote must yield exact line"
    );
}

#[gpui::test]
async fn test_focus_transfer_discriminator_without_on_editor_click(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer("initial line text\nsecond line".to_string());
        let state = AppState::new(doc);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    // 1. Focus workspace view and activate search via Cmd+F
    cx.update(|window, cx| {
        let handle = view.read(cx).focus_handle.clone();
        window.focus(&handle);
    });
    cx.run_until_parked();

    cx.simulate_keystrokes("cmd-f");
    cx.update(|_window, cx| {
        let ws = view.read(cx);
        assert!(ws.search_focused, "Cmd+F must activate search_focused");
    });

    // Type "foo" into search
    cx.simulate_keystrokes("f o o");
    cx.update(|_window, cx| {
        let ws = view.read(cx);
        let si = ws.search_input.as_ref().unwrap().read(cx);
        assert_eq!(si.content.as_ref(), "foo");
    });

    // 2. Critical review discriminator test:
    // Transfer focus to workspace.focus_handle DIRECTLY without calling `on_editor_click_line`!
    cx.update(|window, cx| {
        let handle = view.read(cx).focus_handle.clone();
        window.focus(&handle);
    });
    cx.run_until_parked();

    // 3. Immediately type 'q' in editor:
    // Because search_input lost focus, keystrokes must NOT be swallowed or directed to search!
    cx.simulate_keystrokes("q");
    cx.update(|_window, cx| {
        let ws = view.read(cx);
        // Search input must NOT receive 'q'
        let si = ws.search_input.as_ref().unwrap().read(cx);
        assert_eq!(si.content.as_ref(), "foo", "Search input must remain 'foo'");
        // Editor must have received 'q'
        // Editor must have received 'q'
        assert!(
            ws.state.editor.lines[1].ends_with('q'),
            "Editor line 1 must end with inserted 'q' after direct focus transfer"
        );
    });
}

#[gpui::test]
async fn test_checklist3_split_drag_continuous_width_optimization(cx: &mut TestAppContext) {
    let (_view, cx) = cx.add_window_view(|_window, _cx| EmptyTestView);
    cx.run_until_parked();

    cx.update(|window, _cx| {
        let text_system = window.text_system();
        let mut cache = DocumentLayoutCache::new();

        // Construct 2000 lines where 95% are short (< 300px) and 5% are long (> 600px)
        let mut lines = Vec::with_capacity(2000);
        for i in 0..2000 {
            if i % 20 == 0 {
                lines.push("A very long markdown paragraph line that exceeds narrower containers and requires soft-wrapping across multiple visual lines during split dragging.".to_string());
            } else {
                lines.push(format!("Short line number {} with normal text", i));
            }
        }

        // 1. Initial full layout at 800px: all 2000 lines are shaped
        let start_init = Instant::now();
        let fenced_mask = vec![false; lines.len()];
        let vl_init = cache.update(text_system, &lines, &fenced_mask, 800.0, 14.0);
        let dur_init = start_init.elapsed();
        assert_eq!(cache.metrics.shaped_lines_count, 2000);
        assert_eq!(cache.metrics.reused_lines_count, 0);
        assert!(vl_init.len() >= 2000);

        // 2. Split dragging step: width changes continuously (e.g. from 800px to 750px)
        // Short lines whose natural_width_px <= 750px are reused without font shaping!
        let start_drag = Instant::now();
        let vl_drag = cache.update(text_system, &lines, &fenced_mask, 750.0, 14.0);
        let dur_drag = start_drag.elapsed();

        assert!(
            cache.metrics.reused_lines_count >= 1800,
            "At least 1800 lines must be reused during split width change, got {}",
            cache.metrics.reused_lines_count
        );
        assert!(
            cache.metrics.shaped_lines_count <= 200,
            "At most ~100-200 lines re-wrapped, got {}",
            cache.metrics.shaped_lines_count
        );
        assert!(vl_drag.len() >= 2000);

        println!(
            "Continuous split drag performance: initial full layout {:?}, drag step {:?} (reused {} / shaped {})",
            dur_init, dur_drag, cache.metrics.reused_lines_count, cache.metrics.shaped_lines_count
        );
    });
}

#[gpui::test]
async fn test_view_mode_switch_clears_preview_selection_and_source_mode_no_preview_copy(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer("First line\nSecond line".to_string());
        let mut state = AppState::new(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    cx.update(|_window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.parse_now_and_refresh();

            // Set preview selection
            let sel = PreviewSelection::new(
                PreviewCharPosition::new(0, 0),
                PreviewCharPosition::new(0, 5),
            );
            workspace.preview_selection = Some(sel);
            assert!(workspace.preview_selection.is_some());

            // Switch to Source mode
            workspace.set_view_mode(ViewMode::Source, cx);
            assert!(
                workspace.preview_selection.is_none(),
                "Switching to Source mode must immediately clear preview_selection"
            );
        });
    });
}
