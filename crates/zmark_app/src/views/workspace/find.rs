use super::*;
use gpui_component::input::{InputEvent, InputState};
use std::ops::Range;

#[derive(Default)]
pub(super) struct FindState {
    pub open: bool,
    pub input: Option<Entity<InputState>>,
    query: String,
    revision: Option<usize>,
    matches: Arc<FindMatches>,
    scroll: UniformListScrollHandle,
    current: usize,
}

// Keep one immutable revision and a bounded index instead of storing every match range and line.
const MAX_FIND_CHECKPOINTS: usize = 1024;

#[derive(Default)]
struct FindMatches {
    text: String,
    query: String,
    query_chars: usize,
    checkpoints: Vec<FindCheckpoint>,
    stride: usize,
    count: usize,
}
struct FindCheckpoint {
    byte: usize,
    chars: usize,
    line: usize,
}
struct FindMatch {
    range: Range<usize>,
    line: usize,
}

impl FindMatches {
    fn new(text: String, query: &str) -> Self {
        if query.is_empty() {
            return Self::default();
        }
        let mut checkpoints = Vec::new();
        let (mut stride, mut count) = (1, 0);
        let (mut byte, mut chars, mut line) = (0, 0, 0);
        for (index, (start, _)) in text.match_indices(query).enumerate() {
            count = index + 1;
            if index % stride != 0 {
                continue;
            }
            if checkpoints.len() == MAX_FIND_CHECKPOINTS {
                let mut position = 0;
                checkpoints.retain(|_| {
                    let keep = position % 2 == 0;
                    position += 1;
                    keep
                });
                stride *= 2;
                if index % stride != 0 {
                    continue;
                }
            }
            let prefix = &text[byte..start];
            chars += prefix.chars().count();
            line += prefix.bytes().filter(|&b| b == b'\n').count();
            byte = start;
            checkpoints.push(FindCheckpoint { byte, chars, line });
        }
        if count == 0 {
            return Self::default();
        }
        Self {
            text,
            query: query.to_owned(),
            query_chars: query.chars().count(),
            checkpoints,
            stride,
            count,
        }
    }

    fn get(&self, index: usize) -> Option<FindMatch> {
        if index >= self.count {
            return None;
        }
        let checkpoint = &self.checkpoints[index / self.stride];
        let text = &self.text[checkpoint.byte..];
        let (start, _) = text.match_indices(&self.query).nth(index % self.stride)?;
        let prefix = &text[..start];
        let chars = checkpoint.chars + prefix.chars().count();
        Some(FindMatch {
            range: chars..chars + self.query_chars,
            line: checkpoint.line + prefix.bytes().filter(|&b| b == b'\n').count(),
        })
    }
}

impl FindState {
    pub fn invalidate(&mut self) {
        self.revision = None;
        self.current = 0;
        self.matches = Arc::default();
    }
}

