//! Places preview blocks using heights measured in the same frame.
//!
//! The size table in [`super::virtual_list`] only learns a block's real height
//! after it has been painted. Positioning blocks from that table showed one frame
//! built from estimates after every reparse, line-height change or document
//! switch: blank gaps and jumps. This element lays each visible block out as a
//! root first (like GPUI's `list`), stacks blocks by their real heights from the
//! table origin of the first one, and only then prepaints them. The measured
//! heights are reported to the size table, which re-anchors on the block under
//! the viewport top, so the next frame draws the same positions.

use gpui::{
    point, px, relative, size, AnyElement, App, AvailableSpace, Bounds, ContentMask, Element,
    ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Style, Window,
};

use super::virtual_list::MeasurementSink;

/// Height difference below which a measurement is not reported.
const REPORT_EPSILON: f32 = 1.0;
/// Extra space rendered below the visible area when real heights are shorter than estimated.
const OVERDRAW: f32 = 32.0;
/// Upper bound on blocks rendered past the table's visible range in one frame.
const MAX_EXTRA_BLOCKS: usize = 64;

pub type BuildBlock = Box<dyn Fn(usize) -> Option<AnyElement>>;

/// A group of blocks whose combined height eases from `from` to its measured
/// height (block editing opening or closing). The group is clipped to the eased
/// height and the blocks below follow it, so nothing else on screen moves.
#[derive(Clone, Debug, PartialEq)]
pub struct HeightMotion {
    pub range: std::ops::Range<usize>,
    pub from: f32,
    /// Eased progress in `0.0..1.0`.
    pub progress: f32,
}

pub struct MeasuredBlocks {
    start: usize,
    min_end: usize,
    count: usize,
    start_origin: f32,
    total_height: f32,
    /// Table heights for `start..`, used to report only real changes.
    table_sizes: Vec<f32>,
    build: BuildBlock,
    sink: MeasurementSink,
    motions: Vec<HeightMotion>,
}

impl MeasuredBlocks {
    /// `range` is the table's visible range; `origins`/`sizes` are the size table.
    pub fn new(
        range: std::ops::Range<usize>,
        origins: &[f32],
        sizes: &[f32],
        total_height: f32,
        sink: MeasurementSink,
        build: BuildBlock,
        motions: Vec<HeightMotion>,
    ) -> Self {
        let count = sizes.len();
        let start = range.start.min(count);
        let limit = (range.end + MAX_EXTRA_BLOCKS).min(count);
        Self {
            start,
            min_end: range.end.min(count),
            count: limit,
            start_origin: origins.get(start).copied().unwrap_or(0.0),
            total_height,
            table_sizes: sizes[start..limit].to_vec(),
            build,
            sink,
            motions,
        }
    }
}

impl IntoElement for MeasuredBlocks {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for MeasuredBlocks {
    type RequestLayoutState = ();
    /// Elements with the clip they were prepainted under.
    type PrepaintState = Vec<(AnyElement, Option<ContentMask<Pixels>>)>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = px(self.total_height).into();
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let available = size(AvailableSpace::Definite(bounds.size.width), AvailableSpace::MinContent);
        let visible_bottom = window.content_mask().bounds.bottom() + px(OVERDRAW);
        let mut y = bounds.origin.y + px(self.start_origin);
        let mut items = Vec::new();
        let mut reported = false;
        let mut ix = self.start;
        while ix < self.count && (ix < self.min_end || y < visible_bottom) {
            if let Some(motion) = self.motions.iter().find(|m| m.range.start == ix && m.range.end <= self.count).cloned() {
                // Lay the whole group out at its natural height, then show the eased part.
                let mut group = Vec::new();
                for i in motion.range.clone() {
                    if let Some(mut element) = (self.build)(i) {
                        let height = f32::from(element.layout_as_root(available, window, cx).height);
                        group.push((i, element, height));
                    }
                }
                let natural: f32 = group.iter().map(|(_, _, h)| h).sum();
                let shown = motion.from + (natural - motion.from) * motion.progress.clamp(0.0, 1.0);
                let mask = ContentMask {
                    bounds: Bounds::new(point(bounds.origin.x, y), size(bounds.size.width, px(shown.max(0.0)))),
                };
                let scale = if natural > 0.0 { shown / natural } else { 0.0 };
                let mut inner = y;
                for (i, mut element, height) in group {
                    window.with_content_mask(Some(mask.clone()), |window| {
                        element.prepaint_at(point(bounds.origin.x, inner), window, cx);
                    });
                    inner += px(height);
                    let reported = height * scale;
                    if (reported - self.table_sizes.get(i - self.start).copied().unwrap_or(0.0)).abs() > REPORT_EPSILON {
                        self.sink.borrow_mut().push((i, reported));
                    }
                    items.push((element, Some(mask.clone())));
                }
                // The next frame advances the motion.
                reported = true;
                y += px(shown);
                ix = motion.range.end;
                continue;
            }
            if let Some(mut element) = (self.build)(ix) {
                let height = element.layout_as_root(available, window, cx).height;
                element.prepaint_at(point(bounds.origin.x, y), window, cx);
                let height = f32::from(height);
                if (height - self.table_sizes[ix - self.start]).abs() > REPORT_EPSILON {
                    self.sink.borrow_mut().push((ix, height));
                    reported = true;
                }
                y += px(height);
                items.push((element, None));
            }
            ix += 1;
        }
        if reported {
            // The size table catches up next frame, at the positions drawn now.
            window.request_animation_frame();
        }
        items
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        items: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for (item, mask) in items {
            match mask {
                Some(mask) => window.with_content_mask(Some(mask.clone()), |window| item.paint(window, cx)),
                None => item.paint(window, cx),
            }
        }
    }
}
