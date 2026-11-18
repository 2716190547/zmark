use zmark_app::state::{AppState, ViewMode};
use zmark_app::views::drawer::SearchInput;
use zmark_app::views::editor::DocumentLayoutCache;
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;
use gpui::{px, Focusable, Point, ScrollDelta, ScrollEvent, ScrollSource, TestAppContext};
use std::time::Instant;

#[gpui::test]
async fn test_search_input_simulate_keystrokes_pipeline(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|window, cx| {
        zmark_app::theme::init_components(cx);
        SearchInput::new(window, cx).placeholder("Search...")
    });
    cx.run_until_parked();

    // 1. Focus SearchInput and simulate "a b x"
    cx.update(|window, cx| {
        let handle = view.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
    cx.run_until_parked();

    cx.simulate_keystrokes("a b x");
    cx.update(|_window, cx| {
        let input = view.read(cx);
        assert_eq!(
            input.value().as_ref(),
            "abx",
            "Typing 'a b x' via simulate_keystrokes must produce 'abx'"
        );
        assert_eq!(input.selected_range(), 3..3);
    });

    // 2. Move left twice and type 'z' -> "azbx", cursor at 2..2
    cx.simulate_keystrokes("left left z");
    cx.update(|_window, cx| {
        let input = view.read(cx);
        assert_eq!(
            input.value().as_ref(),
            "azbx",
            "Typing 'left left z' must produce 'azbx'"
        );
        assert_eq!(
            input.selected_range(),
            2..2,
            "Cursor must be immediately after 'z' at index 2"
        );
    });

    // 3. Shift+left select and type 'q' -> "aqbx", cursor at 2..2
    cx.simulate_keystrokes("shift-left q");
    cx.update(|_window, cx| {
        let input = view.read(cx);
        assert_eq!(
            input.value().as_ref(),
            "aqbx",
            "Shift-left selection replaced by 'q' must produce 'aqbx'"
        );
        assert_eq!(input.selected_range(), 2..2);
    });

    // 4. Cmd+A and type 'r' -> "r", cursor at 1..1
    cx.simulate_keystrokes("secondary-a r");
    cx.update(|_window, cx| {
        let input = view.read(cx);
        assert_eq!(
            input.value().as_ref(),
            "r",
            "Cmd-a select all followed by 'r' must produce 'r'"
        );
        assert_eq!(input.selected_range(), 1..1);
    });
}

#[gpui::test]
async fn test_search_to_editor_focus_transition_and_typing(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer("initial line\nsecond line".to_string());
        let mut state = AppState::new_isolated(doc);
        state.drawer_open = true;
        state.preferences.reduce_motion = true;
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    // File-name search is a separate input; Cmd+F opens file-content find.
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| {
        let input = view.read(cx).search_input.clone().unwrap();
        let focus = input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let ws = view.read(cx);
        assert!(ws.search_input.as_ref().unwrap().read(cx).focus_handle(cx).is_focused(window),
            "the real sidebar input must own focus before typing");
    });

    // 2. Type "foo" into search
    cx.simulate_keystrokes("f o o");
    cx.update(|_window, cx| {
        let ws = view.read(cx);
        let si = ws.search_input.as_ref().unwrap().read(cx);
        assert_eq!(
            si.value().as_ref(),
            "foo",
            "Search input must receive 'foo'"
        );
    });

    // 3. Click editor line to transfer focus
    cx.update(|window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.on_editor_click_line(0, 0, window, cx);
        });
    });
    cx.run_until_parked();

    // Verify search_focused was cleared and workspace.focus_handle is focused
    cx.update(|window, cx| {
        let ws = view.read(cx);
        assert!(
            !ws.search_focused,
            "Clicking editor must clear search_focused"
        );
        assert!(
            ws.focus_handle.is_focused(window),
            "Workspace focus_handle must be focused"
        );
    });

    // 4. Type 'q' in editor
    cx.simulate_keystrokes("q");
    cx.update(|_window, cx| {
        let ws = view.read(cx);
        // Search input content must NOT receive 'q'
        let si = ws.search_input.as_ref().unwrap().read(cx);
        assert_eq!(si.value().as_ref(), "foo", "Search input must remain 'foo'");
        // Editor line must contain 'q'
        assert!(
            ws.state.editor.lines[0].starts_with('q'),
            "Editor line 0 must start with inserted 'q'"
        );
    });
}

