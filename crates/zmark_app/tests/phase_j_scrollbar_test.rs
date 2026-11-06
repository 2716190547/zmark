use zmark_app::state::{AppState, ViewMode};
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;
use gpui::{
    px, Modifiers, MouseButton, Point, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase,
};

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
    let track = cx
        .debug_bounds("preview_scrollbar")
        .expect("track is displayed");
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
    let top = Point {
        x: track.center().x,
        y: track.origin.y + px(2.),
    };
    let bottom = Point {
        x: track.center().x,
        y: track.origin.y + track.size.height - px(2.),
    };
    cx.simulate_mouse_down(top, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(bottom, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(bottom, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let thumb = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
    assert!(
        (f32::from(track.bottom()) - f32::from(thumb.bottom())).abs() < 1.0,
        "thumb reaches the bottom at document end"
    );
    cx.update(|_, cx| {
        let ws = view.read(cx);
        let count = ws.state.doc().parsed.blocks.len();
        assert!(
            ws.preview_list.logical_scroll_top().item_ix >= count - 20,
            "drag must reach the final viewport"
        );
        assert!(ws.preview_scroll_drag.is_none());
        assert!(
            f32::from(ws.editor_scroll_handle.0.borrow().base_handle.offset().y)
                < editor_offset_before,
            "preview track navigation synchronizes the source pane"
        );
    });
    let before = cx.update(|_, cx| view.read(cx).state.editor.to_text());
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert!(
            !view.read(cx).preview_scroll_focus.is_focused(window),
            "Tab moves focus off the scrollbar"
        )
    });
    cx.simulate_keystrokes("shift-tab");
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert!(
            view.read(cx).preview_scroll_focus.is_focused(window),
            "Shift+Tab returns to the scrollbar"
        )
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
    let track = cx.debug_bounds("preview_scrollbar").unwrap();
    let top = Point {
        x: track.center().x,
        y: track.origin.y + px(3.),
    };
    let bottom = Point {
        x: track.center().x,
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
        assert!(ws.preview_scroll_drag.is_some());
    });
    cx.deactivate_window();
    cx.update(|_, cx| assert!(view.read(cx).preview_scroll_drag.is_none()));
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
    let track = cx.debug_bounds("preview_scrollbar").unwrap();
    let start_thumb = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
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
    let inner_thumb = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
    assert!(
        inner_thumb.origin.y > start_thumb.origin.y,
        "thumb tracks measured local progress in a long block"
    );
    let grab = Point {
        x: track.center().x,
        y: inner_thumb.center().y,
    };
    cx.simulate_mouse_down(grab, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let after_press = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
    assert!(
        (f32::from(after_press.origin.y) - f32::from(inner_thumb.origin.y)).abs() < 1.0,
        "grabbing the thumb must not jump"
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
    let track = cx.debug_bounds("preview_scrollbar").unwrap();
    let end = Point {
        x: track.center().x,
        y: track.bottom() - px(1.),
    };
    cx.simulate_mouse_down(end, MouseButton::Left, Modifiers::none());
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
    let thumb = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
    assert!((f32::from(thumb.bottom()) - f32::from(track.bottom())).abs() < 1.0);
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
    let track = cx.debug_bounds("preview_scrollbar").unwrap();
    let thumb = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
    let target = Point {
        x: track.center().x,
        y: track.origin.y
            + px(
                (f32::from(track.size.height) - f32::from(thumb.size.height)) * 0.75
                    + f32::from(thumb.size.height) / 2.0,
            ),
    };
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
        let wheel = ScrollWheelEvent {
            position: end,
            delta: ScrollDelta::Pixels(Point {
                x: px(0.),
                y: px(30.),
            }),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
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
    let track = cx.debug_bounds("preview_scrollbar").unwrap();
    let thumb = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
    let start = thumb.center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.update(|window, cx| { let _ = window.draw(cx); });
    let outside = Point { x: px(-30.), y: track.origin.y + track.size.height * 0.5 };
    cx.simulate_mouse_move(outside, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| { let _ = window.draw(cx); });
    let actual = cx.debug_bounds("preview_scrollbar_thumb").unwrap().center().y;
    assert!((f32::from(actual - outside.y)).abs() <= 2., "thumb {actual:?}, pointer {:?}", outside.y);
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    cx.update(|_, cx| assert!(view.read(cx).preview_scroll_drag.is_none()));
    cx.update(|window, cx| { let _ = window.draw(cx); });
    let released = cx.debug_bounds("preview_scrollbar_thumb").unwrap().center().y;
    assert!((f32::from(released - outside.y)).abs() <= 2., "release moved thumb to {released:?}, pointer {:?}", outside.y);
}

#[gpui::test]
async fn sample_wheel_does_not_jump_thumb_to_bottom(cx: &mut TestAppContext) {
    let text = include_str!("../src/main.rs").split("let sample_text = r#\"").nth(1).unwrap().split("\"#;").next().unwrap().to_owned();
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
    let wheel_at = Point { x: track.origin.x - px(40.), y: track.center().y };
    let mut previous = f32::from(cx.debug_bounds("preview_scrollbar_thumb").unwrap().origin.y);
    for _ in 0..30 {
        cx.simulate_event(ScrollWheelEvent { position: wheel_at, delta: ScrollDelta::Pixels(gpui::point(px(0.), px(-12.))), modifiers: Modifiers::none(), touch_phase: TouchPhase::Moved });
        cx.update(|window, cx| { let _ = window.draw(cx); });
        cx.run_until_parked();
        cx.update(|window, cx| { let _ = window.draw(cx); });
        let y = f32::from(cx.debug_bounds("preview_scrollbar_thumb").unwrap().origin.y);
        assert!(y >= previous - 2. && y - previous < 100., "thumb jumped {previous} -> {y}");
        previous = y;
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
    let track = cx.debug_bounds("preview_scrollbar").unwrap();
    let thumb = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
    let start = thumb.center();
    let target = Point { x: start.x, y: track.origin.y + (track.size.height - thumb.size.height) * 0.97 + thumb.size.height * 0.5 };
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.update(|window, cx| { let _ = window.draw(cx); });
    cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    for _ in 0..3 {
        cx.run_until_parked(); cx.update(|window, cx| { let _ = window.draw(cx); });
        let y = cx.debug_bounds("preview_scrollbar_thumb").unwrap().center().y;
        assert!((f32::from(y - target.y)).abs() <= 2., "release moved thumb from {:?} to {y:?}", target.y);
    }
    cx.update(|_, cx| assert!(view.read(cx).preview_scroll_drag.is_none()));
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
    let thumb = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
    cx.simulate_mouse_down(thumb.center(), MouseButton::Left, Modifiers::none());
    cx.update(|_, cx| assert!(view.read(cx).preview_scroll_drag.is_some()));
    cx.update(|window, cx| { view.update(cx, |ws, cx| { ws.split_ratio = 0.7; cx.notify(); }); let _ = window.draw(cx); });
    cx.run_until_parked(); cx.update(|window, cx| { let _ = window.draw(cx); });
    cx.update(|_, cx| assert!(view.read(cx).preview_scroll_drag.is_none(), "viewport resize ends drag"));
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
    let thumb = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
    cx.simulate_mouse_down(thumb.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(thumb.center(), MouseButton::Left, Modifiers::none());
    cx.update(|window, cx| { let _ = window.draw(cx); });
    cx.simulate_keystrokes("down down down");
    cx.run_until_parked(); cx.update(|window, cx| { let _ = window.draw(cx); });
    let moved = cx.update(|_, cx| { let ws = view.read(cx); let top = ws.preview_list.logical_scroll_top(); (top.item_ix, f32::from(top.offset_in_item)) });
    assert!(moved.0 > 0 || moved.1 > 70., "repeated keys should accumulate: {moved:?}");
}
