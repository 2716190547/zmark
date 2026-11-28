use super::WorkspaceView;
use crate::{actions::*, editor_engine::VisualLine, state::ViewMode, views::editor::MarkdownToken};
use gpui::*;
use std::{collections::HashMap, ops::Range, sync::Arc, time::Duration};

#[derive(Clone)]
pub struct SourceRow {
    pub bounds: Bounds<Pixels>,
    pub index: usize,
    pub positions: Arc<Vec<f32>>,
    pub visual: VisualLine,
    pub offset: usize,
    pub tokens: Vec<MarkdownToken>,
}

pub struct SourceComposition {
    pub ranges: Vec<Range<usize>>,
    pub active: Range<usize>,
    pub text: String,
    pub selected: Range<usize>,
    pub display: Arc<Vec<String>>,
    pub cursor: (usize, usize),
}

impl SourceComposition {
    fn original_offset(&self, offset: usize, end_boundary: bool) -> usize {
        let end = self.active.start + self.text.chars().count();
        if offset <= self.active.start {
            offset
        } else if offset >= end {
            offset - end + self.active.end
        } else if end_boundary {
            self.active.end
        } else {
            self.active.start
        }
    }
}

pub struct SourceDrag {
    pub button: MouseButton,
    pub geometry_key: (usize, u64, u32),
    pub column_positions: HashMap<usize, (usize, Arc<Vec<f32>>)>,
    pub origin: Point<Pixels>,
    pub pointer: Point<Pixels>,
    pub anchor: Range<usize>,
    pub granularity: usize,
}

#[derive(Clone, Copy)]
pub enum SourceCommand {
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    SelectAll,
    Delete,
    Bold,
    NumberedList,
    Indent,
    Outdent,
}

fn chars_in_lines(lines: &[String]) -> impl Iterator<Item = char> + '_ {
    lines.iter().enumerate().flat_map(move |(index, line)| {
        line.chars().chain((index + 1 < lines.len()).then_some('\n'))
    })
}

fn char_to_utf16(chars: impl Iterator<Item = char>, offset: usize) -> usize {
    chars.take(offset).map(char::len_utf16).sum()
}

fn utf16_to_char(chars: impl Iterator<Item = char>, offset: usize) -> Option<usize> {
    let mut units = 0;
    let mut count = 0;
    for ch in chars {
        if units == offset {
            return Some(count);
        }
        units += ch.len_utf16();
        if units > offset {
            return None;
        }
        count += 1;
    }
    (units == offset).then_some(count)
}

fn byte_for_char(text: &str, offset: usize) -> Option<usize> {
    if offset == text.chars().count() {
        Some(text.len())
    } else {
        text.char_indices().nth(offset).map(|(byte, _)| byte)
    }
}

fn position_in_lines(lines: &[String], mut offset: usize) -> (usize, usize) {
    for (index, line) in lines.iter().enumerate() {
        let length = line.chars().count();
        if offset <= length {
            return (index, offset);
        }
        offset -= length + 1;
    }
    let last = lines.len() - 1;
    (last, lines[last].chars().count())
}

