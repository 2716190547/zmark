//! Size-table virtual list for the preview pane.
//!
//! GPUI 0.2.2's `ListState` only knows the height of blocks it has already
//! measured. Driving a scrollbar from those partial heights makes the thumb
//! jump and clamps `scroll_to` back into the measured region. This module keeps
//! one estimated height per AST block, updates it from real prepaint bounds,
//! and derives every scroll position from prefix sums instead of measured-only
//! offsets. It is intentionally local: no new GPUI version or dependency.

use zmark_core::ast::{flatten_block, BlockElement, ParsedDocument};
use gpui::{point, px, size, Bounds, ListOffset, Pixels, Point, ScrollHandle, Size};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

const DEFAULT_ITEM_HEIGHT: f32 = 40.0;

/// Saved preview layout of a background document; see [`PreviewScrollState::snapshot`].
#[derive(Debug, Clone, Default)]
pub struct PreviewSnapshot {
    measured: Vec<Option<f32>>,
    top: (usize, f32),
}

pub type MeasurementSink = Rc<RefCell<Vec<(usize, f32)>>>;

#[derive(Clone)]
pub struct PreviewScrollState {
    scroll: ScrollHandle,
    sizes: Vec<f32>,
    origins: Vec<f32>,
    padding_top: f32,
    padding_bottom: f32,
    total: f32,
    total_shared: Rc<Cell<f32>>,
    layout_key: u128,
    last_parsed_ptr: usize,
    /// Font size and family key the measured heights were taken with.
    last_font_key: u32,
    pending: MeasurementSink,
    last_range: std::ops::Range<usize>,
    /// Real prepaint heights, kept across estimate rebuilds for the same AST.
    measured: Vec<Option<f32>>,
    /// AST the measured heights belong to, used to carry them across reparses.
    measured_ast: Option<Arc<ParsedDocument>>,
    /// Scroll position from a restored [`PreviewSnapshot`], applied by the next layout.
    pending_restore: Option<(usize, f32)>,
    /// (scroll offset when set, requested block, in-block progress). Keeps a
    /// programmatic `scroll_to` meaningful when GPUI clamps a short document.
    logical_override: Rc<Cell<Option<(f32, usize, f32)>>>,
    scrollbar_dragging: Rc<Cell<bool>>,
    scrollbar_drag_ratio: Rc<Cell<Option<f32>>>,
    scrollbar_drag_until: Rc<Cell<Option<Instant>>>,
    scrollbar_scrolled: Rc<Cell<bool>>,
    shared_content_height: Rc<Cell<Option<f32>>>,
}

impl Default for PreviewScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewScrollState {
    pub fn new() -> Self {
        Self {
            scroll: ScrollHandle::new(),
            sizes: Vec::new(),
            origins: Vec::new(),
            padding_top: 24.0,
            padding_bottom: 24.0,
            total: 48.0,
            total_shared: Rc::new(Cell::new(48.0)),
            layout_key: 0,
            last_parsed_ptr: 0,
            last_font_key: 0,
            pending: Rc::new(RefCell::new(Vec::new())),
            last_range: 0..0,
            measured: Vec::new(),
            measured_ast: None,
            pending_restore: None,
            logical_override: Rc::new(Cell::new(None)),
            scrollbar_dragging: Rc::new(Cell::new(false)),
            scrollbar_drag_ratio: Rc::new(Cell::new(None)),
            scrollbar_drag_until: Rc::new(Cell::new(None)),
            scrollbar_scrolled: Rc::new(Cell::new(false)),
            shared_content_height: Rc::new(Cell::new(None)),
        }
    }

    pub fn scroll_handle(&self) -> &ScrollHandle {
        &self.scroll
    }

    pub fn measurement_sink(&self) -> MeasurementSink {
        self.pending.clone()
    }

    pub fn item_count(&self) -> usize {
        self.sizes.len()
    }

