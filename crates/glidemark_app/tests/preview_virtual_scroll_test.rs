//! Size-table preview scrollbar regressions: the thumb must follow the pointer
//! all the way through an unmeasured long document, and editor scrolling to the
//! bottom must not pull the preview back into the measured region.

use glidemark_app::state::{AppState, ViewMode};
use glidemark_app::views::workspace::WorkspaceView;
use glidemark_core::document::Document;
use gpui::{
    point, px, Modifiers, MouseButton, Point, ScrollDelta, ScrollWheelEvent, TestAppContext,
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
        let target = Point { x: start.x, y: target_y };
        cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
        settle(cx, 3);
        let actual = cx.debug_bounds("preview_scrollbar_thumb").unwrap().center().y;
        assert!(
            (f32::from(actual - target_y)).abs() <= 2.0,
            "thumb must follow pointer at ratio {ratio}: thumb {actual:?}, pointer {target_y:?}"
        );
        let (state_ratio, item) = cx.update(|_, cx| {
            let ws = view.read(cx);
            (ws.preview_list.scroll_ratio(), ws.preview_list.logical_scroll_top().item_ix)
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
    let last = Point { x: start.x, y: track.origin.y + track.size.height - px(2.) };
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

    let wheel_at = Point { x: px(160.), y: px(420.) };
    let mut previous = cx.update(|_, cx| view.read(cx).preview_list.offset());
    for _ in 0..140 {
        cx.simulate_event(ScrollWheelEvent {
            position: wheel_at,
            delta: ScrollDelta::Pixels(point(px(0.), px(-320.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.run_until_parked();
        let offset = cx.update(|_, cx| view.read(cx).preview_list.offset());
        assert!(
            offset + 2.0 >= previous,
            "preview jumped backwards at the document end: {previous} -> {offset}"
        );
        previous = offset;
    }
    let (ratio, max_scroll) = cx.update(|_, cx| {
        let ws = view.read(cx);
        (ws.preview_list.scroll_ratio(), ws.preview_list.max_scroll())
    });
    assert!(max_scroll > 100.0, "long document must have a real scroll range");
    assert!(
        ratio > 0.85,
        "editor at bottom must leave the preview near the end, got ratio {ratio}"
    );
}