impl WorkspaceView {
    pub fn source_accepts_input(&self, window: &Window, cx: &App) -> bool {
        !self.sidebar_busy
            && self.state.active.is_some()
            && self.state.view_mode != ViewMode::Preview
            && !self.preview_focused
            && !self.lightbox.is_open
            && !self.is_renaming
            && self.state.pending_close.is_none()
            && self.state.pending_remove_root.is_none()
            && self.focus_handle.is_focused(window)
            && !self
                .search_input
                .as_ref()
                .is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window))
    }

    fn source_input_replacement(
        &self,
        range: Option<Range<usize>>,
        text: &str,
    ) -> Option<(Vec<Range<usize>>, String, usize)> {
        let fallback = || {
            let ranges = self.state.editor.selection_ranges();
            if ranges.is_empty() {
                let offset = self
                    .state
                    .editor
                    .offset_for_position(self.state.editor.cursor());
                vec![offset..offset]
            } else {
                ranges
            }
        };
        let Some(range) = range else {
            return Some((
                self.source_composition
                    .as_ref()
                    .map_or_else(fallback, |composition| composition.ranges.clone()),
                text.to_owned(),
                0,
            ));
        };
        let display = self.source_display_lines();
        let (a, b) = (
            utf16_to_char(chars_in_lines(&display), range.start)?,
            utf16_to_char(chars_in_lines(&display), range.end)?,
        );
        if a > b {
            return None;
        }
        if let Some(composition) = &self.source_composition {
            let end = composition.active.start + composition.text.chars().count();
            if a >= composition.active.start && b <= end {
                let (start, stop) = (a - composition.active.start, b - composition.active.start);
                let mut updated = composition.text.clone();
                updated.replace_range(
                    byte_for_char(&composition.text, start)?
                        ..byte_for_char(&composition.text, stop)?,
                    text,
                );
                return Some((
                    composition.ranges.clone(),
                    updated,
                    char_to_utf16(composition.text.chars(), start),
                ));
            }
            return Some((
                vec![composition.original_offset(a, false)..composition.original_offset(b, true)],
                text.to_owned(),
                0,
            ));
        }
        Some((vec![a..b], text.to_owned(), 0))
    }

    pub fn source_display_lines(&self) -> Arc<Vec<String>> {
        self.source_composition.as_ref().map_or_else(
            || self.state.editor.lines.clone(),
            |composition| composition.display.clone(),
        )
    }

    pub fn source_display_cursor(&self) -> (usize, usize) {
        self.source_composition.as_ref().map_or_else(
            || self.state.editor.cursor(),
            |composition| composition.cursor,
        )
    }

    pub fn source_display_selection_ranges(&self) -> Vec<Range<usize>> {
        let Some(composition) = &self.source_composition else {
            return self.state.editor.selection_ranges();
        };
        let mut ranges = composition.ranges[..composition.ranges.len() - 1].to_vec();
        if composition.selected.start != composition.selected.end {
            let start = utf16_to_char(composition.text.chars(), composition.selected.start).unwrap();
            let end = utf16_to_char(composition.text.chars(), composition.selected.end).unwrap();
            ranges.push(composition.active.start + start..composition.active.start + end);
        }
        ranges
    }

    pub fn source_marked_range(&self) -> Option<Range<usize>> {
        self.source_composition.as_ref().map(|composition| {
            composition.active.start..composition.active.start + composition.text.chars().count()
        })
    }

    pub fn cancel_source_composition(&mut self) {
        if self.source_composition.take().is_some() {
            self.cached_visual_rev = usize::MAX;
        }
    }

    pub fn end_source_drag(&mut self) {
        self.source_drag = None;
        self.source_click_after_composition = None;
        self.source_autoscroll_task = None;
    }

    pub fn release_source_drag(&mut self, button: MouseButton) {
        let click = self
            .source_click_after_composition
            .take()
            .map(|(event, released)| {
                let released = released || event.button == button;
                (event, released)
            });
        self.end_source_drag();
        self.source_click_after_composition = click;
    }

    pub fn source_after_paint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((event, released)) = self.source_click_after_composition.take() {
            self.source_mouse_down(&event, window, cx);
            if released {
                self.end_source_drag();
            }
        } else {
            self.update_source_drag(window, cx);
        }
    }

    fn source_changed(&mut self, revision: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.editor.revision != revision {
            self.state.mark_editor_dirty();
            self.schedule_parse(cx);
            self.trigger_auto_save(cx);
            window.notify_text_input_changed(&self.focus_handle);
        } else {
            window.notify_text_selection_changed(&self.focus_handle);
        }
        cx.notify();
    }

    pub fn source_command(
        &mut self,
        command: SourceCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.source_accepts_input(window, cx) {
            return;
        }
        self.cancel_source_composition();
        self.end_source_drag();
        let revision = self.state.editor.revision;
        match command {
            SourceCommand::Undo => {
                self.state.editor.undo();
            }
            SourceCommand::Redo => {
                self.state.editor.redo();
            }
            SourceCommand::Copy | SourceCommand::Cut => {
                if let Some(text) = self.state.editor.selected_text() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    if matches!(command, SourceCommand::Cut) {
                        self.state.editor.delete_selection();
                    }
                }
            }
            SourceCommand::Paste => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    if let Err(error) = self.state.editor.paste_text(&text) {
                        self.error_banner = Some(error.into());
                    }
                }
            }
            SourceCommand::SelectAll => self.state.editor.select_all(),
            SourceCommand::Delete => {
                self.state.editor.delete_selection();
            }
            SourceCommand::Bold => self.state.editor.toggle_bold(),
            SourceCommand::NumberedList => self.state.editor.toggle_numbered_list(),
            SourceCommand::Indent => self
                .state
                .editor
                .indent_lines(self.state.preferences.tab_size, false),
            SourceCommand::Outdent => self
                .state
                .editor
                .indent_lines(self.state.preferences.tab_size, true),
        }
        self.source_changed(revision, window, cx);
    }

    pub fn source_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.source_accepts_input(window, cx) {
            return false;
        }
        let key = event.keystroke.key.to_lowercase();
        let modifiers = event.keystroke.modifiers;
        let command = if modifiers.secondary(cx) {
            match key.as_str() {
                "z" => Some(if modifiers.shift {
                    SourceCommand::Redo
                } else {
                    SourceCommand::Undo
                }),
                "y" if !cfg!(target_os = "macos") => Some(SourceCommand::Redo),
                "c" => Some(SourceCommand::Copy),
                "x" => Some(SourceCommand::Cut),
                "v" => Some(SourceCommand::Paste),
                "a" => Some(SourceCommand::SelectAll),
                _ => None,
            }
        } else if key == "tab" && !modifiers.control && !modifiers.alt {
            // Ctrl+Tab cycles documents; only plain and Shift+Tab indent.
            Some(if modifiers.shift {
                SourceCommand::Outdent
            } else {
                SourceCommand::Indent
            })
        } else {
            None
        };
        if let Some(command) = command {
            self.source_command(command, window, cx);
            return true;
        }
        if self.source_composition.is_some() {
            if key == "escape" {
                self.cancel_source_composition();
                window.notify_text_input_changed(&self.focus_handle);
                cx.notify();
                return true;
            }
            // The platform IME owns arrows, Enter and Backspace while composing.
            return false;
        }
        let by_word = if cfg!(target_os = "macos") {
            modifiers.alt
        } else {
            modifiers.control
        };
        let revision = self.state.editor.revision;
        match key.as_str() {
            "left" | "right" if by_word => self
                .state
                .editor
                .move_by_word(key == "right", modifiers.shift),
            "left" | "right" if cfg!(target_os = "macos") && modifiers.platform => {
                let (line, _) = self.state.editor.cursor();
                let col = if key == "left" {
                    0
                } else {
                    self.state.editor.line_len(line)
                };
                if modifiers.shift {
                    self.state.editor.select_to(line, col, true);
                } else {
                    self.state.editor.set_cursor(line, col);
                }
            }
            "left" | "right" => {
                if !modifiers.shift {
                    if key == "left" {
                        self.state.editor.move_left();
                    } else {
                        self.state.editor.move_right();
                    }
                } else {
                    let (line, col) = self.state.editor.cursor();
                    let position = if key == "left" {
                        if col > 0 {
                            (line, col - 1)
                        } else if line > 0 {
                            (line - 1, self.state.editor.line_len(line - 1))
                        } else {
                            (line, col)
                        }
                    } else if col < self.state.editor.line_len(line) {
                        (line, col + 1)
                    } else if line + 1 < self.state.editor.lines.len() {
                        (line + 1, 0)
                    } else {
                        (line, col)
                    };
                    self.state.editor.select_to(position.0, position.1, true);
                }
            }
            "up" | "down" => {
                let visual = self.compute_visual_lines(window);
                let (index, col) = crate::editor_engine::physical_to_visual(
                    self.state.editor.cursor_line,
                    self.state.editor.cursor_col,
                    &visual,
                );
                let next = if key == "up" {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(visual.len().saturating_sub(1))
                };
                let position = crate::editor_engine::visual_to_physical(next, col, &visual);
                if modifiers.shift {
                    self.state.editor.select_to(position.0, position.1, true);
                } else {
                    self.state.editor.set_cursor(position.0, position.1);
                }
            }
            "home" | "end" => {
                let (line, _) = self.state.editor.cursor();
                let col = if key == "home" {
                    0
                } else {
                    self.state.editor.line_len(line)
                };
                if modifiers.shift {
                    self.state.editor.select_to(line, col, true);
                } else {
                    self.state.editor.set_cursor(line, col);
                }
            }
            "backspace" => self.state.editor.delete_backward(),
            "delete" => self.state.editor.delete_forward(),
            "enter" => self.state.editor.insert_newline(),
            "escape" => self.state.editor.clear_selection(),
            _ => return false,
        }
        self.end_source_drag();
        self.source_changed(revision, window, cx);
        true
    }

    pub fn hit_source_point(
        &self,
        position: Point<Pixels>,
        _window: &Window,
    ) -> Option<(usize, usize, usize)> {
        if !f32::from(position.x).is_finite() || !f32::from(position.y).is_finite() {
            return None;
        }
        let rows = self.source_rows.lock().unwrap();
        let row = rows.iter().min_by(|a, b| {
            let distance = |row: &SourceRow| {
                (f32::from(position.y) - f32::from(row.bounds.top()))
                    .max(0.0)
                    .min(f32::from(row.bounds.size.height))
            };
            let actual =
                |row: &SourceRow| (f32::from(position.y - row.bounds.top()) - distance(row)).abs();
            actual(a).total_cmp(&actual(b))
        })?;
        let col = crate::views::editor::closest_char_position(
            &row.positions,
            f32::from(position.x - row.bounds.left()),
        )
        .min(row.visual.char_len);
        Some((
            row.index,
            row.visual.physical_line_idx,
            row.visual.char_start + col,
        ))
    }

    pub fn source_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.source_composition.is_some() {
            self.cancel_source_composition();
            self.end_source_drag();
            // Re-hit the click after the original buffer's rows are laid out again.
            self.source_click_after_composition = Some((event.clone(), false));
            window.notify_text_input_changed(&self.focus_handle);
            cx.notify();
            return;
        }
        self.end_source_drag();
        self.preview_focused = false;
        self.search_focused = false;
        window.focus(&self.focus_handle, cx);
        if !self.source_accepts_input(window, cx) {
            return;
        }
        let Some((_, line, col)) = self.hit_source_point(event.position, window) else {
            return;
        };
        self.preview_selection = None;
        self.is_preview_selecting = false;
        self.preview_autoscroll_task = None;
        if event.button == MouseButton::Right {
            let offset = self.state.editor.offset_for_position((line, col));
            if !self
                .state
                .editor
                .selection_ranges()
                .iter()
                .any(|range| range.contains(&offset))
            {
                self.state.editor.set_cursor(line, col);
            }
            self.source_menu().show(event.position, window, cx);
            cx.notify();
            return;
        }
        if !matches!(event.button, MouseButton::Left | MouseButton::Middle) {
            return;
        }
        if event.button == MouseButton::Middle {
            let offset = self.state.editor.offset_for_position((line, col));
            self.state.editor.set_column_ranges(vec![offset..offset]);
        } else if event.modifiers.shift {
            self.state.editor.select_to(line, col, true);
        } else if event.click_count >= 3 {
            self.state.editor.select_line(line);
        } else if event.click_count == 2 {
            self.state.editor.select_word(line, col);
        } else {
            self.state.editor.set_cursor(line, col);
        }
        let granularity = if event.modifiers.shift {
            1
        } else {
            event.click_count.clamp(1, 3)
        };
        let offset = self
            .state
            .editor
            .selection_anchor_offset()
            .unwrap_or_else(|| {
                self.state
                    .editor
                    .offset_for_position(self.state.editor.cursor())
            });
        let anchor = if granularity == 1 {
            offset..offset
        } else {
            self.state
                .editor
                .selection()
                .map_or(offset..offset, |(start, end)| start..end)
        };
        self.source_drag = Some(SourceDrag {
            button: event.button,
            geometry_key: (
                self.cached_visual_rev,
                self.cached_visual_width_bits,
                self.cached_visual_font_bits,
            ),
            column_positions: HashMap::new(),
            origin: event.position,
            pointer: event.position,
            anchor,
            granularity,
        });
        self.source_changed(self.state.editor.revision, window, cx);
    }

    pub fn update_source_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.source_accepts_input(window, cx) {
            self.end_source_drag();
            return;
        }
        let Some(mut drag) = self.source_drag.take() else {
            return;
        };
        let (button, origin, pointer, anchor, granularity) = (
            drag.button,
            drag.origin,
            drag.pointer,
            drag.anchor.clone(),
            drag.granularity,
        );
        let Some((visual_index, line, col)) = self.hit_source_point(pointer, window) else {
            self.source_drag = Some(drag);
            return;
        };
        let before = (
            self.state.editor.selection_ranges(),
            self.state.editor.cursor(),
        );
        if button == MouseButton::Middle {
            let visual_lines = self.cached_visual_lines.clone();
            let anchor_position = self.state.editor.position_for_offset(anchor.start);
            let anchor_visual = crate::editor_engine::physical_to_visual(
                anchor_position.0,
                anchor_position.1,
                &visual_lines,
            )
            .0;
            let key = (
                self.cached_visual_rev,
                self.cached_visual_width_bits,
                self.cached_visual_font_bits,
            );
            if drag.geometry_key != key {
                drag.column_positions.clear();
                drag.geometry_key = key;
            }
            let text_left = self
                .source_rows
                .lock()
                .unwrap()
                .first()
                .map(|row| row.bounds.left())
                .unwrap_or(px(0.));
            let x0 = f32::from(origin.x - text_left).min(f32::from(pointer.x - text_left));
            let x1 = f32::from(origin.x - text_left).max(f32::from(pointer.x - text_left));
            let mut ranges = Vec::new();
            for index in anchor_visual.min(visual_index)..=anchor_visual.max(visual_index) {
                let visual = &visual_lines[index];
                let (offset, positions) = drag.column_positions.entry(index).or_insert_with(|| {
                    if let Some(row) = self
                        .source_rows
                        .lock()
                        .unwrap()
                        .iter()
                        .find(|row| row.index == index)
                    {
                        return (row.offset, row.positions.clone());
                    }
                    let mut spans = HashMap::new();
                    let mut cache = self.syntax_cache.lock().unwrap();
                    for block in self.source_code_blocks.iter().filter(|block| {
                        visual.physical_line_idx >= block.fence_open_line
                            && visual.physical_line_idx
                                <= block
                                    .fence_close_line
                                    .unwrap_or(self.state.editor.lines.len() - 1)
                    }) {
                        if let Some(tokens) = cache.get(&block.key) {
                            spans.insert(block.key.clone(), tokens);
                        }
                    }
                    let tokens = crate::views::editor::physical_line_tokens(
                        visual.physical_line_idx,
                        &self.state.editor.lines[visual.physical_line_idx],
                        &self.source_code_blocks,
                        !self.state.doc().is_parse_stale(),
                        &spans,
                    );
                    let tokens = crate::views::editor::slice_tokens_for_visual_subline(
                        &tokens,
                        visual.char_start,
                        visual.char_len,
                    );
                    (
                        self.state
                            .editor
                            .offset_for_position((visual.physical_line_idx, visual.char_start)),
                        Arc::new(crate::views::editor::token_char_positions(
                            window.text_system(),
                            &tokens,
                            self.state.preferences.font_size,
                        )),
                    )
                });
                let a =
                    crate::views::editor::closest_char_position(positions, x0).min(visual.char_len);
                let b =
                    crate::views::editor::closest_char_position(positions, x1).min(visual.char_len);
                ranges.push(*offset + a.min(b)..*offset + a.max(b));
            }
            self.state.editor.set_column_ranges(ranges);
        } else {
            let mut end = self.state.editor.offset_for_position((line, col));
            let reversed = end < anchor.start;
            if granularity >= 3 {
                self.state.editor.select_line(line);
            } else if granularity == 2 {
                self.state.editor.select_word(line, col);
            }
            if granularity >= 2 {
                if let Some((a, b)) = self.state.editor.selection() {
                    end = if reversed { a } else { b };
                }
            }
            self.state.editor.set_selection_offsets(
                if reversed { end } else { anchor.start },
                if reversed { anchor.end } else { end },
                reversed,
            );
        }
        self.source_drag = Some(drag);
        if before
            != (
                self.state.editor.selection_ranges(),
                self.state.editor.cursor(),
            )
        {
            window.notify_text_selection_changed(&self.focus_handle);
            cx.notify();
        }
    }

    pub fn source_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.source_drag.as_mut() else {
            return;
        };
        if event.pressed_button != Some(drag.button) {
            self.end_source_drag();
            return;
        }
        drag.pointer = event.position;
        self.update_source_drag(window, cx);
        self.start_source_autoscroll(cx);
    }

    fn source_scroll_velocity(&self) -> f32 {
        let (Some(drag), Some(bounds)) = (&self.source_drag, self.editor_content_bounds) else {
            return 0.0;
        };
        let y = f32::from(drag.pointer.y);
        let top = f32::from(bounds.top()) + 24.0;
        let bottom = f32::from(bounds.bottom()) - 24.0;
        if y < top {
            -((top - y) * 0.3).min(18.0)
        } else if y > bottom {
            ((y - bottom) * 0.3).min(18.0)
        } else {
            0.0
        }
    }

    fn start_source_autoscroll(&mut self, cx: &mut Context<Self>) {
        if self.source_scroll_velocity() == 0.0 {
            self.source_autoscroll_task = None;
            return;
        }
        if self.source_autoscroll_task.is_some() {
            return;
        }
        let Some(handle) = self.window_handle else {
            return;
        };
        let view = cx.entity().downgrade();
        self.source_autoscroll_task = Some(cx.spawn(async move |_, app| loop {
            app.background_executor()
                .timer(Duration::from_millis(16))
                .await;
            let keep = handle
                .update(app, |_, window, cx| {
                    view.update(cx, |this, cx| {
                        if !this.source_accepts_input(window, cx) || this.source_drag.is_none() {
                            this.end_source_drag();
                            return false;
                        }
                        let velocity = this.source_scroll_velocity();
                        let scroll = this.editor_scroll_handle.0.borrow().base_handle.clone();
                        let old = scroll.offset();
                        let max = scroll.max_offset().y;
                        let next = point(old.x, (old.y - px(velocity)).clamp(-max, px(0.)));
                        if velocity == 0.0 || old == next {
                            this.source_autoscroll_task = None;
                            return false;
                        }
                        scroll.set_offset(next);
                        cx.notify();
                        true
                    })
                })
                .and_then(|result| result)
                .unwrap_or(false);
            if !keep {
                break;
            }
        }));
    }

    pub fn source_menu(&self) -> gpui_component::native_menu::NativeMenu {
        use gpui_component::native_menu::NativeMenu;
        let selected = self
            .state
            .editor
            .selection_ranges()
            .iter()
            .any(|range| !range.is_empty());
        let labels = source_labels(self.state.preferences.locale.resolve());
        NativeMenu::new()
            .menu_with_disabled(
                labels[0],
                !self.state.editor.can_undo(),
                Box::new(SourceUndo),
            )
            .menu_with_disabled(
                labels[1],
                !self.state.editor.can_redo(),
                Box::new(SourceRedo),
            )
            .separator()
            .menu_with_disabled(labels[2], !selected, Box::new(SourceCut))
            .menu_with_disabled(labels[3], !selected, Box::new(SourceCopy))
            .menu(labels[4], Box::new(SourcePaste))
            .menu(labels[5], Box::new(SourceSelectAll))
            .menu_with_disabled(labels[6], !selected, Box::new(SourceDelete))
            .separator()
            .menu_with_disabled(
                labels[7],
                self.state.editor.is_column_selection(),
                Box::new(SourceBold),
            )
            .menu_with_disabled(
                labels[8],
                self.state.editor.is_column_selection(),
                Box::new(SourceNumberedList),
            )
            .menu(labels[9], Box::new(SourceIndent))
            .menu(labels[10], Box::new(SourceOutdent))
    }
}

