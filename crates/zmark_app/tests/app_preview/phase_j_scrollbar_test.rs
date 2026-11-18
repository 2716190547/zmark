use zmark_app::state::{AppState, ViewMode};
use zmark_app::views::preview::PreviewScrollState;
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;
use gpui::{
    px, Bounds, Modifiers, MouseButton, Pixels, Point, ScrollDelta, ScrollEvent, ScrollSource,
    TestAppContext, TouchPhase,
};
use gpui_component::scroll::ScrollbarHandle as _;

fn reveal_preview_scrollbar(
    cx: &mut gpui::VisualTestContext,
    view: &gpui::Entity<WorkspaceView>,
) -> (Bounds<Pixels>, f32, f32) {
    let track = cx
        .debug_bounds("preview_scrollbar")
        .expect("preview scrollbar overlay is mounted");
    let hover_pt = Point {
        x: track.right() - px(6.),
        y: track.origin.y + px(16.),
    };
    cx.simulate_mouse_move(hover_pt, None, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let (logical_len, travel, ratio) = cx.update(|_, cx| {
        let ws = view.read(cx);
        let vp_h = f32::from(track.size.height).max(1.0);
        let content_h = f32::from(ws.preview_list.content_size().height).max(vp_h);
        let logical_len = ((vp_h / content_h) * vp_h).max(48.0).min(vp_h);
        let travel = (vp_h - logical_len).max(0.0);
        (logical_len, travel, ws.preview_list.scroll_ratio())
    });
    (track, logical_len, travel * ratio)
}

fn thumb_center_for(track: Bounds<Pixels>, logical_len: f32, offset_in_travel: f32) -> Point<Pixels> {
    Point {
        x: track.right() - px(6.),
        y: track.origin.y + px(offset_in_travel + logical_len * 0.5),
    }
}

#[gpui::test]
async fn real_drag_reaches_end_and_keyboard_does_not_edit(cx: &mut TestAppContext) {
    let text = (0..1000)
        .map(|n| format!("## Heading {n}\n\nSome paragraph {n}.\n\n"))
        .collect::<String>();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let (track, logical_len, thumb_offset) = reveal_preview_scrollbar(cx, &view);
    let editor_offset_before = cx.update(|_, cx| {
        f32::from(
            view.read(cx)
                .editor_scroll_handle
                .0
                .borrow()
                .base_handle
                .offset()
                .y,
        )
    });
    let start = thumb_center_for(track, logical_len, thumb_offset);
    let bottom = Point {
        x: start.x,
        y: track.origin.y + track.size.height - px(2.),
    };
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(bottom, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(bottom, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.update(|_, cx| {
        let ws = view.read(cx);
        let count = ws.state.doc().parsed.blocks.len();
        assert!(
            ws.preview_list.logical_scroll_top().item_ix >= count - 20,
            "drag must reach the final viewport"
        );
        assert!(!ws.preview_list.is_scrollbar_dragging());
        assert!(
            ws.preview_list.scroll_ratio() > 0.95,
            "ratio should reach document end, got {}",
            ws.preview_list.scroll_ratio()
        );
        assert!(
            f32::from(ws.editor_scroll_handle.0.borrow().base_handle.offset().y)
                < editor_offset_before,
            "preview track navigation synchronizes the source pane"
        );
    });
    let before = cx.update(|_, cx| view.read(cx).state.editor.to_text());
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.preview_focused = true;
            window.focus(&ws.focus_handle, cx);
        });
    });
    cx.simulate_keystrokes("home x end");
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert_eq!(ws.state.editor.to_text(), before);
        assert!(
            ws.preview_list.logical_scroll_top().item_ix >= ws.state.doc().parsed.blocks.len() - 20
        );
    });
}