    pub fn sizes(&self) -> &[f32] {
        &self.sizes
    }

    pub fn origins(&self) -> &[f32] {
        &self.origins
    }

    pub fn total_height(&self) -> f32 {
        self.total
    }

    pub fn offset(&self) -> f32 {
        (-f32::from(self.scroll.offset().y)).max(0.0)
    }

    pub fn max_scroll(&self) -> f32 {
        let viewport_h = f32::from(self.scroll.bounds().size.height).max(1.0);
        (self.total - viewport_h).max(0.0)
    }

    pub fn set_scroll_offset(&mut self, desired: f32) {
        let clamped = desired.clamp(0.0, self.max_scroll());
        self.scroll.set_offset(point(px(0.), px(-clamped)));
        self.logical_override.set(None);
    }

    pub fn is_scrollbar_dragging(&self) -> bool {
        self.scrollbar_dragging.get()
    }

    pub fn take_scrollbar_scrolled(&self) -> bool {
        self.scrollbar_scrolled.replace(false)
    }

    pub fn active_drag_ratio(&self) -> Option<f32> {
        if self.scrollbar_dragging.get()
            || self
                .scrollbar_drag_until
                .get()
                .is_some_and(|until| Instant::now() < until)
        {
            self.scrollbar_drag_ratio.get()
        } else {
            self.scrollbar_drag_ratio.set(None);
            self.scrollbar_drag_until.set(None);
            None
        }
    }

    pub fn cancel_scrollbar_drag(&self) {
        self.scrollbar_dragging.set(false);
        self.scrollbar_drag_ratio.set(None);
        self.scrollbar_drag_until.set(None);
    }

    pub fn shared_content_height(&self) -> Option<f32> {
        self.shared_content_height.get()
    }

    pub fn set_shared_content_height(&self, height: Option<f32>) {
        self.shared_content_height.set(height);
    }

    fn virtual_scroll_bounds(&self) -> (f32, f32, f32) {
        let viewport_h = f32::from(self.scroll.bounds().size.height).max(1.0);
        let real_max = (self.total_shared.get() - viewport_h).max(0.0);
        let virtual_total = match self.shared_content_height.get() {
            Some(shared) if real_max > 0.5 => shared.max(self.total_shared.get()),
            _ => self.total_shared.get(),
        };
        let virtual_max = (virtual_total - viewport_h).max(0.0);
        (real_max, virtual_total, virtual_max)
    }

    pub fn scroll_ratio(&self) -> f32 {
        let max = self.max_scroll();
        if max <= 0.5 {
            0.0
        } else {
            (self.offset() / max).clamp(0.0, 1.0)
        }
    }

    /// Returns the visible block range and whether it changed since last frame.
    /// The caller notifies on change so prepaint bounds can settle before input.
    pub fn visible_range(&mut self, overdraw: f32) -> (std::ops::Range<usize>, bool) {
        let range = if self.sizes.is_empty() {
            0..0
        } else {
            let offset = self.offset();
            let viewport_h = f32::from(self.scroll.bounds().size.height).max(1.0);
            // Only overdraw below the viewport. Rendering blocks above the top
            // registers off-screen text bounds and breaks real hit-test controls.
            let start_y = offset.max(0.0);
            let end_y = offset + viewport_h + overdraw;
            let mut start = self
                .origins
                .partition_point(|&o| o <= start_y)
                .saturating_sub(1);
            if start < self.sizes.len() && self.origins[start] + self.sizes[start] <= start_y {
                start += 1;
            }
            let end = self
                .origins
                .partition_point(|&o| o < end_y)
                .min(self.sizes.len());
            start.min(end)..end
        };
        let changed = range != self.last_range;
        self.last_range = range.clone();
        (range, changed)
    }