fn source_labels(locale: zmark_core::i18n::SupportedLocale) -> [&'static str; 11] {
    use zmark_core::i18n::SupportedLocale::*;
    match locale {
        ZhCn => [
            "撤销",
            "重做",
            "剪切",
            "复制",
            "粘贴",
            "全选",
            "删除",
            "加粗",
            "有序列表／段落编号",
            "增加缩进",
            "减少缩进",
        ],
        ZhTw => [
            "復原",
            "重做",
            "剪下",
            "複製",
            "貼上",
            "全選",
            "刪除",
            "粗體",
            "編號清單",
            "增加縮排",
            "減少縮排",
        ],
        Ja => [
            "元に戻す",
            "やり直す",
            "切り取り",
            "コピー",
            "貼り付け",
            "すべて選択",
            "削除",
            "太字",
            "番号付きリスト",
            "インデント",
            "インデント解除",
        ],
        Ko => [
            "실행 취소",
            "다시 실행",
            "잘라내기",
            "복사",
            "붙여넣기",
            "모두 선택",
            "삭제",
            "굵게",
            "번호 목록",
            "들여쓰기",
            "내어쓰기",
        ],
        Fr => [
            "Annuler",
            "Rétablir",
            "Couper",
            "Copier",
            "Coller",
            "Tout sélectionner",
            "Supprimer",
            "Gras",
            "Liste numérotée",
            "Indenter",
            "Désindenter",
        ],
        De => [
            "Rückgängig",
            "Wiederholen",
            "Ausschneiden",
            "Kopieren",
            "Einfügen",
            "Alles auswählen",
            "Löschen",
            "Fett",
            "Nummerierte Liste",
            "Einrücken",
            "Ausrücken",
        ],
        Es => [
            "Deshacer",
            "Rehacer",
            "Cortar",
            "Copiar",
            "Pegar",
            "Seleccionar todo",
            "Eliminar",
            "Negrita",
            "Lista numerada",
            "Aumentar sangría",
            "Reducir sangría",
        ],
        Ru => [
            "Отменить",
            "Повторить",
            "Вырезать",
            "Копировать",
            "Вставить",
            "Выделить всё",
            "Удалить",
            "Жирный",
            "Нумерованный список",
            "Увеличить отступ",
            "Уменьшить отступ",
        ],
        Pt => [
            "Desfazer",
            "Refazer",
            "Recortar",
            "Copiar",
            "Colar",
            "Selecionar tudo",
            "Excluir",
            "Negrito",
            "Lista numerada",
            "Aumentar recuo",
            "Diminuir recuo",
        ],
        _ => [
            "Undo",
            "Redo",
            "Cut",
            "Copy",
            "Paste",
            "Select All",
            "Delete",
            "Bold",
            "Numbered List",
            "Indent",
            "Outdent",
        ],
    }
}

