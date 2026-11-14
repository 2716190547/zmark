use super::*;
use gpui_component::input::{InputEvent, InputState};
use std::ops::Range;

#[derive(Default)]
pub(super) struct FindState {
    pub open: bool,
    pub input: Option<Entity<InputState>>,
    query: String,
    revision: Option<usize>,
    matches: Vec<Range<usize>>,
    current: usize,
}

// Editor offsets count Unicode characters, while match_indices returns byte offsets.
fn find_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    let mut byte = 0;
    let mut chars = 0;
    let length = query.chars().count();
    text.match_indices(query)
        .map(|(start, matched)| {
            chars += text[byte..start].chars().count();
            let range = chars..chars + length;
            chars += length;
            byte = start + matched.len();
            range
        })
        .collect()
}

impl FindState {
    pub fn invalidate(&mut self) {
        self.revision = None;
        self.current = 0;
        self.matches.clear();
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
                    InputEvent::PressEnter { shift, .. } => this.step_find(*shift, window, cx),
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
        self.find.matches = find_ranges(&self.state.editor.to_text(), &self.find.query);
        self.find.current = self
            .find
            .current
            .min(self.find.matches.len().saturating_sub(1));
        self.find.revision = Some(self.state.editor.revision);
    }

    pub(super) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.find.open {
            return;
        }
        self.find.open = false;
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
        let count = self.find.matches.len();
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
        let Some(range) = self.find.matches.get(index).cloned() else {
            cx.notify();
            return;
        };
        self.cancel_source_composition();
        if self.state.view_mode == ViewMode::Preview {
            self.set_view_mode(ViewMode::Split, cx);
        }
        self.find.current = index;
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

    pub(super) fn render_find(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = &self.state.theme;
        let strings = self.state.strings;
        let count = self.find.matches.len();
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
        let mut bar = div().flex().items_center().gap_2();
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
        // Keep the panel compact even when a document contains thousands of matches.
        let first = self
            .find
            .current
            .saturating_sub(2)
            .min(count.saturating_sub(5));
        for index in first..(first + 5).min(count) {
            let (line, _) = self
                .state
                .editor
                .position_for_offset(self.find.matches[index].start);
            let snippet = self
                .state
                .editor
                .lines
                .get(line)
                .map(|s| s.trim())
                .unwrap_or("");
            panel = panel.child(
                div()
                    .id(("find_result", index))
                    .px_2()
                    .py_1()
                    .rounded(px(4.))
                    .text_size(px(12.))
                    .truncate()
                    .cursor_pointer()
                    .when(index == self.find.current, |el| {
                        el.bg(theme.control_selection_bg())
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.jump_find(index, window, cx);
                        }),
                    )
                    .child(format!("{}: {}", line + 1, snippet)),
            );
        }
        panel.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::find_ranges;
    #[test]
    fn unicode_offsets_and_literal_matching() {
        assert_eq!(find_ranges("中🙂a 中🙂a", "🙂a"), vec![1..3, 5..7]);
        assert_eq!(find_ranges("a.a", "."), vec![1..2]);
        assert!(find_ranges("abc", "").is_empty());
        assert_eq!(find_ranges("a\n中🙂\na", "中🙂\na"), vec![2..6]);
        assert!(find_ranges("abc", "missing").is_empty());
    }
}