#[gpui::test]
async fn one_tall_block_scrolls_and_window_deactivation_ends_drag(cx: &mut TestAppContext) {
    let text = format!(
        "```text\n{}\n```",
        (0..200).map(|n| format!("line {n}\n")).collect::<String>()
    );
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.update(|window, _| window.activate_window());
    let (track, logical_len, thumb_offset) = reveal_preview_scrollbar(cx, &view);
    let top = thumb_center_for(track, logical_len, thumb_offset);
    let bottom = Point {
        x: top.x,
        y: track.origin.y + track.size.height - px(3.),
    };
    cx.simulate_mouse_down(top, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(bottom, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert_eq!(ws.state.doc().parsed.blocks.len(), 1);
        assert!(ws.preview_list.logical_scroll_top().offset_in_item > px(100.));
        assert!(ws.preview_list.is_scrollbar_dragging());
    });
    cx.deactivate_window();
    cx.update(|_, cx| assert!(!view.read(cx).preview_list.is_scrollbar_dragging()));
}

#[gpui::test]
async fn measured_long_block_moves_thumb_within_document_order(cx: &mut TestAppContext) {
    let long = (0..500)
        .map(|n| format!("let variable_{n} = {n};\n"))
        .collect::<String>();
    let text = format!("```rust\n{long}```\n\n## End\n\nshort");
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let start_ratio = cx.update(|_, cx| view.read(cx).preview_list.scroll_ratio());
    cx.update(|_, cx| {
        view.update(cx, |ws, _| {
            assert!(ws.preview_list.bounds_for_item(0).unwrap().size.height > px(600.));
            ws.preview_list.scroll_to(gpui::ListOffset {
                item_ix: 0,
                offset_in_item: px(180.),
            });
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let inner_ratio = cx.update(|_, cx| view.read(cx).preview_list.scroll_ratio());
    assert!(
        inner_ratio > start_ratio,
        "scroll ratio tracks measured local progress in a long block"
    );
    let (track, logical_len, thumb_offset) = reveal_preview_scrollbar(cx, &view);
    let grab = thumb_center_for(track, logical_len, thumb_offset);
    let offset_before = cx.update(|_, cx| view.read(cx).preview_list.offset());
    cx.simulate_mouse_down(grab, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let offset_after = cx.update(|_, cx| view.read(cx).preview_list.offset());
    assert!(
        (offset_after - offset_before).abs() < 4.0,
        "grabbing the thumb must not jump: {offset_before} -> {offset_after}"
    );
    cx.simulate_mouse_up(grab, MouseButton::Left, Modifiers::none());
}

#[gpui::test]
async fn drag_to_end_reaches_bottom_of_final_tall_block(cx: &mut TestAppContext) {
    let tail = (0..250).map(|n| format!("line {n}\n")).collect::<String>();
    let text = format!("Short first paragraph.\n\n```text\n{tail}```");
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let (track, logical_len, thumb_offset) = reveal_preview_scrollbar(cx, &view);
    let start = thumb_center_for(track, logical_len, thumb_offset);
    let end = Point {
        x: start.x,
        y: track.bottom() - px(1.),
    };
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let viewport = cx.update(|_, cx| view.read(cx).preview_list.viewport_bounds());
    let last = cx
        .update(|_, cx| view.read(cx).preview_list.bounds_for_item(1))
        .expect("last item measured");
    assert!(
        f32::from(viewport.bottom() - last.bottom()) >= -2.0
            && f32::from(viewport.bottom() - last.bottom()) < 60.0,
        "last block should end at the viewport bottom, allowing the list's trailing padding"
    );
    let ratio = cx.update(|_, cx| view.read(cx).preview_list.scroll_ratio());
    assert!(ratio > 0.99, "scroll ratio should reach 1.0, got {ratio}");
}

#[gpui::test]
async fn dragging_into_final_tall_block_can_reach_its_middle(cx: &mut TestAppContext) {
    let tail = (0..250).map(|n| format!("line {n}\n")).collect::<String>();
    let text = format!("Short first paragraph.\n\n```text\n{tail}```");
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let (track, logical_len, _) = reveal_preview_scrollbar(cx, &view);
    let travel = (f32::from(track.size.height) - logical_len).max(0.0);
    let target = thumb_center_for(track, logical_len, travel * 0.75);
    cx.simulate_mouse_down(target, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    let (last_top, last_bounds) = cx.update(|_, cx| {
        let ws = view.read(cx);
        assert_eq!(ws.state.doc().parsed.blocks.len(), 2);
        (
            ws.preview_list.logical_scroll_top(),
            ws.preview_list.bounds_for_item(1).unwrap(),
        )
    });
    assert!(
        last_top.item_ix == 1 && last_top.offset_in_item > px(500.),
        "the final block's interval must provide access to its middle"
    );
    assert!(
        last_bounds.top() < track.bottom() && last_bounds.bottom() > track.origin.y,
        "the final block remains visible after coarse seeking"
    );
}

#[gpui::test]
async fn later_wheel_cancels_deferred_end_seek(cx: &mut TestAppContext) {
    let text = (0..100)
        .map(|n| format!("## Heading {n}\n\nSome paragraph {n}.\n\n"))
        .collect::<String>();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let track = cx.debug_bounds("preview_scrollbar").unwrap();
    let end = Point {
        x: track.center().x,
        y: track.bottom() - px(1.),
    };
    cx.update(|window, cx| {
        let wheel = ScrollEvent {
            position: end,
            delta: ScrollDelta::Pixels(Point {
                x: px(0.),
                y: px(30.),
            }),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        };
        view.update(cx, |ws, _| {
            ws.preview_list.scroll_to(gpui::ListOffset {
                item_ix: ws.state.doc().parsed.blocks.len() - 1,
                offset_in_item: px(0.),
            });
            ws.preview_scroll_end_pending = true;
        });
        let _ = window.draw(cx);
        view.update(cx, |ws, cx| {
            assert!(ws.preview_scroll_end_pending);
            ws.preview_list.scroll_to(gpui::ListOffset {
                item_ix: 0,
                offset_in_item: px(0.),
            });
            ws.handle_preview_scroll_wheel(&wheel, window, cx);
            assert!(!ws.preview_scroll_end_pending);
        });
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.update(|_, cx| {
        assert_eq!(
            view.read(cx).preview_list.logical_scroll_top().item_ix,
            0,
            "the stale deferred end seek must not override the later scroll"
        )
    });
}

#[gpui::test]
async fn thumb_follows_pointer_outside_window_and_releases(cx: &mut TestAppContext) {
    let text = (0..100).map(|n| format!("## {n}\n\n{}\n\n", "long text ".repeat((n % 20) + 1))).collect::<String>();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| { view.update(cx, |ws, cx| { ws.parse_now_and_refresh(); cx.notify(); }); let _ = window.draw(cx); });
    cx.run_until_parked();
    cx.update(|window, cx| { let _ = window.draw(cx); });
    let (track, logical_len, thumb_offset) = reveal_preview_scrollbar(cx, &view);
    let start = thumb_center_for(track, logical_len, thumb_offset);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.update(|window, cx| { let _ = window.draw(cx); });
    let travel = (f32::from(track.size.height) - logical_len).max(1.0);
    let outside = Point {
        x: px(-30.),
        y: track.origin.y + px(travel * 0.5 + logical_len * 0.5),
    };
    cx.simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| { let _ = window.draw(cx); });
    let ratio = cx.update(|_, cx| view.read(cx).preview_list.scroll_ratio());
    assert!((ratio - 0.5).abs() <= 0.05, "expected ratio ~0.5 while dragging outside window, got {ratio}");
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    cx.update(|_, cx| assert!(!view.read(cx).preview_list.is_scrollbar_dragging()));
    cx.update(|window, cx| { let _ = window.draw(cx); });
    let released = cx.update(|_, cx| view.read(cx).preview_list.scroll_ratio());
    assert!((released - 0.5).abs() <= 0.05, "release moved ratio to {released}");
}

#[gpui::test]
async fn sample_wheel_does_not_jump_thumb_to_bottom(cx: &mut TestAppContext) {
    let text = include_str!("../fixtures/legacy_startup_sample.md").to_owned();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| { view.update(cx, |ws, cx| { ws.parse_now_and_refresh(); cx.notify(); }); let _ = window.draw(cx); });
    cx.run_until_parked();
    cx.update(|window, cx| { let _ = window.draw(cx); });
    let track = cx.debug_bounds("preview_scrollbar").unwrap();
    let wheel_at = Point { x: track.center().x, y: track.center().y };
    let mut previous = cx.update(|_, cx| view.read(cx).preview_list.scroll_ratio());
    for _ in 0..30 {
        cx.simulate_event(ScrollEvent { position: wheel_at, delta: ScrollDelta::Pixels(gpui::point(px(0.), px(-12.))), modifiers: Modifiers::none(), touch_phase: TouchPhase::Moved, source: ScrollSource::Pointer });
        cx.update(|window, cx| { let _ = window.draw(cx); });
        cx.run_until_parked();
        cx.update(|window, cx| { let _ = window.draw(cx); });
        let ratio = cx.update(|_, cx| view.read(cx).preview_list.scroll_ratio());
        assert!(ratio >= previous - 0.01 && ratio - previous < 0.15, "scroll ratio jumped {previous} -> {ratio}");
        previous = ratio;
    }
}

#[gpui::test]
async fn unmeasured_end_drag_release_keeps_thumb_position(cx: &mut TestAppContext) {
    let text = (0..500).map(|n| format!("## Heading {n}\n\nA paragraph.\n\n")).collect::<String>();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch(); doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc); state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| { view.update(cx, |ws, cx| { ws.parse_now_and_refresh(); cx.notify(); }); let _ = window.draw(cx); });
    cx.run_until_parked(); cx.update(|window, cx| { let _ = window.draw(cx); });
    let (track, logical_len, thumb_offset) = reveal_preview_scrollbar(cx, &view);
    let start = thumb_center_for(track, logical_len, thumb_offset);
    let travel = (f32::from(track.size.height) - logical_len).max(1.0);
    let target = thumb_center_for(track, logical_len, travel * 0.97);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.update(|window, cx| { let _ = window.draw(cx); });
    cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    for _ in 0..3 {
        cx.run_until_parked(); cx.update(|window, cx| { let _ = window.draw(cx); });
        let ratio = cx.update(|_, cx| view.read(cx).preview_list.scroll_ratio());
        assert!((ratio - 0.97).abs() <= 0.04, "release moved ratio from 0.97 to {ratio}");
    }
    cx.update(|_, cx| assert!(!view.read(cx).preview_list.is_scrollbar_dragging()));
}

#[gpui::test]
async fn split_resize_cancels_drag_in_unmeasured_document(cx: &mut TestAppContext) {
    let text = (0..500).map(|n| format!("## Heading {n}\n\nParagraph with some words.\n\n")).collect::<String>();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch(); doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc); state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| { view.update(cx, |ws, cx| { ws.parse_now_and_refresh(); cx.notify(); }); let _ = window.draw(cx); });
    cx.run_until_parked(); cx.update(|window, cx| { let _ = window.draw(cx); });
    let (track, logical_len, thumb_offset) = reveal_preview_scrollbar(cx, &view);
    let start = thumb_center_for(track, logical_len, thumb_offset);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.update(|_, cx| assert!(view.read(cx).preview_list.is_scrollbar_dragging()));
    cx.update(|window, cx| { view.update(cx, |ws, cx| { ws.split_ratio = 0.7; cx.notify(); }); let _ = window.draw(cx); });
    cx.run_until_parked(); cx.update(|window, cx| { let _ = window.draw(cx); });
    cx.update(|_, cx| assert!(!view.read(cx).preview_list.is_scrollbar_dragging(), "viewport resize ends drag"));
}

#[gpui::test]
async fn repeated_arrow_keys_advance_without_intermediate_draw(cx: &mut TestAppContext) {
    let text = (0..40).map(|n| format!("## Heading {n}\n\n{}\n\n", "Words ".repeat(20))).collect::<String>();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch(); doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc); state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| { view.update(cx, |ws, cx| { ws.parse_now_and_refresh(); cx.notify(); }); let _ = window.draw(cx); });
    cx.run_until_parked(); cx.update(|window, cx| { let _ = window.draw(cx); });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.preview_focused = true;
            window.focus(&ws.focus_handle, cx);
        });
    });
    cx.simulate_keystrokes("down down down");
    cx.run_until_parked(); cx.update(|window, cx| { let _ = window.draw(cx); });
    let moved = cx.update(|_, cx| { let ws = view.read(cx); let top = ws.preview_list.logical_scroll_top(); (top.item_ix, f32::from(top.offset_in_item)) });
    assert!(moved.0 > 0 || moved.1 > 70., "repeated keys should accumulate: {moved:?}");
}

