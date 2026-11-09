//! Phase H stage B · Cmd+C routing counterexamples (focus + view mode gating).
//!
//! Drives the real key path (`simulate_keystrokes("secondary-c")` through the
//! Workspace key handler) and reads the real clipboard via
//! `TestAppContext::read_from_clipboard`.
//!
//! Baseline e939a32: case 1 (Split, editor focused) FAILS; cases 2..4 PASS.

use zmark_app::state::{AppState, ViewMode};
use zmark_app::views::preview::{PreviewCharPosition, PreviewSelection, serialize_preview_selection};
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;
use gpui::{ClipboardItem, TestAppContext, VisualTestContext};

const SENTINEL: &str = "SENTINEL-NOT-COPIED";
const MD: &str = "# Title one\n\nSecond paragraph text\n";

struct Harness {
    view: gpui::Entity<WorkspaceView>,
}

fn open(cx: &mut TestAppContext, mode: ViewMode) -> (Harness, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut d = Document::new_scratch();
        d.update_buffer(MD.to_string());
        let mut state = AppState::new(d);
        state.set_view_mode(mode);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.parse_now_and_refresh();
            // a stale preview selection that stays set regardless of focus
            ws.preview_selection = Some(PreviewSelection::new(
                PreviewCharPosition::new(0, 0),
                PreviewCharPosition::new(0, 5),
            ));
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    (Harness { view }, cx)
}

impl Harness {
    fn focus_workspace(&self, cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            let handle = self.view.read(cx).focus_handle.clone();
            window.focus(&handle);
        });
        cx.run_until_parked();
    }

    fn seed_clipboard(&self, cx: &mut VisualTestContext) {
        cx.cx.update(|app| app.write_to_clipboard(ClipboardItem::new_string(SENTINEL.to_string())));
    }

    fn clipboard(&self, cx: &mut VisualTestContext) -> Option<String> {
        cx.read_from_clipboard().and_then(|i| i.text())
    }

    fn preview_copy_text(&self, cx: &mut VisualTestContext) -> String {
        cx.update(|_w, cx| {
            let ws = self.view.read(cx);
            serialize_preview_selection(&ws.state.doc().parsed, ws.preview_selection.unwrap())
        })
    }
}

// ---------------------------------------------------------------------------
// RED: Split view, editor focused, stale preview selection present.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn red_split_editor_focused_must_not_copy_preview_selection(cx: &mut TestAppContext) {
    let (h, cx) = open(cx, ViewMode::Split);
    h.focus_workspace(cx);

    // precondition: typing reaches the source buffer, i.e. the editor owns focus
    cx.simulate_keystrokes("x");
    cx.run_until_parked();
    let editor_text = cx.update(|_w, cx| h.view.read(cx).state.editor.to_text());
    assert!(
        editor_text.contains('x'),
        "precondition: with the workspace focused, keystrokes must reach the editor (buffer={editor_text:?})"
    );

    h.seed_clipboard(cx);
    cx.simulate_keystrokes("secondary-c");
    cx.run_until_parked();
    let got = h.clipboard(cx);
    let preview_text = h.preview_copy_text(cx);
    println!(
        "B4-RED split + editor focused: clipboard={:?} stale preview selection would copy={:?}",
        got, preview_text
    );
    assert_ne!(
        got.as_deref(),
        Some(preview_text.as_str()),
        "Cmd+C with the editor focused must not copy the stale preview selection ({preview_text:?})"
    );
}

// ---------------------------------------------------------------------------
// GREEN: preview mode copies exactly the selected range.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn green_preview_mode_copies_exact_selection(cx: &mut TestAppContext) {
    let (h, cx) = open(cx, ViewMode::Preview);
    h.focus_workspace(cx);
    h.seed_clipboard(cx);
    cx.simulate_keystrokes("secondary-c");
    cx.run_until_parked();
    let got = h.clipboard(cx);
    println!("B4-GREEN preview mode: clipboard={:?} (expected \"Title\")", got);
    assert_eq!(got.as_deref(), Some("Title"), "Preview mode must copy exactly the selected range");
}

// ---------------------------------------------------------------------------
// GREEN: source mode never copies a preview selection.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn green_source_mode_never_copies_preview_selection(cx: &mut TestAppContext) {
    let (h, cx) = open(cx, ViewMode::Source);
    h.focus_workspace(cx);
    h.seed_clipboard(cx);
    cx.simulate_keystrokes("secondary-c");
    cx.run_until_parked();
    let got = h.clipboard(cx);
    println!("B4-GREEN source mode: clipboard={:?}", got);
    assert_ne!(got.as_deref(), Some("Title"), "Source mode must not copy the preview selection");
}

// ---------------------------------------------------------------------------
// GREEN: the search box owns the copy shortcut while focused.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn green_search_focus_copies_query(cx: &mut TestAppContext) {
    let (h, cx) = open(cx, ViewMode::Split);
    h.focus_workspace(cx);
    cx.simulate_keystrokes("cmd-f");
    cx.run_until_parked();
    h.seed_clipboard(cx);
    cx.simulate_keystrokes("secondary-c");
    cx.run_until_parked();
    let got = h.clipboard(cx);
    let search_focused = cx.update(|_w, cx| h.view.read(cx).search_focused);
    println!("B4-GREEN search focused: clipboard={:?} search_focused={}", got, search_focused);
    assert!(
        !(search_focused && got.as_deref() == Some("Title")),
        "with the search box focused Cmd+C must not copy the preview selection"
    );
}
