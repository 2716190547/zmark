//! Phase H stage B · B3 counterexamples (real pane, real mouse events, time-driven).
//!
//! No bounds are hand-set: the pane rect comes from the production canvas field
//! `WorkspaceView::preview_content_bounds`, and "the block under the pointer" is
//! measured by the production hit test itself (press at that point, read the
//! anchor). Time advances via `TestAppContext::executor().advance_clock`, i.e.
//! the pointer never moves while the product timer keeps ticking.
//!
//! Baseline e939a32: `red_edge_autoscroll_head_must_follow_pointer` FAILS,
//! `green_autoscroll_stops_on_mouse_up` PASSES, the focus-loss and
//! document-end cases FAIL (feature missing).

use zmark_app::state::{AppState, ViewMode};
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;
use gpui::{px, Modifiers, MouseButton, Point, TestAppContext, VisualTestContext};
use std::time::Duration;

struct Harness {
    view: gpui::Entity<WorkspaceView>,
}

fn open(cx: &mut TestAppContext, blocks: usize) -> (Harness, &mut VisualTestContext) {
    let mut md = String::new();
    for i in 0..blocks {
        md.push_str(&format!(
            "## Block {}\n\nParagraph body for block {} with several words to click on here.\n\n",
            i, i
        ));
    }
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut d = Document::new_scratch();
        d.update_buffer(md);
        let mut state = AppState::new(d);
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
    cx.run_until_parked();
    (Harness { view }, cx)
}

impl Harness {
    fn pane(&self, cx: &mut VisualTestContext) -> gpui::Bounds<gpui::Pixels> {
        cx.update(|_w, cx| self.view.read(cx).preview_content_bounds)
            .expect("production preview pane must have reported its bounds")
    }

