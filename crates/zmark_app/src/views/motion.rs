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

#[derive(Debug, Clone)]
pub struct FolderTransition {
    pub from: f32,
    pub expanded: bool,
    pub started: std::time::Instant,
}

impl FolderTransition {
    pub fn value(&self, now: std::time::Instant) -> f32 {
        let delta = (now.saturating_duration_since(self.started).as_secs_f32() / 0.6).min(1.0);
        let eased = 1.0 - (1.0 - delta).powi(3);
        self.from + ((if self.expanded { 1.0 } else { 0.0 }) - self.from) * eased
    }
    pub fn finished(&self, now: std::time::Instant) -> bool {
        now.saturating_duration_since(self.started) >= Duration::from_millis(600)
    }
}

/// Advances the dirty pulse only on a clean -> dirty transition.
pub fn advance_dirty_generation(was_dirty: bool, is_dirty: bool, generation: u64) -> (u64, bool) {
    let generation = if is_dirty && !was_dirty {
        generation.wrapping_add(1)
    } else {
        generation
    };
    (generation, is_dirty)
}

pub fn motion_delta(reduce_motion: bool, delta: f32) -> f32 {
    if reduce_motion {
        1.0
    } else {
        delta
    }
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
    fn reduce_motion_returns_the_end_state() {
        assert_eq!(motion_delta(true, 0.0), 1.0);
        assert_eq!(motion_delta(true, 0.4), 1.0);
        assert_eq!(motion_delta(false, 0.0), 0.0);
        assert_eq!(motion_delta(false, 0.4), 0.4);
    }

    #[test]
    fn folder_reversal_starts_at_the_current_value_and_finishes_in_600ms() {
        let start = std::time::Instant::now();
        let opening = FolderTransition {
            from: 0.0,
            expanded: true,
            started: start,
        };
        let halfway = start + Duration::from_millis(300);
        assert_eq!(opening.value(halfway), 0.875);
        let closing = FolderTransition {
            from: opening.value(halfway),
            expanded: false,
            started: halfway,
        };
        assert_eq!(closing.value(halfway), 0.875);
        assert!(!closing.finished(halfway + Duration::from_millis(599)));
        assert!(closing.finished(halfway + Duration::from_millis(600)));
        assert_eq!(closing.value(halfway + Duration::from_millis(600)), 0.0);
    }

    #[test]
    fn dirty_pulse_only_advances_on_clean_to_dirty() {
        assert_eq!(advance_dirty_generation(false, true, 4), (5, true));
        assert_eq!(advance_dirty_generation(true, true, 5), (5, true));
        assert_eq!(advance_dirty_generation(true, false, 5), (5, false));
        assert_eq!(advance_dirty_generation(false, true, 5), (6, true));
    }
}
