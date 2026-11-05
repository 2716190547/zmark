//! Phase H stage B · B1 counterexamples (click mapping must use real rendered geometry).
//!
//! Every `expected` value is derived from the *rendered* text container bounds
//! (`preview::obs::text_bounds`, recorded by the canvas probe inserted by
//! patches/0000-test-observation.patch) plus glyph positions from the same
//! `WindowTextSystem` the renderer uses. No click position is produced by
//! re-implementing the production helper, and no bounds are hand-filled.
//!
//! Baseline e939a32: `red_*` cases FAIL, `control_*` cases PASS.

#![cfg(feature = "test-observation")]

use glidemark_app::state::{AppState, ViewMode};
use glidemark_app::views::preview::obs;
use glidemark_app::views::workspace::WorkspaceView;
use glidemark_core::document::Document;
use gpui::{px, Modifiers, MouseButton, Point, TestAppContext, VisualTestContext};

struct Harness {
    view: gpui::Entity<WorkspaceView>,
}

impl Harness {
    /// Replaces the document in the already-open window (keeps a single window,
    /// so geometry is always produced by the real renderer for the current doc).
    fn set_doc(&self, cx: &mut VisualTestContext, md: &str) {
        cx.update(|_w, cx| {
            self.view.update(cx, |ws, cx| {
                ws.state.editor.reset_text(md);
                ws.parse_now_and_refresh();
                ws.preview_selection = None;
                ws.preview_list.reset(ws.state.doc.parsed.blocks.len());
                obs::reset();
                cx.notify();
            });
        });
        draw(cx);
    }

