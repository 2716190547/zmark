//! Size-table preview scrollbar regressions: the thumb must follow the pointer
//! all the way through an unmeasured long document, and editor scrolling to the
//! bottom must not pull the preview back into the measured region.

use zmark_app::state::{AppState, ViewMode};
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;
use gpui::{
    point, px, Modifiers, MouseButton, Point, ScrollDelta, ScrollEvent, ScrollSource, TestAppContext,
    TouchPhase,
};

fn long_doc() -> String {
    (0..180)
        .map(|n| {
            format!(
                "## Section {n}\n\nParagraph with enough words to occupy a line and make heights vary by block {n}.\n\n~~~python\nprint({n})\n~~~\n\n"
            )
        })
        .collect()
}

fn settle(cx: &mut gpui::VisualTestContext, frames: usize) {
    for _ in 0..frames {
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.run_until_parked();
    }
}

#[gpui::test]
async fn thumb_follows_pointer_through_unmeasured_long_document(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(long_doc());
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    settle(cx, 8);

    let track = cx.debug_bounds("preview_scrollbar").unwrap();
    let thumb = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
    let start = thumb.center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    let mut previous_item = 0usize;
    for ratio in [0.31_f32, 0.68, 0.97] {
        let target_y = track.origin.y
            + (track.size.height - thumb.size.height) * ratio
            + thumb.size.height * 0.5;
        let target = Point {
            x: start.x,
            y: target_y,
        };
        cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
        settle(cx, 3);
        let actual = cx
            .debug_bounds("preview_scrollbar_thumb")
            .unwrap()
            .center()
            .y;
        assert!(
            (f32::from(actual - target_y)).abs() <= 2.0,
            "thumb must follow pointer at ratio {ratio}: thumb {actual:?}, pointer {target_y:?}"
        );
        let (state_ratio, item) = cx.update(|_, cx| {
            let ws = view.read(cx);
            (
                ws.preview_list.scroll_ratio(),
                ws.preview_list.logical_scroll_top().item_ix,
            )
        });
        assert!(
            (state_ratio - ratio).abs() <= 0.03,
            "size-table ratio must match pointer ratio {ratio}, got {state_ratio}"
        );
        assert!(
            item > previous_item,
            "drag to {ratio} must advance past item {previous_item}, got {item}"
        );
        previous_item = item;
    }
    let last = Point {
        x: start.x,
        y: track.origin.y + track.size.height - px(2.),
    };
    cx.simulate_mouse_up(last, MouseButton::Left, Modifiers::none());
    settle(cx, 2);
    cx.update(|_, cx| assert!(view.read(cx).preview_scroll_drag.is_none()));
}

