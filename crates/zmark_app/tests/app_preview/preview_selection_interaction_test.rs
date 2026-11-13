//! Preview text selection must behave like a browser: a press anywhere clears the old
//! selection, and a drag that starts in empty space still selects the nearest text.

use gpui::{point, px, Modifiers, MouseButton, Point, TestAppContext, VisualTestContext};
use zmark_app::state::{AppState, ViewMode};
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;

fn open(cx: &mut TestAppContext) -> (gpui::Entity<WorkspaceView>, &mut VisualTestContext) {
    let mut md = String::new();
    for i in 0..4 {
        md.push_str(&format!("## Block {i}\n\nParagraph body for block {i} with several words to click on here.\n\n"));
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
    (view, cx)
}

fn pane(view: &gpui::Entity<WorkspaceView>, cx: &mut VisualTestContext) -> gpui::Bounds<gpui::Pixels> {
    cx.update(|_, cx| view.read(cx).preview_content_bounds).expect("pane bounds reported")
}

fn selection(view: &gpui::Entity<WorkspaceView>, cx: &mut VisualTestContext) -> Option<zmark_app::views::preview::PreviewSelectionRange> {
    cx.update(|_, cx| view.read(cx).preview_selection)
}

fn press(cx: &mut VisualTestContext, p: Point<gpui::Pixels>) {
    cx.simulate_mouse_down(p, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn release(cx: &mut VisualTestContext, p: Point<gpui::Pixels>) {
    cx.simulate_mouse_up(p, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
async fn pressing_in_the_side_margin_starts_a_selection_from_the_nearest_text(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let b = pane(&view, cx);
    // Far left of the column: inside the pane but outside every text block.
    let margin = point(b.left() + px(3.), b.top() + px(80.));
    press(cx, margin);
    let sel = selection(&view, cx).expect("a press in the margin starts a selection at the nearest text");
    assert_eq!(sel.anchor, sel.head, "a bare press is a collapsed caret, not a range");
    release(cx, margin);
}

#[gpui::test]
async fn dragging_from_empty_space_selects_text(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let b = pane(&view, cx);
    let start = point(b.left() + px(3.), b.top() + px(60.));
    let end = point(b.left() + px(400.), b.top() + px(200.));
    press(cx, start);
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let sel = selection(&view, cx).expect("dragging out of the margin selects text");
    assert_ne!(sel.anchor, sel.head, "the drag must extend the selection");
    release(cx, end);
}

#[gpui::test]
async fn pressing_elsewhere_clears_an_existing_selection(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let b = pane(&view, cx);
    let (a, z) = (point(b.left() + px(60.), b.top() + px(60.)), point(b.left() + px(500.), b.top() + px(200.)));
    press(cx, a);
    cx.simulate_mouse_move(z, MouseButton::Left, Modifiers::none());
    release(cx, z);
    let before = selection(&view, cx).expect("a range is selected");
    assert_ne!(before.anchor, before.head);

    // A plain click in empty space must collapse it to a caret (nothing highlighted).
    let empty = point(b.left() + px(3.), b.bottom() - px(4.));
    press(cx, empty);
    release(cx, empty);
    let after = selection(&view, cx);
    assert!(
        after.is_none_or(|s| s.anchor == s.head),
        "a click elsewhere must leave no highlighted range, got {after:?}"
    );
}