    fn click(&self, cx: &mut VisualTestContext, point: Point<gpui::Pixels>) -> Option<(usize, usize)> {
        cx.update(|_w, cx| {
            self.view.update(cx, |ws, _cx| ws.preview_selection = None);
        });
        cx.simulate_mouse_down(point, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let got = cx.update(|_w, cx| {
            self.view
                .read(cx)
                .preview_selection
                .map(|s| (s.anchor.block_ix, s.anchor.char_offset))
        });
        cx.simulate_mouse_up(point, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        got
    }

    fn flat_text(&self, cx: &mut VisualTestContext, block_ix: usize) -> String {
        cx.update(|_w, cx| self.view.read(cx).state.doc.parsed.blocks[block_ix].to_plain_text())
    }
}

fn draw(cx: &mut VisualTestContext) {
    let _ = cx.update(|window, cx| window.draw(cx));
    cx.run_until_parked();
}

fn open(cx: &mut TestAppContext) -> (Harness, &mut VisualTestContext) {
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let mut d = Document::new_scratch();
        d.update_buffer(String::new());
        let mut state = AppState::new(d);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();
    (Harness { view }, cx)
}

/// Shaped advance widths of `text` at `font_size`, from the real text system.
fn glyph_xs(cx: &mut VisualTestContext, text: &str, font_size: f32) -> Vec<f32> {
    cx.update(|window, _app| {
        let ts = window.text_system();
        let bytes: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        let run = gpui::TextRun {
            len: text.len(),
            font: gpui::font("ui-sans-serif, system-ui"),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = ts.shape_line(
            gpui::SharedString::from(text.to_string()),
            px(font_size),
            &[run],
            None,
        );
        bytes.iter().map(|&b| f32::from(shaped.x_for_index(b))).collect()
    })
}

fn char_at(glyph_xs: &[f32], x: f32) -> usize {
    let mut idx = 0;
    for (k, gx) in glyph_xs.iter().enumerate() {
        if *gx <= x {
            idx = k + 1;
        }
    }
    idx.min(glyph_xs.len())
}

fn bounds_of(key: &str) -> (f32, f32, f32, f32) {
    obs::text_bounds(key).unwrap_or_else(|| panic!("no rendered text container recorded for {key:?}"))
}

fn point_at(b: (f32, f32, f32, f32), dx: f32, dy: f32) -> Point<gpui::Pixels> {
    Point { x: px(b.0 + dx), y: px(b.1 + dy) }
}

// ---------------------------------------------------------------------------
// GREEN control 1: plain paragraph maps the click to the glyph under the pointer.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn control_plain_paragraph(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    let para = "Plain paragraph with many words to click into here";
    h.set_doc(cx, &format!("{}\n", para));
    let b = bounds_of(para);
    let xs = glyph_xs(cx, para, 16.0);
    let dx = xs[xs.len() / 2];
    let expected = char_at(&xs, dx);
    let got = h.click(cx, point_at(b, dx, b.3 / 2.0)).expect("click must produce a selection");
    println!(
        "B1-GREEN paragraph: text_container=({:.0},{:.0},{:.0},{:.0}) dx={:.1} expected_char={} got={:?}",
        b.0, b.1, b.2, b.3, dx, expected, got
    );
    assert!(
        (got.1 as i32 - expected as i32).abs() <= 1,
        "plain paragraph must map the click to the glyph under the pointer (expected {expected}, got {})",
        got.1
    );
}

// ---------------------------------------------------------------------------
// GREEN control 2: same invariant with a non-zero viewport origin (after scroll).
// ---------------------------------------------------------------------------
#[gpui::test]
async fn control_scrolled_paragraph(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    let mut md = String::new();
    for i in 0..12 {
        md.push_str(&format!("## Heading {}\n\nParagraph body number {} stays short.\n\n", i, i));
    }
    h.set_doc(cx, &md);
    cx.update(|_w, cx| {
        h.view.update(cx, |ws, _cx| {
            ws.preview_list.scroll_to(gpui::ListOffset { item_ix: 10, offset_in_item: px(0.) });
        });
    });
    cx.run_until_parked();
    obs::reset();
    draw(cx);

    let visible = (0..12)
        .map(|i| format!("Paragraph body number {} stays short.", i))
        .find(|k| obs::text_bounds(k).is_some())
        .expect("at least one paragraph must be rendered after scrolling");
    let block_ix = cx.update(|_w, cx| {
        h.view
            .read(cx)
            .state
            .doc
            .parsed
            .blocks
            .iter()
            .position(|blk| blk.to_plain_text() == visible)
            .unwrap()
    });
    let b = bounds_of(&visible);
    let xs = glyph_xs(cx, &visible, 16.0);
    let dx = xs[xs.len() / 4];
    let expected = char_at(&xs, dx);
    let got = h.click(cx, point_at(b, dx, b.3 / 2.0)).expect("click must produce a selection");
    println!(
        "B1-GREEN scrolled paragraph: bounds=({:.0},{:.0}) dx={:.1} expected=(block {}, char {}) got={:?}",
        b.0, b.1, dx, block_ix, expected, got
    );
    assert_eq!(got.0, block_ix, "scrolled click must hit the block under the pointer");
    assert!(
        (got.1 as i32 - expected as i32).abs() <= 1,
        "scrolled paragraph must map to the glyph under the pointer (expected {expected}, got {})",
        got.1
    );
}

// ---------------------------------------------------------------------------
// RED 1: H1 is rendered at 1.8x while the mapping measures the text at base size.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn red_heading_uses_base_font_size(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    let title = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    h.set_doc(cx, &format!("# {}\n", title));
    let b = bounds_of(title);
    // Heading render (preview.rs Heading branch): text_size(base * 1.8), bold.
    let xs = glyph_xs(cx, title, 16.0 * 1.8);
    let dx = xs[xs.len() / 2];
    let expected = char_at(&xs, dx);
    let got = h.click(cx, point_at(b, dx, 12.0)).expect("click must produce a selection");
    println!(
        "B1-RED heading(1.8x): text_container=({:.0},{:.0},{:.0},{:.0}) click_local_x={:.1} expected_char={} got={:?}",
        b.0, b.1, b.2, b.3, dx, expected, got
    );
    assert!(
        (got.1 as i32 - expected as i32).abs() <= 1,
        "clicking the middle of an H1 must select the glyph under the pointer \
         (real layout at 1.8x => char {expected}, production returned char {})",
        got.1
    );
}

// ---------------------------------------------------------------------------
// RED 2: blockquote text starts 16px (4px border + 12px padding) inside the block.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn red_blockquote_padding_offset(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    let text = "Quoted text for the quote block sample";
    h.set_doc(cx, &format!("> {}\n", text));
    let b = bounds_of(text);
    let got = h.click(cx, point_at(b, 3.0, b.3 / 2.0)).expect("click must produce a selection");
    println!(
        "B1-RED blockquote: text_container=({:.0},{:.0}) click 3px into first glyph => got={:?} (expected char 0..=1)",
        b.0, b.1, got
    );
    assert!(
        got.1 <= 1,
        "clicking 3px into the first glyph of a blockquote must select char 0..=1, got {}",
        got.1
    );
}