impl EntityInputHandler for WorkspaceView {
    fn accepts_text_input(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.source_accepts_input(window, cx)
    }

    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        if !self.source_accepts_input(window, cx) {
            return;
        }
        self.cancel_source_composition();
        let revision = self.state.editor.revision;
        if let Some(text) = item.text() {
            if let Err(error) = self.state.editor.paste_text(&text) {
                self.error_banner = Some(error.into());
            }
        }
        self.source_changed(revision, window, cx);
    }

    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let lines = self.source_display_lines();
        let a = utf16_to_char(chars_in_lines(&lines), range.start)?;
        let b = utf16_to_char(chars_in_lines(&lines), range.end)?;
        if a > b {
            return None;
        }
        *adjusted = Some(range);
        Some(chars_in_lines(&lines).skip(a).take(b - a).collect())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        if !self.source_accepts_input(window, cx) {
            return None;
        }
        let lines = self.source_display_lines();
        if let Some(composition) = &self.source_composition {
            let start = char_to_utf16(chars_in_lines(&lines), composition.active.start);
            return Some(UTF16Selection {
                range: start + composition.selected.start..start + composition.selected.end,
                reversed: false,
            });
        }
        let cursor = self
            .state
            .editor
            .offset_for_position(self.state.editor.cursor());
        let range = self
            .state
            .editor
            .selection()
            .map_or(cursor..cursor, |(start, end)| start..end);
        Some(UTF16Selection {
            range: char_to_utf16(chars_in_lines(&lines), range.start)..char_to_utf16(chars_in_lines(&lines), range.end),
            reversed: self.state.editor.selection_reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let composition = self.source_composition.as_ref()?;
        let start = char_to_utf16(chars_in_lines(&composition.display), composition.active.start);
        Some(start..start + composition.text.encode_utf16().count())
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_source_composition();
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        let lines = self.source_display_lines();
        Some(chars_in_lines(&lines).map(char::len_utf16).sum())
    }

    fn set_selected_text_range(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.source_accepts_input(window, cx) {
            return;
        }
        let lines = self.source_display_lines();
        let (Some(mut a), Some(mut b)) = (
            utf16_to_char(chars_in_lines(&lines), range.start),
            utf16_to_char(chars_in_lines(&lines), range.end),
        ) else {
            return;
        };
        if a > b {
            return;
        }
        if let Some(composition) = self.source_composition.as_mut() {
            let end = composition.active.start + composition.text.chars().count();
            if a >= composition.active.start && b <= end {
                composition.selected =
                    char_to_utf16(composition.text.chars(), a - composition.active.start)
                        ..char_to_utf16(composition.text.chars(), b - composition.active.start);
                composition.cursor = position_in_lines(&composition.display, b);
                window.notify_text_selection_changed(&self.focus_handle);
                cx.notify();
                return;
            }
            a = composition.original_offset(a, false);
            b = composition.original_offset(b, true);
        }
        self.cancel_source_composition();
        self.state.editor.set_selection_offsets(a, b, false);
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.source_accepts_input(window, cx) {
            return;
        }
        let Some((ranges, replacement, _)) = self.source_input_replacement(range, text) else {
            return;
        };
        if replacement
            .len()
            .checked_mul(ranges.len())
            .is_none_or(|bytes| bytes > 8 * 1024 * 1024)
        {
            self.error_banner = Some("Edit exceeds the input budget".into());
            cx.notify();
            return;
        }
        let revision = self.state.editor.revision;
        if let Err(error) = self
            .state
            .editor
            .replace_ranges(&ranges, &vec![replacement; ranges.len()])
        {
            self.error_banner = Some(error.into());
            cx.notify();
            return;
        }
        self.cancel_source_composition();
        self.source_changed(revision, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.source_accepts_input(window, cx) || text.len() > 8 * 1024 * 1024 {
            return;
        }
        if text.is_empty() {
            self.unmark_text(window, cx);
            return;
        }
        let Some((ranges, replacement, selected_base)) = self.source_input_replacement(range, text)
        else {
            return;
        };
        if replacement.len() > 8 * 1024 * 1024 {
            return;
        }
        let original = self.state.editor.to_text();
        let active = ranges.last().unwrap().clone();
        let (Some(a), Some(b)) = (
            byte_for_char(&original, active.start),
            byte_for_char(&original, active.end),
        ) else {
            return;
        };
        let length = text.encode_utf16().count();
        let selected = selected.unwrap_or(length..length);
        if selected.start > selected.end
            || selected.end > length
            || utf16_to_char(text.chars(), selected.start).is_none()
        {
            return;
        }
        let selected = selected_base + selected.start..selected_base + selected.end;
        let Some(cursor_char) = utf16_to_char(replacement.chars(), selected.end) else {
            return;
        };
        let mut display = original;
        display.replace_range(a..b, &replacement);
        let display = Arc::new(display.split('\n').map(str::to_owned).collect::<Vec<_>>());
        let cursor = position_in_lines(&display, active.start + cursor_char);
        self.source_composition = Some(SourceComposition {
            ranges,
            active,
            text: replacement,
            selected,
            display,
            cursor,
        });
        self.cached_visual_rev = usize::MAX;
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn selection_bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Vec<Bounds<Pixels>> {
        let lines = self.source_display_lines();
        let (Some(start), Some(end)) = (
            utf16_to_char(chars_in_lines(&lines), range.start),
            utf16_to_char(chars_in_lines(&lines), range.end),
        ) else {
            return Vec::new();
        };
        if start > end {
            return Vec::new();
        }
        self.source_rows
            .lock()
            .unwrap()
            .iter()
            .filter_map(|row| {
                let a = start.max(row.offset);
                let b = end.min(row.offset + row.visual.char_len);
                if a > b || (a == b && start != end) {
                    return None;
                }
                let x0 = row.positions[a - row.offset];
                let x1 = row.positions[b - row.offset];
                Some(Bounds::new(
                    point(row.bounds.left() + px(x0.min(x1)), row.bounds.top()),
                    size(px((x1 - x0).abs().max(2.)), row.bounds.size.height),
                ))
            })
            .collect()
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.selection_bounds_for_range(range, bounds, window, cx)
            .into_iter()
            .next()
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let (_, line, col) = self.hit_source_point(position, window)?;
        let lines = self.source_display_lines();
        let offset = lines
            .iter()
            .take(line)
            .map(|line| line.chars().count() + 1)
            .sum::<usize>()
            + col;
        Some(char_to_utf16(chars_in_lines(&lines), offset))
    }
}
