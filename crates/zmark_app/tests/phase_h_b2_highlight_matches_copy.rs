//! Phase H stage B · B2 lock-in (rendered highlight must equal the copied text).
//!
//! Reads the *rendered* highlight ranges (test-observation probe in
//! `render_styled_text_only`) and compares them with the production copy path
//! (`serialize_preview_selection`). A green result means the two production
//! paths agree; nothing here re-implements the parent/child offset mapping.
//!
//! Baseline e939a32: GREEN.
//! Isolation check: apply patches/0001-revert-b2-range-propagation.patch and
//! these tests must go RED (that is what makes them regressions rather than
//! decoration).

#![cfg(feature = "test-observation")]

use zmark_app::state::{AppState, ViewMode};
use zmark_app::views::preview::{
    obs, serialize_preview_selection, PreviewCharPosition, PreviewSelection,
};
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;
use gpui::{TestAppContext, VisualTestContext};

struct Harness {
    view: gpui::Entity<WorkspaceView>,
}

impl Harness {
    fn set_doc(&self, cx: &mut VisualTestContext, md: &str) {
        cx.update(|_w, cx| {
            self.view.update(cx, |ws, cx| {
                ws.state.editor.reset_text(md);
                ws.parse_now_and_refresh();
                ws.preview_selection = None;
                ws.preview_list.reset(ws.state.doc().parsed.blocks.len());
                obs::reset();
                cx.notify();
            });
        });
        let _ = cx.update(|window, cx| window.draw(cx));
        cx.run_until_parked();
    }

    fn flat_text(&self, cx: &mut VisualTestContext, block_ix: usize) -> String {
        cx.update(|_w, cx| self.view.read(cx).state.doc().parsed.blocks[block_ix].to_plain_text())
    }

    /// Selects `range` inside `block_ix`, repaints, and returns
    /// (rendered highlight runs, text produced by the production copy path).
    fn select_and_compare(
        &self,
        cx: &mut VisualTestContext,
        block_ix: usize,
        range: (usize, usize),
    ) -> (Vec<String>, String) {
        let sel = PreviewSelection::new(
            PreviewCharPosition::new(block_ix, range.0),
            PreviewCharPosition::new(block_ix, range.1),
        );
        let copied = cx.update(|_w, cx| {
            let ws = self.view.read(cx);
            serialize_preview_selection(&ws.state.doc().parsed, sel)
        });
        cx.update(|window, cx| {
            self.view.update(cx, |ws, cx| {
                ws.preview_selection = Some(sel);
                cx.notify();
            });
            obs::reset();
            let _ = window.draw(cx);
        });
        cx.run_until_parked();

        let mut runs: Vec<String> = Vec::new();
        for (text, range) in obs::highlights() {
            if let Some(r) = range {
                let s: String = text
                    .chars()
                    .skip(r.start)
                    .take(r.end.saturating_sub(r.start))
                    .collect();
                if !s.is_empty() && runs.last() != Some(&s) {
                    runs.push(s);
                }
            }
        }
        (runs, copied)
    }
}

fn open(cx: &mut TestAppContext) -> (Harness, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut d = Document::new_scratch();
        d.update_buffer(String::new());
        let mut state = AppState::new_isolated(d);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();
    (Harness { view }, cx)
}

fn offsets(h: &Harness, cx: &mut VisualTestContext, block_ix: usize, needle: &str) -> (usize, usize) {
    let flat = h.flat_text(cx, block_ix);
    let byte = flat
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found in flat block text {flat:?}"));
    let start = flat[..byte].chars().count();
    (start, start + needle.chars().count())
}

#[gpui::test]
async fn lock_two_level_nested_list(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    h.set_doc(cx, "- A\n  - B\n    - C deeper\n");
    let (s, e) = offsets(&h, cx, 0, "C deeper");
    let (runs, copied) = h.select_and_compare(cx, 0, (s, e));
    println!("B2 two-level nested: range={:?} copied={:?} highlight_runs={:?}", (s, e), copied, runs);
    assert_eq!(copied, "C deeper");
    assert_eq!(runs, vec!["C deeper".to_string()], "rendered highlight must match the copied substring");
}

#[gpui::test]
async fn lock_list_inside_blockquote(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    h.set_doc(cx, "> Quote line\n>\n> - inner item\n");
    let (s, e) = offsets(&h, cx, 0, "inner item");
    let (runs, copied) = h.select_and_compare(cx, 0, (s, e));
    println!("B2 quote->list: range={:?} copied={:?} highlight_runs={:?}", (s, e), copied, runs);
    assert_eq!(copied, "inner item");
    assert_eq!(runs, vec!["inner item".to_string()]);
}

#[gpui::test]
async fn lock_unicode_nested_item(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    h.set_doc(cx, "- parent\n  - 中文子项乙\n");
    let (s, e) = offsets(&h, cx, 0, "中文子项乙");
    let (runs, copied) = h.select_and_compare(cx, 0, (s, e));
    println!("B2 unicode nested: range={:?} copied={:?} highlight_runs={:?}", (s, e), copied, runs);
    assert_eq!(copied, "中文子项乙");
    assert_eq!(runs, vec!["中文子项乙".to_string()]);
}

#[gpui::test]
async fn lock_table_cell_partial(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    h.set_doc(cx, "| a | b |\n| --- | --- |\n| c1 | c2 |\n");
    let (s, _) = offsets(&h, cx, 0, "c2");
    let (runs, copied) = h.select_and_compare(cx, 0, (s, s + 2));
    println!("B2 table cell: range={:?} copied={:?} highlight_runs={:?}", (s, s + 2), copied, runs);
    assert_eq!(copied, "c2");
    assert_eq!(runs, vec!["c2".to_string()]);
}

#[gpui::test]
async fn lock_spanning_parent_and_child(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    h.set_doc(cx, "- Parent item\n  - Child A\n  - Child B\n");
    let (s, _) = offsets(&h, cx, 0, "Parent");
    let (_, e) = offsets(&h, cx, 0, "Child A");
    let (runs, copied) = h.select_and_compare(cx, 0, (s, e));
    println!("B2 spanning parent+child: range={:?} copied={:?} highlight_runs={:?}", (s, e), copied, runs);
    assert_eq!(copied, "Parent item\n  - Child A");
    // list markers are separate elements, so only the text runs are highlighted
    // Render order is not document order here (a sub-list is built before its
    // parent item element), so compare the runs order-insensitively.
    let mut sorted = runs.clone();
    sorted.sort();
    let mut expected = vec!["Parent item".to_string(), "Child A".to_string()];
    expected.sort();
    assert_eq!(sorted, expected, "each selected run must be highlighted exactly once");
}
