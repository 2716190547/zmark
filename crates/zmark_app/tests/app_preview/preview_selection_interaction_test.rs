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
        let mut state = AppState::new_isolated(d);
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

#[gpui::test]
async fn fast_click_and_missing_mouse_up_do_not_leave_preview_dragging(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let b = pane(&view, cx);
    let start = point(b.left() + px(60.), b.top() + px(60.));
    let end = point(b.left() + px(400.), b.top() + px(200.));

    // No redraw between press and release: handlers must use live state.
    for _ in 0..3 {
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
    }
    cx.run_until_parked();
    assert!(!cx.update(|_, cx| view.read(cx).is_preview_selecting));
    let after_click = selection(&view, cx);
    cx.simulate_mouse_move(end, None, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(selection(&view, cx), after_click, "hovering after quick clicks must not extend selection");

    // A mouse-up can be lost outside the window. The next unpressed move ends the drag.
    press(cx, start);
    let before_hover = selection(&view, cx);
    cx.simulate_mouse_move(end, None, Modifiers::none());
    cx.run_until_parked();
    assert!(!cx.update(|_, cx| view.read(cx).is_preview_selecting));
    assert_eq!(selection(&view, cx), before_hover, "an unpressed move must preserve the last selection");
}

#[gpui::test]
async fn escape_from_lightbox_cannot_resume_the_previous_text_drag(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    let b = pane(&view, cx);
    let start = point(b.left() + px(60.), b.top() + px(60.));
    press(cx, start);
    assert!(cx.update(|_, cx| view.read(cx).is_preview_selecting));
    let before = selection(&view, cx);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("image.png");
    image::RgbaImage::new(8, 8).save(&path).unwrap();
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.open_image_lightbox(path, window, cx);
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    // The modal receives the release that the text pane no longer sees.
    let card = cx.debug_bounds("lightbox_diagram_card").unwrap();
    release(cx, card.center());
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.simulate_mouse_move(point(b.left() + px(400.), b.top() + px(200.)), None, Modifiers::none());
    cx.run_until_parked();
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert!(!ws.lightbox.is_open);
        assert!(!ws.is_preview_selecting);
        assert!(ws.preview_autoscroll_task.is_none());
        assert!(ws.source_drag.is_none());
        assert_eq!(ws.preview_selection, before, "closing a lightbox must not turn hovering into text selection");
    });
}