impl WorkspaceView {
    pub(super) fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar_busy
            || self.lightbox.is_open
            || self.is_renaming
            || self.state.pending_close.is_some()
            || self.state.pending_remove_root.is_some()
            || self.state.active.is_none()
        {
            return;
        }
        self.cancel_source_composition();
        if self.find.input.is_none() {
            let placeholder = self.state.strings.find_in_file;
            let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
            cx.subscribe_in(
                &input,
                window,
                |this, input, event, window, cx| match event {
                    InputEvent::Change => {
                        this.find.query = input.read(cx).value().to_string();
                        this.find.revision = None;
                        this.find.current = 0;
                        this.refresh_find();
                        this.jump_find(0, window, cx);
                    }
                    _ => {}
                },
            )
            .detach();
            self.find.input = Some(input);
        }
        self.find.open = true;
        self.refresh_find();
        let input = self.find.input.clone().unwrap();
        input.update(cx, |input, cx| {
            input.set_placeholder(self.state.strings.find_in_file, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub(super) fn refresh_find(&mut self) {
        if !self.find.open
            || self.state.active.is_none()
            || self.find.revision == Some(self.state.editor.revision)
        {
            return;
        }
        self.find.matches = if self.find.query.is_empty() {
            Arc::default()
        } else {
            Arc::new(FindMatches::new(
                self.state.editor.to_text(),
                &self.find.query,
            ))
        };
        self.find.current = self
            .find
            .current
            .min(self.find.matches.count.saturating_sub(1));
        self.find.revision = Some(self.state.editor.revision);
    }

    pub(super) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.find.open {
            return;
        }
        self.find.open = false;
        self.find.invalidate();
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    pub(super) fn step_find(
        &mut self,
        previous: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.find.open
            || self.lightbox.is_open
            || self.is_renaming
            || self.sidebar_busy
            || self.state.pending_close.is_some()
            || self.state.pending_remove_root.is_some()
        {
            return;
        }
        self.refresh_find();
        let count = self.find.matches.count;
        if count == 0 {
            return;
        }
        let index = if previous {
            (self.find.current + count - 1) % count
        } else {
            (self.find.current + 1) % count
        };
        self.jump_find(index, window, cx);
    }

    fn jump_find(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(found) = self.find.matches.get(index) else {
            cx.notify();
            return;
        };
        let range = found.range;
        self.cancel_source_composition();
        if self.state.view_mode == ViewMode::Preview {
            self.set_view_mode(ViewMode::Split, cx);
        }
        self.find.current = index;
        self.find
            .scroll
            .scroll_to_item(index, ScrollStrategy::Center);
        self.preview_focused = false;
        self.state
            .editor
            .set_selection_offsets(range.start, range.end, false);
        let (line, col) = self.state.editor.position_for_offset(range.start);
        let visual = self.compute_visual_lines(window);
        let row = visual
            .iter()
            .position(|v| {
                v.physical_line_idx == line
                    && col >= v.char_start
                    && col < v.char_start + v.char_len.max(1)
            })
            .unwrap_or(0);
        self.editor_scroll_handle
            .scroll_to_item_strict(row, ScrollStrategy::Top);
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    pub(super) fn render_find(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.state.theme;
        let strings = self.state.strings;
        let count = self.find.matches.count;
        let text_start = if self.state.view_mode == ViewMode::Preview {
            calculate_title_header_padding(
                cfg!(target_os = "macos"),
                window.is_fullscreen(),
                self.state.drawer_open,
                self.state.view_mode,
                f32::from(window.viewport_size().width),
                self.state.preferences.max_column_width,
                self.editor_text_start_offset(window),
            ) + 8.0
        } else {
            self.editor_text_start_offset(window)
        };
        let gutter = (text_start - 16.0).max(0.0);
        let mut panel = div()
            .key_context("FileFind")
            .flex()
            .flex_col()
            .px_4()
            .py_2()
            .gap_1()
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.text_muted.opacity(0.2));
        let mut bar = div().pl(px(gutter)).flex().items_center().gap_2();
        if let Some(input) = &self.find.input {
            bar = bar.child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .child(crate::views::drawer::render_input(input, false)),
            );
        }
        bar = bar.child(
            div()
                .text_size(px(11.))
                .text_color(theme.text_muted)
                .child(format!(
                    "{} / {}",
                    if count == 0 { 0 } else { self.find.current + 1 },
                    count
                )),
        );
        for (id, icon, label, action) in [
            (
                "find_previous",
                "icons/arrow-up.svg",
                format!("{} · ↑ / Shift+Enter", strings.find_previous),
                0,
            ),
            (
                "find_next",
                "icons/arrow-down.svg",
                format!("{} · ↓ / Enter", strings.find_next),
                1,
            ),
            (
                "find_close",
                "icons/x.svg",
                format!("{} · Esc", strings.modal_cancel),
                2,
            ),
        ] {
            bar = bar.child(
                div()
                    .id(id)
                    .size(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .tooltip(move |_, cx| {
                        cx.new(|_| TooltipView {
                            text: label.to_owned(),
                        })
                        .into()
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            if action == 2 {
                                this.close_find(window, cx);
                            } else {
                                this.step_find(action == 0, window, cx);
                            }
                        }),
                    )
                    .child(svg().path(icon).size(px(14.)).text_color(theme.text_muted)),
            );
        }
        panel = panel.child(bar);
        let lines = self.state.editor.lines.clone();
        let matches = self.find.matches.clone();
        let current = self.find.current;
        let selected = theme.control_selection_bg();
        let foreground = theme.selection_text();
        let view = cx.entity().clone();
        if count > 0 {
            panel = panel.child(
                uniform_list("find_results", count, move |range, _, _| {
                    range
                        .map(|index| {
                            let line = matches
                                .get(index)
                                .expect("result index is within count")
                                .line;
                            let snippet: String = lines[line].trim().chars().take(240).collect();
                            let view = view.clone();
                            div()
                                .id(("find_result", index))
                                .debug_selector(move || format!("find_result_{index}"))
                                .h(px(28.))
                                .pr_0()
                                .py_1()
                                .rounded(px(4.))
                                .flex()
                                .items_center()
                                .text_size(px(12.))
                                .cursor_pointer()
                                .text_color(foreground)
                                .when(index == current, |el| el.bg(selected))
                                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                    cx.stop_propagation();
                                    view.update(cx, |this, cx| this.jump_find(index, window, cx));
                                })
                                .child(
                                    div()
                                        .w(px(gutter))
                                        .flex_shrink_0()
                                        .pr_2()
                                        .text_right()
                                        .text_color(foreground.opacity(0.65))
                                        .child((line + 1).to_string()),
                                )
                                .child(div().min_w(px(0.)).flex_1().truncate().child(snippet))
                        })
                        .collect()
                })
                .h(px(28. * count.min(5) as f32))
                .w_full()
                .track_scroll(&self.find.scroll),
            );
        }
        panel.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{FindMatches, FindState, MAX_FIND_CHECKPOINTS};

    fn ranges(text: &str, query: &str) -> Vec<std::ops::Range<usize>> {
        let matches = FindMatches::new(text.to_owned(), query);
        (0..matches.count)
            .map(|i| matches.get(i).unwrap().range)
            .collect()
    }

    #[test]
    fn unicode_offsets_and_literal_matching() {
        assert_eq!(ranges("中🙂a 中🙂a", "🙂a"), vec![1..3, 5..7]);
        assert_eq!(ranges("a.a", "."), vec![1..2]);
        assert!(ranges("abc", "").is_empty());
        assert_eq!(ranges("a\n中🙂\na", "中🙂\na"), vec![2..6]);
        assert!(ranges("abc", "missing").is_empty());
        assert_eq!(ranges("aaaaa", "aa"), vec![0..2, 2..4]);
    }

    #[test]
    fn dense_results_are_bounded_and_every_match_remains_reachable() {
        let matches = FindMatches::new("中🙂a\n".repeat(20_000), "🙂a\n中");
        assert_eq!(matches.count, 19_999);
        assert!(matches.checkpoints.capacity() <= MAX_FIND_CHECKPOINTS);
        for index in 0..matches.count {
            let found = matches.get(index).unwrap();
            assert_eq!(found.range, index * 4 + 1..index * 4 + 5);
            assert_eq!(found.line, index);
        }
        assert!(matches.get(matches.count).is_none());
        let dense = FindMatches::new("a".repeat(2 * 1024 * 1024), "a");
        assert_eq!(dense.count, 2 * 1024 * 1024);
        assert!(dense.checkpoints.capacity() <= MAX_FIND_CHECKPOINTS);
        assert_eq!(
            dense.get(dense.count - 1).unwrap().range,
            dense.count - 1..dense.count
        );
    }

    #[test]
    fn invalidation_releases_the_revision_and_index() {
        let mut find = FindState::default();
        find.matches = std::sync::Arc::new(FindMatches::new("a".repeat(2 * 1024 * 1024), "a"));
        let previous = std::sync::Arc::downgrade(&find.matches);
        find.invalidate();
        assert!(previous.upgrade().is_none());
        assert!(find.matches.text.is_empty());
        assert_eq!(find.matches.checkpoints.capacity(), 0);
    }
}
