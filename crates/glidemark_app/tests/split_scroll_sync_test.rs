//! Split-mode scroll synchronization: the editor pane must never drive the preview
//! past its own end, and in-block offsets must scale with real block heights.

use glidemark_app::state::{AppState, ViewMode};
use glidemark_app::views::workspace::WorkspaceView;
use glidemark_core::document::Document;
use gpui::{
    point, px, Modifiers, Point, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase,
};

fn long_doc() -> String {
    (0..120)
        .map(|n| format!("## 小节 {n}\n\n第 {n} 段正文内容，用来撑高文档高度。\n\n~~~python\nprint({n})\n~~~\n\n"))
        .collect()
}

#[gpui::test]
async fn editor_overscroll_never_sends_preview_back_to_start(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(long_doc());
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

    let bounds = cx.update(|window, _| window.bounds());
    let wheel_at = Point {
        x: bounds.origin.x + bounds.size.width * 0.25,
        y: bounds.center().y,
    };
    let (count, editor_max) = cx.update(|_, cx| {
        let ws = view.read(cx);
        let max = f32::from(
            ws.editor_scroll_handle
                .0
                .borrow()
                .base_handle
                .max_offset()
                .height,
        )
        .max(0.0);
        (ws.state.doc().parsed.blocks.len(), max)
    });
    let max_steps = ((editor_max / 500.0).ceil() as usize + 40).min(400);

    let mut prev_item = 0usize;
    let mut min_after_end = usize::MAX;
    let mut saw_end = false;
    let mut overscroll_steps = 0usize;
    // Scroll until the preview reaches the last tenth, then keep wheeling for a
    // fixed 40 events so the end-of-document echo path is still exercised. The
    // old fixed 400 iterations spent most of their time long past the end.
    for _ in 0..max_steps {
        let wheel = ScrollWheelEvent {
            position: wheel_at,
            delta: ScrollDelta::Pixels(point(px(0.), px(-500.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        };
        cx.simulate_event(wheel);
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let item = cx.update(|_, cx| view.read(cx).preview_list.logical_scroll_top().item_ix);
        assert!(
            item + 4 >= prev_item,
            "preview must never jump backwards while the editor scrolls down ({prev_item} -> {item})"
        );
        if item >= count * 9 / 10 {
            saw_end = true;
            min_after_end = min_after_end.min(item);
            overscroll_steps += 1;
        }
        prev_item = item;
        if saw_end && overscroll_steps >= 40 {
            break;
        }
    }

    assert!(saw_end, "scrolling the editor must reach the document end");
    assert!(
        min_after_end >= count * 9 / 10,
        "after reaching the end the preview must stay at the end, not fall back to {min_after_end} of {count}"
    );
}

#[gpui::test]
async fn in_block_offset_uses_measured_block_height(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer(long_doc());
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

    let bounds = cx.update(|window, _| window.bounds());
    let wheel_at = Point {
        x: bounds.origin.x + bounds.size.width * 0.25,
        y: bounds.center().y,
    };

    // Scroll a little way into the document, then collect in-block offsets.
    let mut offsets = Vec::new();
    for _ in 0..40 {
        let wheel = ScrollWheelEvent {
            position: wheel_at,
            delta: ScrollDelta::Pixels(point(px(0.), px(-37.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        };
        cx.simulate_event(wheel);
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        let (item, offset, height) = cx.update(|_, cx| {
            let ws = view.read(cx);
            let top = ws.preview_list.logical_scroll_top();
            let h = ws
                .preview_list
                .bounds_for_item(top.item_ix)
                .map(|b| f32::from(b.size.height))
                .unwrap_or(0.0);
            (top.item_ix, f32::from(top.offset_in_item), h)
        });
        if height > 40.0 {
            offsets.push((item, offset, height));
        }
    }

    assert!(
        offsets.len() > 5,
        "expected several measured in-block offsets"
    );
    assert!(
        offsets
            .iter()
            .all(|(_, offset, height)| *offset <= height + 1.0),
        "in-block offset must stay inside the measured block height: {offsets:?}"
    );
    // A fixed 100px scale can only ever land on multiples of 25 (0/25/50/75/100).
    // Proportional offsets derived from real heights do not.
    assert!(
        offsets
            .iter()
            .any(|(_, offset, height)| { *height > 40.0 && (*offset % 25.0).abs() > 0.01 }),
        "in-block offsets must come from the measured block height: {offsets:?}"
    );
}