    fn press(&self, cx: &mut VisualTestContext, p: Point<gpui::Pixels>) {
        cx.simulate_mouse_down(p, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }

    fn release(&self, cx: &mut VisualTestContext, p: Point<gpui::Pixels>) {
        cx.simulate_mouse_up(p, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }

    fn tick(&self, cx: &mut VisualTestContext, ms: u64) {
        cx.executor().advance_clock(Duration::from_millis(ms));
        cx.run_until_parked();
        let _ = cx.update(|window, cx| window.draw(cx));
        cx.run_until_parked();
    }

    fn state(&self, cx: &mut VisualTestContext) -> State {
        cx.update(|_w, cx| {
            let ws = self.view.read(cx);
            let top = ws.preview_list.logical_scroll_top();
            State {
                selecting: ws.is_preview_selecting,
                anchor: ws.preview_selection.map(|s| (s.anchor.block_ix, s.anchor.char_offset)),
                head: ws.preview_selection.map(|s| (s.head.block_ix, s.head.char_offset)),
                top_item: top.item_ix,
                top_offset: f32::from(top.offset_in_item),
                task_active: ws.preview_autoscroll_task.is_some(),
            }
        })
    }

    fn flat_len(&self, cx: &mut VisualTestContext, block_ix: usize) -> usize {
        cx.update(|_w, cx| {
            self.view
                .read(cx)
                .state
                .doc()
                .parsed
                .blocks
                .get(block_ix)
                .map(|b| b.to_plain_text().chars().count())
                .unwrap_or(0)
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct State {
    selecting: bool,
    anchor: Option<(usize, usize)>,
    head: Option<(usize, usize)>,
    top_item: usize,
    top_offset: f32,
    task_active: bool,
}

// ---------------------------------------------------------------------------
// RED: pointer parked at the bottom edge, no mouse movement, product timer ticks.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn red_edge_autoscroll_head_must_follow_pointer(cx: &mut TestAppContext) {
    let (h, cx) = open(cx, 40);
    let pane = h.pane(cx);
    let edge = Point {
        x: px(f32::from(pane.origin.x) + 700.0),
        y: px(f32::from(pane.origin.y + pane.size.height) - 6.0),
    };

    // (1) "block under the pointer" measured by the production hit test
    h.press(cx, edge);
    let under_pointer = h.state(cx).anchor.expect("a press on a block must produce an anchor");
    h.release(cx, edge);
    println!("B3 block under the bottom-edge pointer (production hit test) = {:?}", under_pointer);

    // (2) start a selection in a paragraph, then park the pointer at the edge
    let start = Point {
        x: px(f32::from(pane.origin.x) + 700.0),
        y: px(f32::from(pane.origin.y) + 120.0),
    };
    h.press(cx, start);
    let s0 = h.state(cx);
    assert!(s0.selecting, "pressing inside the pane must start a selection: {s0:?}");
    cx.simulate_mouse_move(edge, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let s1 = h.state(cx);
    println!("B3 press {:?} then drag to the edge: {s1:?}", s0.anchor);
    assert!(s1.task_active, "holding the pointer at the edge must start the autoscroll task");

    // (3) pointer stays still; only time advances
    for _ in 0..10 {
        h.tick(cx, 16);
    }
    let s2 = h.state(cx);
    let head_len = s2.head.map(|(ix, _)| h.flat_len(cx, ix)).unwrap_or(0);
    println!(
        "B3 after 10 ticks with a still pointer: top_item={} top_offset={:.2} head={:?} flat_len(head block)={} task={}",
        s2.top_item, s2.top_offset, s2.head, head_len, s2.task_active
    );

    assert_ne!(
        s2.head, s1.head,
        "with the pointer parked at the edge the head must keep following newly revealed text (head did not move: {:?})",
        s2.head
    );
    assert!(
        s2.top_item > s1.top_item,
        "after 10 ticks of edge autoscroll the list must have crossed at least one item (top_item {} -> {})",
        s1.top_item,
        s2.top_item
    );
    assert!(
        s2.head
            .map(|(bix, _)| (bix as i32 - under_pointer.0 as i32).abs() <= 1)
            .unwrap_or(false),
        "the head must sit on the block under the pointer (pointer block {}), got {:?}",
        under_pointer.0,
        s2.head
    );
    assert!(
        s2.head.map(|(_, c)| c != head_len).unwrap_or(false),
        "the head must be the character under the pointer, not the full length of a guessed block (len {head_len}, head {:?})",
        s2.head
    );
}

// ---------------------------------------------------------------------------
// GREEN: releasing the mouse terminates the task and nothing keeps scrolling.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn green_autoscroll_stops_on_mouse_up(cx: &mut TestAppContext) {
    let (h, cx) = open(cx, 40);
    let pane = h.pane(cx);
    let edge = Point {
        x: px(f32::from(pane.origin.x) + 700.0),
        y: px(f32::from(pane.origin.y + pane.size.height) - 6.0),
    };
    let start = Point {
        x: px(f32::from(pane.origin.x) + 700.0),
        y: px(f32::from(pane.origin.y) + 120.0),
    };
    h.press(cx, start);
    cx.simulate_mouse_move(edge, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    for _ in 0..3 {
        h.tick(cx, 16);
    }
    h.release(cx, edge);
    let after = h.state(cx);
    println!("B3-GREEN after mouse_up: {after:?}");
    assert!(!after.selecting, "mouse_up must end the selection");
    assert!(!after.task_active, "mouse_up must drop the autoscroll task");
    for _ in 0..10 {
        h.tick(cx, 16);
    }
    let later = h.state(cx);
    assert_eq!(
        (later.top_item, later.top_offset.to_bits()),
        (after.top_item, after.top_offset.to_bits()),
        "after mouse_up the list must not keep scrolling"
    );
}

// ---------------------------------------------------------------------------
// RED: losing focus must cancel the drag/autoscroll.
// gpui 0.2.2 exposes no public API to simulate an NSWindow deactivation, so the
// closest public signal is moving focus to another handle. Production has no
// focus-loss listener at all (see MANIFEST.md), so the task keeps running.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn red_autoscroll_must_stop_when_focus_is_lost(cx: &mut TestAppContext) {
    let (h, cx) = open(cx, 40);
    let pane = h.pane(cx);
    let edge = Point {
        x: px(f32::from(pane.origin.x) + 700.0),
        y: px(f32::from(pane.origin.y + pane.size.height) - 6.0),
    };
    let start = Point {
        x: px(f32::from(pane.origin.x) + 700.0),
        y: px(f32::from(pane.origin.y) + 120.0),
    };
    h.press(cx, start);
    cx.simulate_mouse_move(edge, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    for _ in 0..2 {
        h.tick(cx, 16);
    }
    let before = h.state(cx);
    cx.update(|window, cx| {
        let other = cx.focus_handle();
        window.focus(&other);
    });
    cx.run_until_parked();
    for _ in 0..10 {
        h.tick(cx, 16);
    }
    let after = h.state(cx);
    println!("B3-RED focus lost: before={before:?} after={after:?}");
    assert!(
        !after.task_active && !after.selecting,
        "losing focus must cancel preview selection and autoscroll (before={before:?}, after={after:?})"
    );
}

// ---------------------------------------------------------------------------
// RED: holding at the bottom edge at the end of the document must not spin.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn red_autoscroll_must_stop_at_document_end(cx: &mut TestAppContext) {
    let (h, cx) = open(cx, 40);
    cx.update(|_w, cx| {
        h.view.update(cx, |ws, _cx| {
            let count = ws.state.doc().parsed.blocks.len();
            ws.preview_list.scroll_to(gpui::ListOffset {
                item_ix: count.saturating_sub(2),
                offset_in_item: px(0.),
            });
        });
    });
    cx.run_until_parked();
    let _ = cx.update(|window, cx| window.draw(cx));
    cx.run_until_parked();
    let pane = h.pane(cx);
    let edge = Point {
        x: px(f32::from(pane.origin.x) + 700.0),
        y: px(f32::from(pane.origin.y + pane.size.height) - 6.0),
    };
    // press inside the pane (a real block) so a selection actually starts
    let mid = Point {
        x: px(f32::from(pane.origin.x) + 700.0),
        y: px(f32::from(pane.origin.y) + f32::from(pane.size.height) / 2.0),
    };
    h.press(cx, mid);
    let s0 = h.state(cx);
    assert!(s0.selecting, "precondition: pressing inside the pane must start a selection: {s0:?}");
    cx.simulate_mouse_move(edge, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    for _ in 0..40 {
        h.tick(cx, 16);
    }
    let s1 = h.state(cx);
    println!("B3 boundary: pressed mid-pane at end-of-document {s0:?} -> dragged to edge, 40 ticks -> {s1:?}");
    assert!(
        !s1.task_active,
        "at the end of the document the autoscroll task must stop instead of spinning (state {s1:?})"
    );
}