// ---------------------------------------------------------------------------
// RED 3: checked list item — rendered marker (14px + 8px gap) vs flat prefix "- [x] ".
// ---------------------------------------------------------------------------
#[gpui::test]
async fn red_checked_list_prefix_offset(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    let content = "Checked task content here";
    h.set_doc(cx, &format!("- [x] {}\n", content));
    let expected = h.flat_text(cx, 0).find(content).expect("content in flat text");
    let b = bounds_of(content);
    let got = h.click(cx, point_at(b, 3.0, b.3 / 2.0)).expect("click must produce a selection");
    println!(
        "B1-RED checked list: content_container=({:.0},{:.0}) flat_prefix_len={} click 3px into content => got={:?}",
        b.0, b.1, expected, got
    );
    assert!(
        (got.1 as i32 - expected as i32).abs() <= 1,
        "clicking the first glyph of a checked item must return the flat offset of its content ({expected}), got {}",
        got.1
    );
}

// ---------------------------------------------------------------------------
// RED 4: nested list child — click on the child's own rendered line.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn lock_nested_list_child_offset(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    let child = "Child item text";
    h.set_doc(cx, "- Parent item text\n  - Child item text\n");
    let expected = h.flat_text(cx, 0).find(child).expect("child text in flat block text");
    let b = bounds_of(child);
    let got = h.click(cx, point_at(b, 3.0, b.3 / 2.0)).expect("click must produce a selection");
    println!(
        "B1-LOCK nested list (this shape already maps correctly): child_container=({:.0},{:.0}) flat_offset_of_child={} click first glyph => got={:?}",
        b.0, b.1, expected, got
    );
    assert!(
        (got.1 as i32 - expected as i32).abs() <= 1,
        "clicking the first glyph of a nested child must return flat offset {expected}, got {}",
        got.1
    );
}

// ---------------------------------------------------------------------------
// RED 5: table — columns are side by side, the mapping flattens them into one line.
// ---------------------------------------------------------------------------
#[gpui::test]
async fn red_table_second_column(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    h.set_doc(cx, "| Name | Description |\n| --- | --- |\n| Alpha | Beta gamma |\n");
    let expected = h.flat_text(cx, 0).find("Description").expect("col2 in flat text");
    let b = bounds_of("Description");
    let got = h.click(cx, point_at(b, 3.0, b.3 / 2.0)).expect("click must produce a selection");
    println!(
        "B1-RED table col2: col2_container=({:.0},{:.0}) flat_offset_of_col2={} click first glyph => got={:?}",
        b.0, b.1, expected, got
    );
    assert!(
        (got.1 as i32 - expected as i32).abs() <= 1,
        "clicking the first glyph of column 2 must return flat offset {expected}, got {}",
        got.1
    );
}

// ---------------------------------------------------------------------------
// RED 6: fenced code — hit testing must follow the rendered monospace line geometry.
// The real line height is *measured* from two code blocks of different length.
// ---------------------------------------------------------------------------
fn code_block(lines: usize) -> String {
    let mut md = String::from("```rust\n");
    for i in 0..lines {
        md.push_str(&format!("line{:02}abcdefghij\n", i));
    }
    md.push_str("```\n");
    md
}

const CHARS_PER_CODE_LINE: usize = 17; // "lineNNabcdefghij" + '\n'

#[gpui::test]
async fn red_code_block_line_drift(cx: &mut TestAppContext) {
    let (h, cx) = open(cx);
    h.set_doc(cx, &code_block(20));
    let small = bounds_of(&code_block(20).replace("```rust\n", "").replace("```\n", ""));
    h.set_doc(cx, &code_block(30));
    let big = bounds_of(&code_block(30).replace("```rust\n", "").replace("```\n", ""));
    println!("B1 code heights: small={:?} big={:?}", small, big);
    let line_h = (big.3 - small.3) / 10.0;
    assert!((line_h - 16.0 * 1.75).abs() < 1.0, "Code line height must match body: {line_h}");
    let pad = (small.3 - 20.0 * line_h) / 2.0;

    h.set_doc(cx, &code_block(30));
    let b = bounds_of(&code_block(30).replace("```rust\n", "").replace("```\n", ""));
    let target_line = 25usize;
    let click_y = b.1 + pad + (target_line as f32 + 0.5) * line_h;
    let got = h
        .click(cx, Point { x: px(b.0 + 3.0), y: px(click_y) })
        .expect("click must produce a selection");
    let got_line = got.1 / CHARS_PER_CODE_LINE;
    println!(
        "B1-RED code(30 lines): measured_line_height={:.2} pad={:.2} target_line={} expected_char~{} got={:?} got_line~{}",
        line_h, pad, target_line, target_line * CHARS_PER_CODE_LINE, got, got_line
    );
    assert!(
        (got_line as i32 - target_line as i32).abs() <= 1,
        "clicking the 10th rendered code line must select a char from that line \
         (expected char ~{}, got char {} => line {})",
        target_line * CHARS_PER_CODE_LINE,
        got.1,
        got_line
    );
}