    fn progress_from_offset(&self, offset: f32) -> (usize, f32) {
        if self.sizes.is_empty() {
            return (0, 0.0);
        }
        let ix = self
            .origins
            .partition_point(|&o| o <= offset)
            .saturating_sub(1)
            .min(self.sizes.len() - 1);
        let h = self.sizes[ix].max(1.0);
        (ix, ((offset - self.origins[ix]) / h).clamp(0.0, 1.0))
    }

    pub fn block_progress_at_offset(&mut self, offset: f32) -> (usize, f32) {
        if let Some((stored, ix, progress)) = self.logical_override.get() {
            if (stored - offset).abs() <= 0.5 {
                return (ix, progress);
            }
            self.logical_override.set(None);
        }
        self.progress_from_offset(offset)
    }

    pub fn offset_for_block(&self, ix: usize, progress: f32) -> f32 {
        if self.sizes.is_empty() {
            return 0.0;
        }
        let ix = ix.min(self.sizes.len() - 1);
        self.origins[ix] + progress.clamp(0.0, 1.0) * self.sizes[ix]
    }

    /// Rebuild the size table when the AST, column width, font or rem size changes.
    /// Keeps the block currently under the viewport top anchored to the same pixel.
    pub fn ensure_layout(
        &mut self,
        parsed: &Arc<ParsedDocument>,
        max_width: f32,
        font_size: f32,
        line_height: f32,
        rem: f32,
    ) -> bool {
        let key = layout_key(parsed, max_width, font_size, line_height, rem);
        if key == self.layout_key && self.sizes.len() == parsed.blocks.len() {
            return false;
        }
        let old_offset = self.offset();
        let old_max = self.max_scroll();
        let was_at_end = old_max > 0.5 && old_offset >= old_max - 1.0;
        // Anchor against the old origins, before replacing the size table.
        let anchor_delta = self.top_anchor(old_offset);
        let ptr = Arc::as_ptr(parsed) as usize;
        let font_key = crate::platform::font_layout_key(font_size);
        let ast_changed = ptr != self.last_parsed_ptr
            || self.measured.len() != parsed.blocks.len()
            || font_key != self.last_font_key;
        if ast_changed {
            // Throwing every height away makes all blocks fall back to estimates
            // for a frame, which shows as flicker and blank gaps after a small
            // edit or a line-height change. Unchanged blocks keep their last real
            // height until they are measured again; after a font or line-height
            // change those heights are stale but far closer than an estimate.
            // After `restore` there is no AST to compare; the caller vouched
            // that the source is unchanged, so heights apply one-to-one.
            self.measured = if self.measured_ast.is_none() && self.measured.len() == parsed.blocks.len() {
                std::mem::take(&mut self.measured)
            } else {
                carry_measured(self.measured_ast.as_deref(), &self.measured, parsed)
            };
            self.last_font_key = font_key;
            self.pending.borrow_mut().clear();
            self.last_parsed_ptr = ptr;
            self.measured_ast = Some(parsed.clone());
        }
        self.padding_top = rem * 1.5;
        self.padding_bottom = rem * 1.5;
        self.sizes = estimate_sizes(parsed, max_width, font_size, line_height, rem)
            .into_iter()
            .enumerate()
            .map(|(ix, estimate)| self.measured.get(ix).copied().flatten().unwrap_or(estimate))
            .collect();
        self.recompute();
        self.layout_key = key;
        let new_offset = if let Some((ix, progress)) = self.pending_restore.take() {
            self.offset_for_block(ix, progress)
        } else if was_at_end {
            self.max_scroll()
        } else {
            match anchor_delta {
                None => old_offset,
                Some((ix, delta)) => self.origins.get(ix).copied().unwrap_or(0.0) + delta,
            }
        };
        self.set_scroll_offset(new_offset);
        true
    }