#[gpui::test]
async fn outline_jump_eases_and_manual_wheel_cancels_it(cx: &mut TestAppContext) {
    let text = (0..300)
        .map(|n| format!("## Heading {n}\n\nSome paragraph {n}.\n\n"))
        .collect::<String>();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    let target = cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            let target = ws.preview_list.offset_for_block(400, 0.0).min(ws.preview_list.max_scroll());
            ws.animate_preview_scroll_to(target, window, cx);
            target
        })
    });
    assert!(target > 1000.0, "fixture must be long enough to need easing");
    // Nothing has ticked yet: an eased jump must not land immediately.
    let offset = |cx: &mut gpui::VisualTestContext| cx.update(|_, cx| view.read(cx).preview_list.offset());
    assert!(offset(cx) < target * 0.5);

    cx.executor().advance_clock(std::time::Duration::from_millis(100));
    cx.run_until_parked();
    let mid = offset(cx);
    assert!(mid > 0.0, "scroll must have started moving");

    // A manual wheel event cancels the animation and keeps the user's position.
    cx.simulate_event(ScrollEvent {
        position: Point::new(px(600.), px(300.)),
        delta: ScrollDelta::Pixels(Point::new(px(0.), px(-1.))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    let after_wheel = offset(cx);
    cx.executor().advance_clock(std::time::Duration::from_millis(600));
    cx.run_until_parked();
    assert!(
        (offset(cx) - after_wheel).abs() < 1.0,
        "animation must stop after manual scroll"
    );
    assert!(offset(cx) < target - 1.0);
}

#[gpui::test]
async fn split_scrollbars_share_virtual_content_height_and_sync_smoothly(cx: &mut TestAppContext) {
    let text = (0..40)
        .map(|n| {
            format!(
                "## Heading {n}\n\nParagraph {n} with enough text for a medium scroll range.\n\n```rust\nlet x_{n} = {n};\nlet y_{n} = {n} + 1;\n```\n\n"
            )
        })
        .collect::<String>();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(text);
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    // Both panes report the exact same virtual content height in Split mode so the
    // native gpui-component Scrollbar computes identical thumb lengths.
    cx.update(|_, cx| {
        let ws = view.read(cx);
        let base = &ws.editor_scroll_handle.0.borrow().base_handle;
        let editor_total = f32::from(base.bounds().size.height + base.max_offset().y);
        let preview_total = ws.preview_list.total_height();
        let shared = ws
            .preview_list
            .shared_content_height()
            .expect("split mode sets shared scrollbar height");
        assert!((shared - editor_total.max(preview_total)).abs() < 1.0);
        assert!(
            (f32::from(PreviewScrollState::content_size(&ws.preview_list).height) - shared).abs()
                < 1.0
        );
    });

    // Wheeling the editor starts a smooth chase on the preview rather than snapping
    // in a single discontinuous jump, and glides cleanly to rest after a few frames.
    let editor_track = cx.debug_bounds("editor_scrollbar").unwrap();
    cx.simulate_event(ScrollEvent {
        position: editor_track.center(),
        delta: ScrollDelta::Pixels(gpui::point(px(0.), px(-240.))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    let first_step = cx.update(|_, cx| view.read(cx).preview_list.offset());
    assert!(first_step > 0.0, "first step should respond immediately");
    cx.executor().advance_clock(std::time::Duration::from_millis(80));
    cx.run_until_parked();
    let settled = cx.update(|_, cx| view.read(cx).preview_list.offset());
    assert!(
        settled >= first_step,
        "smooth sync chase must continue gliding forward: {first_step} -> {settled}"
    );
}
