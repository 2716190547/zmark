//! Small, shared motion helpers.
//!
//! GPUI 0.2.2 has no CSS transforms on `div`; motion here is limited to opacity
//! and small size/layout changes. When the user enables "Reduce motion" the
//! animation wrapper is skipped entirely instead of using a zero duration,
//! which would make `elapsed / duration` undefined.

use std::time::Duration;

use gpui::{
    ease_out_quint, Animation, AnimationExt, AnyElement, ElementId, IntoElement, SharedString,
};

/// Advances the dirty pulse only on a clean -> dirty transition.
pub fn advance_dirty_generation(was_dirty: bool, is_dirty: bool, generation: u64) -> (u64, bool) {
    let generation = if is_dirty && !was_dirty {
        generation.wrapping_add(1)
    } else {
        generation
    };
    (generation, is_dirty)
}

/// Applies a one-shot `ease_out_quint` animation, or returns the element as-is
/// when reduced motion is requested.
pub fn animated<E: IntoElement + 'static>(
    element: E,
    reduce_motion: bool,
    id: impl Into<SharedString>,
    duration_ms: u64,
    animator: impl Fn(E, f32) -> E + 'static,
) -> AnyElement {
    if reduce_motion {
        element.into_any_element()
    } else {
        element
            .with_animation(
                ElementId::Name(id.into()),
                Animation::new(Duration::from_millis(duration_ms)).with_easing(ease_out_quint()),
                animator,
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_pulse_only_advances_on_clean_to_dirty() {
        assert_eq!(advance_dirty_generation(false, true, 4), (5, true));
        assert_eq!(advance_dirty_generation(true, true, 5), (5, true));
        assert_eq!(advance_dirty_generation(true, false, 5), (5, false));
        assert_eq!(advance_dirty_generation(false, true, 5), (6, true));
    }
}