    /// Apply real prepaint heights collected during the previous frame.
    /// Returns true when the table changed and another frame is needed.
    ///
    /// Measurement only refines the size table. It must never move the text the
    /// user is reading:
    /// - a scrollbar drag is pointer-owned, so its ratio wins;
    /// - a viewport pinned to the document end stays pinned to the new end;
    /// - otherwise the block under the viewport top keeps its pixel offset.
    ///
    /// Ratio-preserving (`old_offset / old_max * new_max`) was removed: it makes
    /// every measured height change slide the whole viewport, which is the
    /// repeated jump seen when the preview is at the end of a document whose
    /// trailing blocks have not been measured yet.
    pub fn apply_measured(&mut self, drag_ratio: Option<f32>) -> bool {
        let measured = std::mem::take(&mut *self.pending.borrow_mut());
        if measured.is_empty() {
            return false;
        }
        let old_offset = self.offset();
        let old_max = self.max_scroll();
        let anchor_delta = self.top_anchor(old_offset);
        let mut changed = false;
        for (ix, height) in measured {
            if let Some(slot) = self.sizes.get_mut(ix) {
                // Real content height replaces the estimate; min_h only keeps
                // the wrapper from collapsing before the next measurement.
                if (height - *slot).abs() > 1.0 {
                    *slot = height;
                    if let Some(known) = self.measured.get_mut(ix) {
                        *known = Some(height);
                    }
                    changed = true;
                }
            }
        }
        if !changed {
            return false;
        }
        self.recompute();
        let new_offset = match drag_ratio {
            Some(ratio) => ratio.clamp(0.0, 1.0) * self.max_scroll(),
            // `old_max > 0.5` keeps a short document that only just became
            // scrollable from jumping to its new bottom while at the top.
            None if old_max > 0.5 && old_offset >= old_max - 1.0 => self.max_scroll(),
            None => match anchor_delta {
                None => old_offset,
                Some((ix, delta)) => self.origins.get(ix).copied().unwrap_or(0.0) + delta,
            },
        };
        self.set_scroll_offset(new_offset);
        true
    }

    /// Block under the viewport top plus the pixel distance into it.
    fn top_anchor(&self, offset: f32) -> Option<(usize, f32)> {
        if self.sizes.is_empty() || offset < self.padding_top {
            return None;
        }
        let ix = self
            .origins
            .partition_point(|&o| o <= offset)
            .saturating_sub(1);
        Some((ix, offset - self.origins[ix]))
    }

    fn recompute(&mut self) {
        self.origins.clear();
        self.origins.reserve(self.sizes.len());
        let mut y = self.padding_top;
        for &h in &self.sizes {
            self.origins.push(y);
            y += h;
        }
        self.total = y + self.padding_bottom;
        self.total_shared.set(self.total);
    }

    // ---- `ListState`-shaped compatibility methods used by behaviour tests ----

    /// Measured heights and scroll position of the current document, to restore
    /// when it becomes active again. Holds no AST, so a background document can
    /// still release its parse.
    pub fn snapshot(&self) -> PreviewSnapshot {
        PreviewSnapshot {
            measured: self.measured.clone(),
            top: self.progress_from_offset(self.offset()),
        }
    }

    /// Like [`Self::reset`], then reuses `snapshot` when it matches `count` blocks.
    /// The caller must only pass a snapshot taken for the same, unchanged source.
    pub fn restore(&mut self, count: usize, snapshot: PreviewSnapshot) {
        self.reset(count);
        if snapshot.measured.len() == count {
            self.measured = snapshot.measured;
            self.pending_restore = Some(snapshot.top);
        }
    }

    pub fn reset(&mut self, count: usize) {
        self.sizes = vec![DEFAULT_ITEM_HEIGHT; count];
        self.measured = vec![None; count];
        self.measured_ast = None;
        self.pending_restore = None;
        self.last_parsed_ptr = 0;
        self.pending.borrow_mut().clear();
        self.recompute();
        self.layout_key = 0;
        self.logical_override.set(None);
        self.cancel_scrollbar_drag();
        self.scrollbar_scrolled.set(false);
        self.shared_content_height.set(None);
        self.set_scroll_offset(0.0);
    }

