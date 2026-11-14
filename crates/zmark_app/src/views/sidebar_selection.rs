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
        self.focus = Some(key);
    }
}

pub fn row_anchor(state: SharedSelection, key: DocKey) -> impl IntoElement {
    let paint_state = state.clone();
    let paint_key = key.clone();
    canvas(
        move |bounds, _window, _| {
            state.lock().unwrap().rows.insert(key.clone(), bounds);
            bounds
        },
        move |_, bounds, window, _| {
            let state = paint_state.lock().unwrap();
            if state.selected.len() > 1 && state.selected.contains(&paint_key) {
                let mark = Bounds::new(
                    point(bounds.right() - px(56.), bounds.top() + px(10.)),
                    size(px(8.), px(8.)),
                );
                window.paint_quad(fill(mark, rgb(0x777777)).corner_radii(px(2.)));
                let mut tick = PathBuilder::stroke(px(1.2));
                tick.move_to(point(mark.left() + px(1.5), mark.top() + px(4.)));
                tick.line_to(point(mark.left() + px(3.3), mark.top() + px(6.)));
                tick.line_to(point(mark.left() + px(6.5), mark.top() + px(2.)));
                if let Ok(path) = tick.build() {
                    window.paint_path(path, rgb(0xffffff));
                }
            }
        },
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