#[gpui::test]
async fn test_preview_scroll_reverse_mapping_with_soft_wrap(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        // Construct line 0 with enough text to wrap into multiple visual lines at ~300px
        let long_line_0 = "First physical line that is intentionally made very long with multiple repeated words so that it wraps into at least three visual lines in a narrow container.";
        let line_1 = "# Second Physical Heading";
        // Keep the document taller than the preview viewport so the wheel
        // handler has a real scroll range to move within.
        let filler = "Filler paragraph with enough words to extend the document beyond the preview viewport. ".repeat(80);
        doc.update_buffer(format!("{}\n\n{}\n\n{}", long_line_0, line_1, filler));
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    // Parse, then draw once so the preview size table has real origins before
    // the block offset is set (scroll_to cannot leave the top padding otherwise).
    cx.update(|window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        view.update(cx, |workspace, cx| {
            // Set narrow width to force soft wrapping
            workspace.workspace_width = 400.0;
            let vis_lines = workspace.compute_visual_lines(window);

            // Physical line 0 should wrap into >= 2 visual lines
            let phys0_count = vis_lines.iter().filter(|vl| vl.physical_line_idx == 0).count();
            assert!(phys0_count >= 2, "Physical line 0 must wrap into at least 2 visual lines, got {}", phys0_count);

            // Find visual line index for physical heading line
            let heading_phys_idx = workspace.state.editor.lines.iter().position(|l| l.starts_with("# Second")).expect("Heading line exists");
            let expected_vis_idx = vis_lines.iter().position(|vl| vl.physical_line_idx == heading_phys_idx).expect("Visual line for heading exists");
            assert!(expected_vis_idx > heading_phys_idx, "expected_vis_idx ({}) must be strictly greater than heading_phys_idx ({}) due to soft wrapping", expected_vis_idx, heading_phys_idx);

            // Set preview list scroll to heading block (block index 1)
            workspace.preview_list.scroll_to(gpui::ListOffset {
                item_ix: 1,
                offset_in_item: px(0.0),
            });

            // Trigger handle_preview_scroll_wheel. A real (non-zero) delta is
            // required now that zero-delta events are no-ops; 1px down stays
            // inside block 1 and remains within the scroll range.
            let dummy_event = ScrollEvent {
                position: Point::default(),
                delta: ScrollDelta::Pixels(Point {
                    x: px(0.),
                    y: px(-1.),
                }),
                modifiers: gpui::Modifiers::none(),
                touch_phase: gpui::TouchPhase::Moved,
                source: ScrollSource::Pointer,
            };
            workspace.handle_preview_scroll_wheel(&dummy_event, window, cx);

            // Verify reverse sync targets expected_vis_idx (NOT heading_phys_idx!),
            // either via deferred item scroll before layout or continuous smooth target after layout.
            let row_h = zmark_app::views::editor::calculate_line_row_height(
                workspace.state.preferences.font_size,
                workspace.state.preferences.line_height,
            );
            let deferred = workspace
                .editor_scroll_handle
                .0
                .borrow()
                .deferred_scroll_to_item;
            if let Some(deferred) = deferred {
                assert_eq!(
                    deferred.item_index,
                    expected_vis_idx,
                    "handle_preview_scroll_wheel must scroll to visual line index {}, NOT physical line index {}",
                    expected_vis_idx,
                    heading_phys_idx
                );
            } else {
                let (origin, target_y) = workspace
                    .sync_scroll_target()
                    .expect("sync_scroll_target should be set after layout");
                assert_eq!(origin, zmark_app::views::workspace::ScrollOrigin::Editor);
                let target_vis = (target_y / row_h).floor() as usize;
                assert_eq!(
                    target_vis,
                    expected_vis_idx,
                    "handle_preview_scroll_wheel must scroll to visual line index {}, NOT physical line index {} (target_y={}, row_h={})",
                    expected_vis_idx,
                    heading_phys_idx,
                    target_y,
                    row_h
                );
                let current_y = -f32::from(workspace.editor_scroll_handle.0.borrow().base_handle.offset().y);
                assert!(current_y > 0.0, "first smooth sync step must advance editor offset immediately");
            }
        });
    });
}