    /// Force the next render to rebuild estimates from the current AST while
    /// keeping the current scroll offset.
    pub fn invalidate_layout(&mut self) {
        self.layout_key = 0;
    }

    pub fn splice(&mut self, _old: std::ops::Range<usize>, count: usize) {
        self.reset(count);
    }

    pub fn logical_scroll_top(&self) -> ListOffset {
        let offset = self.offset();
        let (ix, progress) = match self.logical_override.get() {
            Some((stored, ix, progress)) if (stored - offset).abs() <= 0.5 => (ix, progress),
            _ => self.progress_from_offset(offset),
        };
        ListOffset {
            item_ix: ix,
            offset_in_item: px(progress * self.sizes.get(ix).copied().unwrap_or(0.0)),
        }
    }

    pub fn scroll_to(&mut self, top: ListOffset) {
        let ix = top.item_ix.min(self.sizes.len().saturating_sub(1));
        let h = self.sizes.get(ix).copied().unwrap_or(0.0).max(1.0);
        let progress = (f32::from(top.offset_in_item) / h).clamp(0.0, 1.0);
        self.set_scroll_offset(self.offset_for_block(ix, progress));
        self.logical_override.set(Some((self.offset(), ix, progress)));
    }

    pub fn bounds_for_item(&self, ix: usize) -> Option<Bounds<Pixels>> {
        let h = *self.sizes.get(ix)?;
        let viewport = self.scroll.bounds();
        let y = viewport.origin.y + px(self.origins[ix]) + self.scroll.offset().y;
        Some(Bounds::new(
            point(viewport.origin.x, y),
            size(viewport.size.width, px(h)),
        ))
    }

    pub fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.scroll.bounds()
    }

    pub fn max_offset_for_scrollbar(&self) -> Size<Pixels> {
        Size {
            width: px(0.),
            height: px(self.max_scroll()),
        }
    }

    pub fn scroll_px_offset_for_scrollbar(&self) -> Point<Pixels> {
        point(px(0.), px(-self.offset()))
    }

    pub fn set_offset_from_scrollbar(&mut self, position: Point<Pixels>) {
        self.set_scroll_offset((-f32::from(position.y)).max(0.0));
    }

    pub fn scrollbar_drag_started(&self) {
        self.scrollbar_dragging.set(true);
        self.scrollbar_drag_until.set(None);
        let viewport_h = f32::from(self.scroll.bounds().size.height).max(1.0);
        let max_scroll = (self.total_shared.get() - viewport_h).max(0.0);
        if max_scroll > 0.5 {
            self.scrollbar_drag_ratio
                .set(Some((self.offset() / max_scroll).clamp(0.0, 1.0)));
        }
    }

    pub fn scrollbar_drag_ended(&self) {
        self.scrollbar_dragging.set(false);
        if self.scrollbar_drag_ratio.get().is_some() {
            self.scrollbar_drag_until
                .set(Some(Instant::now() + Duration::from_millis(250)));
        }
    }
}

