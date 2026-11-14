use crate::state::DocKey;
use gpui::{prelude::*, *};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Default)]
pub struct SidebarSelection {
    pub selected: HashSet<DocKey>,
    pub initialized: bool,
    pub focus: Option<DocKey>,
    anchor: Option<DocKey>,
    pub rows: HashMap<DocKey, Bounds<Pixels>>,
    painted_key: Option<DocKey>,
    current: Option<Bounds<Pixels>>,
    from: Option<Bounds<Pixels>>,
    started: Option<Instant>,
}
pub type SharedSelection = Arc<Mutex<SidebarSelection>>;
impl SidebarSelection {
    pub fn remap(&mut self, old: &std::path::Path, new: &std::path::Path) {
        let map = |key: DocKey| match key {
            DocKey::Path(path) => {
                DocKey::Path(zmark_core::file_ops::remap_path(&path, old, new).unwrap_or(path))
            }
            other => other,
        };
        self.selected = self.selected.drain().map(map).collect();
        self.focus = self.focus.take().map(map);
        self.anchor = self.anchor.take().map(map);
    }
    pub fn clear(&mut self) {
        self.initialized = true;
        self.selected.clear();
        self.focus = None;
        self.anchor = None;
    }
    pub fn click(&mut self, key: DocKey, shift: bool, additive: bool) {
        self.initialized = true;
        let mut visible: Vec<_> = self
            .rows
            .iter()
            .map(|(k, b)| (k.clone(), b.origin.y))
            .collect();
        visible.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        if shift {
            if let (Some(start), Some(end)) = (
                visible
                    .iter()
                    .position(|(k, _)| Some(k) == self.anchor.as_ref()),
                visible.iter().position(|(k, _)| k == &key),
            ) {
                if !additive {
                    self.selected.clear();
                }
                for (k, _) in &visible[start.min(end)..=start.max(end)] {
                    self.selected.insert(k.clone());
                }
            } else {
                self.selected.insert(key.clone());
                self.anchor = Some(key.clone());
            }
        } else if additive {
            if !self.selected.remove(&key) {
                self.selected.insert(key.clone());
            }
            self.anchor = Some(key.clone());
        } else {
            self.selected.clear();
            self.selected.insert(key.clone());
            self.anchor = Some(key.clone());
        }
        self.focus = if self.selected.contains(&key) {
            Some(key)
        } else {
            self.selected.iter().next().cloned()
        };
    }
}

pub fn row_anchor(state: SharedSelection, key: DocKey) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            state.lock().unwrap().rows.insert(key.clone(), bounds);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

pub fn selection_layer(
    state: SharedSelection,
    color: Hsla,
    reduce_motion: bool,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |_, _, window, _| {
            let mut state = state.lock().unwrap();
            let old_len = state.selected.len();
            let visible: HashSet<_> = state.rows.keys().cloned().collect();
            state.selected.retain(|key| visible.contains(key));
            if old_len != state.selected.len() {
                window.request_animation_frame();
            }
            if state.selected.len() > 1 {
                for key in &state.selected {
                    if let Some(bounds) = state.rows.get(key) {
                        window.paint_quad(fill(*bounds, color).corner_radii(px(6.)));
                    }
                }
                state.current = state
                    .focus
                    .as_ref()
                    .and_then(|key| state.rows.get(key))
                    .copied();
                state.painted_key = state.focus.clone();
                state.started = None;
                state.from = None;
                return;
            }
            if !state
                .focus
                .as_ref()
                .is_some_and(|key| state.selected.contains(key))
            {
                state.focus = state.selected.iter().next().cloned();
            }
            let target = state
                .focus
                .as_ref()
                .and_then(|key| state.rows.get(key))
                .copied();
            let Some(target) = target else {
                state.current = None;
                state.painted_key = None;
                return;
            };
            if state.painted_key != state.focus {
                state.from = state.current;
                state.started = Some(Instant::now());
                state.painted_key = state.focus.clone();
            }
            let delta = if reduce_motion {
                1.0
            } else {
                state
                    .started
                    .map(|t| (t.elapsed().as_secs_f32() / 0.24).min(1.0))
                    .unwrap_or(1.0)
            };
            let value = 1.0 - (1.0 - delta).powi(3);
            let bounds = if let Some(from) = state.from.filter(|_| delta < 1.0) {
                Bounds::new(
                    point(
                        from.origin.x + (target.origin.x - from.origin.x) * value,
                        from.origin.y + (target.origin.y - from.origin.y) * value,
                    ),
                    size(
                        from.size.width + (target.size.width - from.size.width) * value,
                        from.size.height + (target.size.height - from.size.height) * value,
                    ),
                )
            } else {
                target
            };
            state.current = Some(bounds);
            window.paint_quad(fill(bounds, color).corner_radii(px(6.)));
            if delta < 1.0 {
                window.request_animation_frame();
            }
        },
    )
    .absolute()
    .size_full()
}
