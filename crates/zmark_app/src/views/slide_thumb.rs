//! A selection thumb that slides between slots of a single-choice control.
//!
//! The sidebar's selection highlight (`sidebar_selection`) animates between measured
//! file rows. Controls with fixed geometry (segmented controls, icon switchers, settings
//! navigation, toggles) do not need measuring: the slot position follows from the parent's
//! size, the slot count and the index. `SlideThumb` fills its `relative` parent, computes
//! the slot rect from that, and eases between rects when the index changes.
//!
//! Place it as the first child so the options paint over it, and leave the active option's
//! own background transparent. The animation state lives in GPUI element state keyed by the
//! element id, so a control that is not mounted (a settings page that is not shown) simply
//! starts at its target. "Reduce motion" (`App::reduce_motion`) makes it jump.

use std::time::{Duration, Instant};

use gpui::{
    ease_out_quint, point, px, relative, size, AnyElement, App, AvailableSpace, Bounds, Element,
    ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels,
    Position, Size, Style, Styled, Window,
};

const DURATION: Duration = Duration::from_millis(200);

/// How the parent is divided into slots, along one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SlotLayout {
    /// `count` equal slots across the parent's width, `gap` apart.
    Equal { count: usize },
    /// Slots of a fixed `width` side by side; each is the parent's height.
    Row { width: f32 },
    /// Slots of a fixed `height` stacked; each is the parent's width.
    Column { height: f32 },
}

pub struct SlideThumb {
    id: ElementId,
    index: usize,
    layout: SlotLayout,
    inset: f32,
    gap: f32,
    child: Option<AnyElement>,
}

impl SlideThumb {
    /// `child` is the thumb's look (background, radius, shadow); it is sized to the slot.
    pub fn new(id: impl Into<ElementId>, index: usize, layout: SlotLayout, child: impl IntoElement) -> Self {
        Self { id: id.into(), index, layout, inset: 0.0, gap: 0.0, child: Some(child.into_any_element()) }
    }

    /// Padding between the parent's edge and the slots.
    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset;
        self
    }

    /// Space between neighbouring slots.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Offset from the parent's top-left and size of slot `index`.
    pub fn slot(layout: SlotLayout, inset: f32, gap: f32, index: usize, parent: Size<Pixels>) -> ((f32, f32), (f32, f32)) {
        let (w, h) = (f32::from(parent.width), f32::from(parent.height));
        let (inner_w, inner_h) = ((w - 2.0 * inset).max(0.0), (h - 2.0 * inset).max(0.0));
        let i = index as f32;
        match layout {
            SlotLayout::Equal { count } => {
                let n = count.max(1) as f32;
                let slot_w = ((inner_w - (n - 1.0) * gap) / n).max(0.0);
                ((inset + i * (slot_w + gap), inset), (slot_w, inner_h))
            }
            SlotLayout::Row { width } => ((inset + i * (width + gap), inset), (width, inner_h)),
            SlotLayout::Column { height } => ((inset, inset + i * (height + gap)), (inner_w, height)),
        }
    }
}

#[derive(Clone, Copy)]
struct ThumbState {
    index: usize,
    from: (f32, f32),
    target: (f32, f32),
    started: Instant,
}

impl ThumbState {
    /// Eased offset at `now`, and whether it is still moving.
    fn sample(&self, now: Instant) -> ((f32, f32), bool) {
        let t = now.saturating_duration_since(self.started).as_secs_f32() / DURATION.as_secs_f32();
        if t >= 1.0 {
            return (self.target, false);
        }
        let k = ease_out_quint()(t.max(0.0));
        (
            (
                self.from.0 + (self.target.0 - self.from.0) * k,
                self.from.1 + (self.target.1 - self.from.1) * k,
            ),
            true,
        )
    }
}

impl IntoElement for SlideThumb {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SlideThumb {
    type RequestLayoutState = ();
    type PrepaintState = Option<AnyElement>;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
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
        style.position = Position::Absolute;
        style.inset.top = px(0.).into();
        style.inset.left = px(0.).into();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let child = self.child.take()?;
        let (target, slot) = Self::slot(self.layout, self.inset, self.gap, self.index, bounds.size);
        let reduce_motion = cx.reduce_motion();
        let now = Instant::now();
        let index = self.index;
        let (offset, running) = window.with_element_state(
            global_id.expect("SlideThumb has an id"),
            |state: Option<ThumbState>, _| {
                // A new thumb, or one whose slot only moved because the parent resized,
                // is already where it belongs.
                let settled = ThumbState { index, from: target, target, started: now - DURATION };
                let mut state = state.unwrap_or(settled);
                if state.index != index {
                    let from = if reduce_motion { target } else { state.sample(now).0 };
                    state = ThumbState { index, from, target, started: if reduce_motion { now - DURATION } else { now } };
                } else if state.target != target {
                    state = settled;
                }
                (state.sample(now), state)
            },
        );
        if running {
            window.request_animation_frame();
        }
        let mut element = gpui::div()
            .w(px(slot.0))
            .h(px(slot.1))
            .child(child)
            .into_any_element();
        element.layout_as_root(
            size(AvailableSpace::Definite(px(slot.0)), AvailableSpace::Definite(px(slot.1))),
            window,
            cx,
        );
        element.prepaint_at(bounds.origin + point(px(offset.0), px(offset.1)), window, cx);
        Some(element)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        element: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(element) = element {
            element.paint(window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SlideThumb, SlotLayout};
    use gpui::{px, size};

    #[test]
    fn equal_slots_split_the_track_after_inset_and_gaps() {
        let parent = size(px(200.), px(40.));
        let ((x0, y0), (w, h)) = SlideThumb::slot(SlotLayout::Equal { count: 2 }, 2.0, 2.0, 0, parent);
        assert_eq!((x0, y0, w, h), (2.0, 2.0, 97.0, 36.0));
        let ((x1, _), _) = SlideThumb::slot(SlotLayout::Equal { count: 2 }, 2.0, 2.0, 1, parent);
        assert_eq!(x1, 101.0);
    }

    #[test]
    fn fixed_row_and_column_slots_advance_by_their_own_size_and_gap() {
        let parent = size(px(100.), px(28.));
        let ((x, y), (w, h)) = SlideThumb::slot(SlotLayout::Row { width: 28.0 }, 0.0, 4.0, 2, parent);
        assert_eq!((x, y, w, h), (64.0, 0.0, 28.0, 28.0));
        let parent = size(px(144.), px(200.));
        let ((x, y), (w, h)) = SlideThumb::slot(SlotLayout::Column { height: 32.0 }, 0.0, 4.0, 3, parent);
        assert_eq!((x, y, w, h), (0.0, 108.0, 144.0, 32.0));
    }
}