impl gpui_component::scroll::ScrollbarHandle for PreviewScrollState {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.scroll.bounds()
    }

    fn offset(&self) -> Point<Pixels> {
        let (real_max, _, virtual_max) = self.virtual_scroll_bounds();
        if real_max <= 0.5 || virtual_max <= 0.5 {
            return self.scroll.offset();
        }
        let real_y = (-f32::from(self.scroll.offset().y)).clamp(0.0, real_max);
        let ratio = (real_y / real_max).clamp(0.0, 1.0);
        point(px(0.), px(-(ratio * virtual_max)))
    }

    fn set_offset(&self, offset: Point<Pixels>) {
        let (real_max, _, virtual_max) = self.virtual_scroll_bounds();
        let ratio = if virtual_max > 0.5 {
            (-f32::from(offset.y) / virtual_max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let real_y = (ratio * real_max).clamp(0.0, real_max);
        self.scroll.set_offset(point(px(0.), px(-real_y)));
        self.logical_override.set(None);
        if self.scrollbar_dragging.get() && real_max > 0.5 {
            self.scrollbar_drag_ratio.set(Some(ratio));
        }
        self.scrollbar_scrolled.set(true);
    }

    fn content_size(&self) -> Size<Pixels> {
        let bounds = self.scroll.bounds();
        let (_, virtual_total, _) = self.virtual_scroll_bounds();
        let total = virtual_total.max(f32::from(bounds.size.height));
        size(bounds.size.width, px(total))
    }

    fn start_drag(&self) {
        self.scrollbar_drag_started();
    }

    fn end_drag(&self) {
        self.scrollbar_drag_ended();
    }
}

/// Maps measured heights from `old` onto `new` for blocks that did not change:
/// the common prefix and the common suffix. Edited blocks start unmeasured.
fn carry_measured(old: Option<&ParsedDocument>, measured: &[Option<f32>], new: &ParsedDocument) -> Vec<Option<f32>> {
    let mut out = vec![None; new.blocks.len()];
    let Some(old) = old else { return out; };
    if old.blocks.len() != measured.len() { return out; }
    let (a, b) = (&old.blocks, &new.blocks);
    let prefix = a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count();
    let max_suffix = a.len().min(b.len()) - prefix;
    let suffix = a.iter().rev().zip(b.iter().rev()).take(max_suffix).take_while(|(x, y)| x == y).count();
    out[..prefix].copy_from_slice(&measured[..prefix]);
    for k in 0..suffix {
        out[b.len() - 1 - k] = measured[a.len() - 1 - k];
    }
    out
}

fn layout_key(
    parsed: &Arc<ParsedDocument>,
    max_width: f32,
    font_size: f32,
    line_height: f32,
    rem: f32,
) -> u128 {
    let ptr = Arc::as_ptr(parsed) as usize as u128;
    let width = max_width.round() as u32 as u128;
    let font = crate::platform::font_layout_key(font_size) as u128;
    let lh = line_height.to_bits() as u128;
    let rem_bits = rem.to_bits() as u128;
    ptr << 64 | width << 40 | font << 20 | lh << 10 | rem_bits
}

fn wrapped_lines(chars: usize, chars_per_line: f32) -> f32 {
    ((chars as f32) / chars_per_line.max(8.0)).ceil().max(1.0)
}

fn estimate_sizes(
    parsed: &ParsedDocument,
    max_width: f32,
    font_size: f32,
    line_height: f32,
    rem: f32,
) -> Vec<f32> {
    let lh = font_size * line_height.clamp(1.0, 3.0);
    // Deliberately conservative: CJK wraps earlier than this estimate, and real
    // prepaint bounds grow the entry instead of clipping content.
    let chars_per_line = (max_width.max(240.0) / (font_size * 0.62)).max(16.0);
    let item_pad = rem; // pb_4 on each virtualized item
    parsed
        .blocks
        .iter()
        .map(|block| {
            // Only paragraphs use the flattened text height. Flattening every
            // heading/list/quote/table on each layout rebuild copied text that
            // the estimate below never reads.
            let text_h = match block {
                BlockElement::Paragraph(_) => {
                    let flat = flatten_block(block, "0");
                    wrapped_lines(flat.text.chars().count(), chars_per_line) * lh
                }
                _ => 0.0,
            };
            let h = match block {
                BlockElement::Heading { level, .. } => {
                    let mult = match level {
                        1 => 1.8,
                        2 => 1.45,
                        3 => 1.25,
                        _ => 1.1,
                    };
                    // Deliberately below real prepaint height; measured bounds grow it.
                    font_size * mult * 1.75 + rem * 0.75
                }
                BlockElement::Paragraph(_) => text_h,
                BlockElement::FencedCode { lang, code } => {
                    if lang.as_deref() == Some("mermaid") {
                        360.0
                    } else {
                        code.lines().count().max(1) as f32 * lh + rem * 1.5
                    }
                }
                BlockElement::BlockQuote(children) => {
                    children
                        .iter()
                        .map(|c| estimate_block(c, &lh, &chars_per_line, rem, font_size))
                        .sum::<f32>()
                        + rem * 1.5
                        + (children.len().saturating_sub(1)) as f32 * rem * 0.5
                }
                BlockElement::List { items, .. } => items
                    .iter()
                    .map(|item| {
                        item.body
                            .iter()
                            .map(|c| estimate_block(c, &lh, &chars_per_line, rem, font_size))
                            .sum::<f32>()
                            + lh
                            + rem * 0.5
                    })
                    .sum::<f32>(),
                BlockElement::Table { rows, .. } => (rows.len() + 1) as f32 * (lh + rem * 0.75),
                BlockElement::TableOfContents => rem * 7.5,
                BlockElement::Spacer { lines } => (*lines as f32) * 14.0,
                BlockElement::Rule => rem * 2.0 + 1.0,
            };
            (h + item_pad).max(DEFAULT_ITEM_HEIGHT)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_reuses_heights_and_scroll_position() {
        let source = (0..30).map(|i| format!("Paragraph {i}\n\n")).collect::<String>();
        let parsed = Arc::new(zmark_core::ast::parse_markdown(&source));
        let mut state = PreviewScrollState::new();
        state.ensure_layout(&parsed, 900.0, 14.0, 1.5, 16.0);
        for ix in 0..30 { state.measurement_sink().borrow_mut().push((ix, 90.0)); }
        state.apply_measured(None);
        state.set_scroll_offset(state.offset_for_block(12, 0.5));
        let snapshot = state.snapshot();
        let reparsed = Arc::new(zmark_core::ast::parse_markdown(&source));
        let mut other = PreviewScrollState::new();
        other.restore(30, snapshot);
        other.ensure_layout(&reparsed, 900.0, 14.0, 1.5, 16.0);
        assert!(other.sizes().iter().all(|&h| (h - 90.0).abs() < 0.01));
        assert!((other.offset() - other.offset_for_block(12, 0.5)).abs() < 0.5);
    }

    #[test]
    fn carry_measured_keeps_unchanged_prefix_and_suffix() {
        let old = zmark_core::ast::parse_markdown("a\n\n- [ ] b\n\nc\n\nd\n");
        let new = zmark_core::ast::parse_markdown("a\n\n- [x] b\n\nc\n\nd\n");
        let measured = vec![Some(10.0), Some(20.0), Some(30.0), Some(40.0)];
        assert_eq!(carry_measured(Some(&old), &measured, &new), vec![Some(10.0), None, Some(30.0), Some(40.0)]);
        let inserted = zmark_core::ast::parse_markdown("a\n\nnew\n\n- [ ] b\n\nc\n\nd\n");
        assert_eq!(carry_measured(Some(&old), &measured, &inserted), vec![Some(10.0), None, Some(20.0), Some(30.0), Some(40.0)]);
        assert_eq!(carry_measured(Some(&old), &measured, &old), measured);
    }

    #[test]
    fn measurement_keeps_the_block_under_the_top_anchored() {
        let mut state = PreviewScrollState::new();
        state.reset(10);
        // reset(): ten 40px items, 24px padding. Offset 150 sits 6px into block 3.
        state.set_scroll_offset(150.0);
        let before = state.top_anchor(state.offset()).unwrap();
        let before_range = state.visible_range(32.0).0;
        assert_eq!(before.0, 3);
        // Block 0 grows above the viewport. Ratio preservation would keep the
        // ratio 150/old_max and move the top block; content anchoring must not.
        state.measurement_sink().borrow_mut().push((0, 120.0));
        assert!(state.apply_measured(None));
        let after = state.top_anchor(state.offset()).unwrap();
        assert_eq!(after.0, 3, "top block changed: {before:?} -> {after:?}");
        assert!(
            (after.1 - before.1).abs() < 0.01,
            "top block moved inside the viewport: {before:?} -> {after:?}"
        );
        assert_eq!(state.visible_range(32.0).0, before_range);
    }

    #[test]
    fn bottom_pinned_viewport_stays_at_the_new_end() {
        let mut state = PreviewScrollState::new();
        state.reset(6);
        state.set_scroll_offset(state.max_scroll());
        for height in [10.0, 120.0] {
            // Both shrinking and growing a block above the viewport must keep
            // a bottom-pinned viewport at the recalculated end.
            state.measurement_sink().borrow_mut().push((0, height));
            assert!(state.apply_measured(None));
            assert!(
                (state.offset() - state.max_scroll()).abs() < 0.01,
                "bottom-pinned viewport left the end: {} vs {}",
                state.offset(),
                state.max_scroll()
            );
        }
    }

    #[test]
    fn layout_rebuild_keeps_a_bottom_pinned_viewport_at_the_new_end() {
        let source = (0..12)
            .map(|index| format!("## Heading {index}\n\nParagraph with enough words to occupy several lines.\n\n"))
            .collect::<String>();
        let parsed = Arc::new(zmark_core::ast::parse_markdown(&source));
        let mut state = PreviewScrollState::new();
        assert!(state.ensure_layout(&parsed, 900.0, 14.0, 1.5, 16.0));
        state.set_scroll_offset(state.max_scroll());
        assert!(state.ensure_layout(&parsed, 700.0, 14.0, 1.5, 16.0));
        assert!(
            (state.offset() - state.max_scroll()).abs() < 0.01,
            "layout rebuild moved the bottom-pinned viewport: {} vs {}",
            state.offset(),
            state.max_scroll()
        );
    }

    #[test]
    fn drag_ratio_still_owns_the_offset_during_measurement() {
        let mut state = PreviewScrollState::new();
        state.reset(10);
        state.set_scroll_offset(150.0);
        state.measurement_sink().borrow_mut().push((0, 120.0));
        assert!(state.apply_measured(Some(0.5)));
        assert!((state.scroll_ratio() - 0.5).abs() < 0.02);
    }
}

fn estimate_block(
    block: &BlockElement,
    lh: &f32,
    chars_per_line: &f32,
    rem: f32,
    font_size: f32,
) -> f32 {
    let text_h = match block {
        BlockElement::Paragraph(_) => {
            let flat = flatten_block(block, "0");
            wrapped_lines(flat.text.chars().count(), *chars_per_line) * *lh
        }
        _ => 0.0,
    };
    match block {
        BlockElement::Heading { level, .. } => {
            let mult = match level {
                1 => 1.8,
                2 => 1.45,
                3 => 1.25,
                _ => 1.1,
            };
            font_size * mult * 1.75 + rem * 0.75
        }
        BlockElement::FencedCode { lang, code } => {
            if lang.as_deref() == Some("mermaid") {
                360.0
            } else {
                code.lines().count().max(1) as f32 * *lh + rem * 1.5
            }
        }
        BlockElement::BlockQuote(children) => {
            children
                .iter()
                .map(|c| estimate_block(c, lh, chars_per_line, rem, font_size))
                .sum::<f32>()
                + rem
        }
        BlockElement::List { items, .. } => items
            .iter()
            .map(|item| {
                item.body
                    .iter()
                    .map(|c| estimate_block(c, lh, chars_per_line, rem, font_size))
                    .sum::<f32>()
                    + *lh
            })
            .sum::<f32>(),
        BlockElement::Table { rows, .. } => (rows.len() + 1) as f32 * (*lh + rem * 0.75),
        BlockElement::Paragraph(_) => text_h,
        BlockElement::TableOfContents => rem * 7.5,
        BlockElement::Spacer { lines } => (*lines as f32) * 14.0,
        BlockElement::Rule => rem * 2.0,
    }
}