#[gpui::test]
async fn editor_bottom_wheel_keeps_preview_at_end(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(long_doc());
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    settle(cx, 6);

    let wheel_at = Point {
        x: px(160.),
        y: px(420.),
    };
    let (mut previous, max_scroll) = cx.update(|_, cx| {
        let ws = view.read(cx);
        (ws.preview_list.offset(), ws.preview_list.max_scroll())
    });
    let max_steps = ((max_scroll / 320.0).ceil() as usize + 40).min(140);
    let mut at_end_steps = 0usize;
    for _ in 0..max_steps {
        cx.simulate_event(ScrollEvent {
            position: wheel_at,
            delta: ScrollDelta::Pixels(point(px(0.), px(-320.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.run_until_parked();
        let (offset, max_now) = cx.update(|_, cx| {
            let ws = view.read(cx);
            (ws.preview_list.offset(), ws.preview_list.max_scroll())
        });
        assert!(
            offset + 2.0 >= previous,
            "preview jumped backwards at the document end: {previous} -> {offset}"
        );
        previous = offset;
        // Keep exercising the pinned-at-end path for a fixed 40 events instead
        // of wheeling hundreds of times past the end.
        if max_now > 0.5 && offset >= max_now - 1.0 {
            at_end_steps += 1;
            if at_end_steps >= 40 {
                break;
            }
        }
    }
    let (ratio, max_scroll) = cx.update(|_, cx| {
        let ws = view.read(cx);
        (ws.preview_list.scroll_ratio(), ws.preview_list.max_scroll())
    });
    assert!(
        max_scroll > 100.0,
        "long document must have a real scroll range"
    );
    assert!(
        ratio > 0.85,
        "editor at bottom must leave the preview near the end, got ratio {ratio}"
    );
}

/// Reproduces the reported jump: drag the preview scrollbar to the end, then
/// wheel in the editor. The preview must never be pulled backwards and must
/// stay pinned to its end instead of oscillating between mapped offsets.
#[gpui::test]
async fn preview_dragged_to_end_stays_pinned_while_editor_wheels(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(long_doc());
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    settle(cx, 6);

    let track = cx.debug_bounds("preview_scrollbar").unwrap();
    let thumb = cx.debug_bounds("preview_scrollbar_thumb").unwrap();
    let start = thumb.center();
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let end = Point {
        x: start.x,
        y: track.origin.y + track.size.height - px(2.),
    };
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    settle(cx, 3);
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    settle(cx, 3);

    let (initial, max_scroll) = cx.update(|_, cx| {
        let ws = view.read(cx);
        (ws.preview_list.offset(), ws.preview_list.max_scroll())
    });
    assert!(
        max_scroll - initial < 3.0,
        "drag must reach the preview end: {initial} of {max_scroll}"
    );

    let wheel_at = Point {
        x: px(160.),
        y: px(420.),
    };
    let mut previous = initial;
    for step in 0..40 {
        cx.simulate_event(ScrollEvent {
            position: wheel_at,
            delta: ScrollDelta::Pixels(point(px(0.), px(-320.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.run_until_parked();
        let offset = cx.update(|_, cx| view.read(cx).preview_list.offset());
        assert!(
            offset + 0.5 >= previous,
            "preview jumped backwards after the drag at step {step}: {previous} -> {offset}"
        );
        previous = offset;
    }
    assert!(
        max_scroll - previous < 1.5,
        "editor wheeling must leave the dragged preview at its end: {previous} of {max_scroll}"
    );
}

/// Preview wheel input must advance the visible range monotonically while real
/// block heights arrive and refine the size table.
#[gpui::test]
async fn preview_wheel_keeps_visible_range_monotonic_during_measurement(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(long_doc());
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    settle(cx, 6);

    let bounds = cx.update(|window, _| window.bounds());
    let wheel_at = Point {
        x: bounds.origin.x + bounds.size.width * 0.75,
        y: bounds.center().y,
    };
    let (mut previous_offset, mut previous_start) = cx.update(|_, cx| {
        let mut sample = None;
        let _ = view.update(cx, |ws, _| {
            let (range, _) = ws.preview_list.visible_range(32.0);
            sample = Some((ws.preview_list.offset(), range.start));
        });
        sample.unwrap()
    });

    for step in 0..20 {
        cx.simulate_event(ScrollEvent {
            position: wheel_at,
            delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        });
        for _ in 0..2 {
            cx.update(|window, cx| {
                let _ = window.draw(cx);
            });
            cx.run_until_parked();
        }
        let (offset, start, max) = cx.update(|_, cx| {
            let mut sample = None;
            let _ = view.update(cx, |ws, _| {
                let (range, _) = ws.preview_list.visible_range(32.0);
                sample = Some((
                    ws.preview_list.offset(),
                    range.start,
                    ws.preview_list.max_scroll(),
                ));
            });
            sample.unwrap()
        });
        assert!(
            offset + 0.5 >= previous_offset,
            "preview offset moved backward during downward wheel at step {step}: {previous_offset} -> {offset}"
        );
        assert!(
            start >= previous_start,
            "visible range moved back toward the top at step {step}: {previous_start} -> {start}"
        );
        previous_offset = offset;
        previous_start = start;
        if offset >= max - 1.0 {
            break;
        }
    }
}