#[gpui::test]
async fn test_layout_cache_incremental_performance_and_fence_propagation(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut doc = Document::new_scratch();
        // Generate a 2000-line markdown document
        let mut lines = Vec::with_capacity(2000);
        for i in 0..2000 {
            lines.push(format!(
                "Line {} of sample document with some normal markdown text",
                i + 1
            ));
        }
        doc.update_buffer(lines.join("\n"));
        let state = AppState::new_isolated(doc);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        view.update(cx, |workspace, _cx| {
            // Invalidate cache to measure true full layout on 2000 lines
            workspace.cached_visual_rev = usize::MAX;
            workspace.cached_visual_lines = std::sync::Arc::new(Vec::new());
            workspace.document_layout_cache = zmark_app::views::editor::DocumentLayoutCache::new();

            // 1. Initial full shape on 2000 lines
            let t0 = Instant::now();
            let _ = workspace.compute_visual_lines(window);
            let init_duration = t0.elapsed();
            println!("[PERF] Initial 2000-line full layout duration: {:?}", init_duration);

            assert_eq!(
                workspace.document_layout_cache.metrics.shaped_lines_count,
                2000,
                "First pass must shape all 2000 lines"
            );
            assert_eq!(
                workspace.document_layout_cache.metrics.reused_lines_count,
                0,
                "First pass has no cached lines to reuse"
            );

            // 2. Incremental single character edit on line 500
            workspace.state.editor.set_cursor(500, 0);
            workspace.state.editor.insert_text("!");

            let t1 = Instant::now();
            let _ = workspace.compute_visual_lines(window);
            let incremental_duration = t1.elapsed();
            println!("[PERF] Incremental single-char edit duration: {:?}", incremental_duration);

            // Assert EXACT workload metrics: 1 line shaped, 1999 lines reused!
            assert_eq!(
                workspace.document_layout_cache.metrics.shaped_lines_count,
                1,
                "Editing a single line must shape EXACTLY 1 line, preventing full-document recomputation"
            );
            assert_eq!(
                workspace.document_layout_cache.metrics.reused_lines_count,
                1999,
                "Editing line 500 must reuse the remaining 1999 lines"
            );
            assert!(
                incremental_duration < init_duration,
                "Incremental edit ({:?}) must be significantly faster than initial layout ({:?})",
                incremental_duration,
                init_duration
            );

            // 3. Opening a code block on line 100 invalidates fence state downwards
            let mut new_lines = (*workspace.state.editor.lines).clone();
            new_lines[100] = "```rust".to_string();
            new_lines[110] = "```".to_string();
            workspace.state.editor.lines = std::sync::Arc::new(new_lines);
            workspace.state.editor.revision += 1;
            workspace.state.sync_from_editor();
            workspace.state.parse_now();

            let _ = workspace.compute_visual_lines(window);
            // Lines 100..=110 had their fence state changed and must be re-shaped
            assert!(
                workspace.document_layout_cache.metrics.shaped_lines_count >= 11,
                "Fence change must propagate and re-shape affected lines (shaped {})",
                workspace.document_layout_cache.metrics.shaped_lines_count
            );
            assert!(
                workspace.document_layout_cache.metrics.reused_lines_count <= 2000 - 11,
                "Lines outside fence region can still be reused"
            );

            // 4. Layout parameter change (font size change or drawer toggle) triggers full re-wrap
            workspace.state.preferences.font_size = 18.0;
            let _ = workspace.compute_visual_lines(window);
            assert_eq!(
                workspace.document_layout_cache.metrics.shaped_lines_count,
                2000,
                "Changing font size must re-shape all lines"
            );
            assert_eq!(
                workspace.document_layout_cache.metrics.reused_lines_count,
                0,
                "Changing font size cannot reuse lines from previous font size"
            );

            // 5. Unchanged lines must retain their shaped text allocations, and an
            // incremental insertion must produce the same layout as a fresh cache.
            let lines: Vec<String> = (0..120)
                .map(|i| format!("line {i}: {}", "text ".repeat(100)))
                .collect();
            let fenced = vec![false; lines.len()];
            let mut cache = DocumentLayoutCache::new();
            let expected = cache.update(window.text_system(), &lines, &fenced, 3000., 14.);
            let original_ptr = cache.entries[60].spans.as_ptr();
            assert_eq!(
                cache.update(window.text_system(), &lines, &fenced, 3000., 14.),
                expected
            );
            assert_eq!(
                cache.entries[60].spans.as_ptr(),
                original_ptr,
                "unchanged line must retain its layout allocation"
            );

            let mut inserted = lines.clone();
            inserted.insert(10, "new line".into());
            let flags = vec![false; inserted.len()];
            let actual = cache.update(window.text_system(), &inserted, &flags, 3000., 14.);
            let mut fresh = DocumentLayoutCache::new();
            assert_eq!(
                actual,
                fresh.update(window.text_system(), &inserted, &flags, 3000., 14.)
            );
            assert_eq!(
                cache.entries[61].spans.as_ptr(),
                original_ptr,
                "suffix reuse must retain its layout allocation"
            );
        });
    });
}
