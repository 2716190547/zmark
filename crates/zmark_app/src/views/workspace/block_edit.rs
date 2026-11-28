//! Preview block editing (`Preferences::preview_inline_edit`, ADR 0041).
//!
//! A click on an editable preview block swaps that block for a small source
//! editor over its own lines. The editor is another view onto the shared
//! `EditorEngine`: keys, IME, undo, dirty state and auto-save go through the
//! source editor's paths, and only the row geometry (`block_rows`) and the
//! wrapping (`block_visual`) are its own. The preview is not reparsed while
//! the block is open; leaving it parses once.

use super::{SourceRow, WorkspaceView};
use crate::editor_engine::VisualLine;
use crate::state::ViewMode;
use crate::views::editor::{calculate_line_row_height, slice_tokens_for_visual_subline, token_char_positions, tokenize_markdown_line};
use gpui::*;
use std::{ops::Range, sync::Arc};
use zmark_core::ast::BlockElement;

/// Horizontal and vertical inset of the editor box inside the block column.
const BOX_PAD_X: f32 = 10.0;
const BOX_PAD_Y: f32 = 6.0;
/// A press that moves less than this before release is a click, not a drag.
pub const CLICK_SLOP: f32 = 4.0;
/// Duration of the open/close height transition.
const MOTION: std::time::Duration = std::time::Duration::from_millis(220);

/// Wrapped rows of the block editor: visual line, char offset, tokens, char x positions.
pub type BlockRows = Arc<Vec<(VisualLine, usize, Vec<crate::views::editor::MarkdownToken>, Arc<Vec<f32>>)>>;
/// Hash of the block text and caret base offset, range, width, font and line-height bits the rows were built for.
pub type BlockLayoutKey = (usize, usize, usize, usize, u32, u32);

/// A running open/close transition over `range` (indices of the current parse).
#[derive(Clone, Debug)]
pub struct BlockMotionState {
    pub range: Range<usize>,
    pub from: f32,
    pub opening: bool,
    pub started: std::time::Instant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockEdit {
    /// First source line of the edited range.
    pub start_line: usize,
    /// Lines after the edited range. Input stays inside the range, so this is fixed.
    pub tail: usize,
    /// Character offset of `start_line` in the buffer; edits after it do not move it.
    pub start_offset: usize,
    /// Parsed blocks (from the last parse) that the editor stands in for.
    pub blocks: Range<usize>,
    /// A fenced or indented code block: lines are code, not Markdown.
    pub code: bool,
}

impl BlockEdit {
    /// Last edited line for a buffer of `line_count` lines, or `None` once the
    /// range has collapsed past its start.
    pub fn end_line(&self, line_count: usize) -> Option<usize> {
        line_count
            .checked_sub(1 + self.tail)
            .filter(|end| *end >= self.start_line)
    }
}

/// Block shift caused by leaving a block edit: blocks from `after` on moved by `delta`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockShift {
    pub after: usize,
    pub delta: isize,
}

impl BlockShift {
    pub fn apply(self, ix: usize) -> usize {
        if ix >= self.after {
            (ix as isize + self.delta).max(0) as usize
        } else {
            ix
        }
    }
}

/// The editor shown in place of `blocks` in the preview list.
#[derive(Clone)]
pub struct BlockEditorSlot {
    pub blocks: Range<usize>,
    pub build: Arc<dyn Fn() -> AnyElement>,
}

pub fn block_is_editable(block: &BlockElement) -> bool {
    matches!(
        block,
        BlockElement::Paragraph(_)
            | BlockElement::Heading { .. }
            | BlockElement::Rule
            | BlockElement::FencedCode { .. }
            | BlockElement::List { .. }
            | BlockElement::BlockQuote(_)
            | BlockElement::Table { .. }
    )
}

/// Maps a character offset in a block's rendered text to a character offset in
/// its source. Rendered characters are matched to source characters in order;
/// markup and rendered-only characters (bullets, cell separators) are skipped.
pub fn source_offset_for_rendered(source: &str, rendered: &str, offset: usize) -> usize {
    let src: Vec<char> = source.chars().collect();
    let mut next = 0;
    for (index, ch) in rendered.chars().enumerate() {
        let found = src[next..].iter().position(|c| *c == ch).map(|p| next + p);
        if index == offset {
            return found.unwrap_or(next);
        }
        if let Some(found) = found {
            next = found + 1;
        }
    }
    next.min(src.len())
}

/// Buffer line indices of an anchor. `SourceAnchor` lines are 1-based (the
/// sync-scroll map compares them with `cursor_line + 1`).
pub fn anchor_lines(anchor: &zmark_core::anchor::SourceAnchor) -> (usize, usize) {
    let start = anchor.source_line_start.saturating_sub(1);
    (start, anchor.source_line_end.saturating_sub(1).max(start))
}

/// Whether `line` opens or closes a code fence.
fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// Source offset for a click at `offset` characters into a code block's rendered
/// text, which is only the code: skips the opening fence line and stays before the
/// closing one.
fn code_source_offset(lines: &[String], offset: usize) -> usize {
    let total: usize = lines.iter().map(|l| l.chars().count() + 1).sum::<usize>().saturating_sub(1);
    let fenced = lines.first().is_some_and(|l| is_fence(l));
    let start = if fenced { lines[0].chars().count() + 1 } else { 0 };
    let closed = lines.len() > 1 && lines.last().is_some_and(|l| is_fence(l));
    let end = if closed { total - lines[lines.len() - 1].chars().count() - 1 } else { total };
    (start + offset).min(end.max(start))
}

/// Largest code payload highlighted live; bigger blocks edit without colours so a
/// keystroke never waits on a full re-highlight.
const LIVE_SYNTAX_MAX_BYTES: usize = 24 * 1024;
const LIVE_SYNTAX_MAX_LINES: usize = 600;

/// Tokens for every line of a code block being edited: fences are muted, code lines
/// carry syntax roles for the fence's language (plain code colour when it is unknown
/// or the block is too large).
pub(crate) fn code_block_tokens(lines: &[String]) -> Vec<Vec<crate::views::editor::MarkdownToken>> {
    use crate::views::editor::{spans_to_code_tokens, MarkdownToken, MarkdownTokenKind};
    let fenced = lines.first().is_some_and(|l| is_fence(l));
    let closed = fenced && lines.len() > 1 && lines.last().is_some_and(|l| is_fence(l));
    let first = usize::from(fenced);
    let end = lines.len() - usize::from(closed);
    let code = &lines[first.min(end)..end];
    let info = if fenced {
        lines[0].trim_start().trim_start_matches(['`', '~']).trim().to_string()
    } else {
        String::new()
    };
    let payload = code.join("\n");
    let spans = if !info.is_empty() && !payload.is_empty() && payload.len() <= LIVE_SYNTAX_MAX_BYTES && code.len() <= LIVE_SYNTAX_MAX_LINES {
        zmark_core::syntax::parse_code_spans_bounded(&payload, Some(&info), None).spans().cloned()
    } else {
        None
    };
    let mut tokens = Vec::with_capacity(lines.len());
    let mut payload_start = 0;
    for (ix, line) in lines.iter().enumerate() {
        if ix < first || ix >= end {
            tokens.push(vec![MarkdownToken { text: line.clone(), kind: MarkdownTokenKind::CodeDelimiter }]);
            continue;
        }
        let len = line.len();
        let line_tokens = if line.is_empty() {
            Vec::new()
        } else {
            let line_spans: Vec<_> = spans.iter().flat_map(|spans| spans.iter()).filter_map(|span| {
                let (a, b) = (span.range.start.max(payload_start), span.range.end.min(payload_start + len));
                (a < b).then(|| zmark_core::syntax::SyntaxSpan { range: a - payload_start..b - payload_start, role: span.role })
            }).collect();
            spans_to_code_tokens(line, &line_spans)
        };
        tokens.push(line_tokens);
        payload_start += len + 1;
    }
    tokens
}

/// (line, column) of character `offset` within `lines`, relative to the first line.
fn position_in(lines: &[String], mut offset: usize) -> (usize, usize) {
    for (index, line) in lines.iter().enumerate() {
        let len = line.chars().count();
        if offset <= len {
            return (index, offset);
        }
        offset -= len + 1;
    }
    let last = lines.len().saturating_sub(1);
    (last, lines.get(last).map_or(0, |l| l.chars().count()))
}

impl WorkspaceView {
    /// Whether a click on preview block `ix` would open it for editing.
    pub fn block_click_edits(&self, ix: usize) -> bool {
        self.state.preferences.preview_inline_edit
            && self.state.view_mode != ViewMode::Source
            && !self.state.doc().is_parse_stale()
            && self
                .state
                .doc()
                .parsed
                .blocks
                .get(ix)
                .is_some_and(block_is_editable)
    }

    /// Opens preview block `ix` for editing with the caret near rendered offset `offset`.
    pub fn enter_block_edit(
        &mut self,
        ix: usize,
        offset: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.block_click_edits(ix) || self.lightbox.is_open || self.workspace_shortcut_blocked() {
            return false;
        }
        let parsed = self.state.doc().parsed.clone();
        let Some(anchor) = parsed.anchors.get(ix) else { return false };
        let lines = self.state.editor.lines.clone();
        let (start, end) = anchor_lines(anchor);
        if start >= lines.len() {
            return false;
        }
        let end = end.min(lines.len() - 1);
        let block_lines = &lines[start..=end];
        let rendered = zmark_core::ast::flatten_block(&parsed.blocks[ix], "0").text;
        let source = block_lines.join("\n");
        let code = matches!(parsed.blocks[ix], BlockElement::FencedCode { .. });
        let source_offset = if code {
            code_source_offset(block_lines, offset)
        } else {
            source_offset_for_rendered(&source, &rendered, offset)
        };
        let (line, col) = position_in(block_lines, source_offset);
        self.cancel_source_composition();
        self.end_source_drag();
        self.start_block_motion(ix..ix + 1, true);
        self.state.editor.set_cursor(start + line, col);
        self.block_edit = Some(BlockEdit {
            start_line: start,
            tail: lines.len() - 1 - end,
            start_offset: self.state.editor.offset_for_position((start, 0)),
            blocks: ix..ix + 1,
            code,
        });
        self.preview_selection = None;
        self.end_preview_selection();
        self.preview_focused = false;
        self.search_focused = false;
        window.focus(&self.focus_handle, cx);
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
        true
    }

    /// Closes the block editor and parses the edit. Returns how later blocks moved.
    pub fn exit_block_edit(&mut self, cx: &mut Context<Self>) -> Option<BlockShift> {
        let edit = self.block_edit.take()?;
        let editor_height: f32 = self.preview_list.sizes().get(edit.blocks.clone()).map_or(0.0, |s| s.iter().sum());
        self.block_rows.lock().unwrap().clear();
        self.block_visual = Arc::new(Vec::new());
        self.block_layout = None;
        self.cancel_source_composition();
        self.end_source_drag();
        let before = self.state.doc().parsed.blocks.len();
        self.state.sync_from_editor();
        if crate::state::parse_debounce(self.state.doc().buffer.len()).is_some() {
            self.parse_task = None;
            self.parse_paused = false;
            if self.state.parse_now() {
                self.prune_syntax_state();
                self.invalidate_preview_list();
            }
            self.schedule_syntax_parse(cx);
        } else {
            self.schedule_parse(cx);
        }
        cx.notify();
        let after = self.state.doc().parsed.blocks.len();
        let shift = BlockShift { after: edit.blocks.end, delta: after as isize - before as isize };
        // Motions still running refer to the old parse.
        for motion in &mut self.block_motions {
            motion.range = shift.apply(motion.range.start)..shift.apply(motion.range.end);
        }
        self.block_motions.retain(|m| m.range.end <= edit.blocks.start || m.range.start >= shift.apply(edit.blocks.end));
        let closed = edit.blocks.start..shift.apply(edit.blocks.end).max(edit.blocks.start);
        if !closed.is_empty() {
            self.block_motions.push(BlockMotionState {
                range: closed, from: editor_height, opening: false, started: std::time::Instant::now(),
            });
            if self.state.preferences.reduce_motion { self.block_motions.clear(); }
        }
        Some(shift)
    }

    /// Drops the block editor without parsing (document swap, reload).
    pub fn discard_block_edit(&mut self) {
        self.block_motions.clear();
        if self.block_edit.take().is_some() {
            self.block_rows.lock().unwrap().clear();
            self.block_visual = Arc::new(Vec::new());
            self.block_layout = None;
        }
    }

    /// After a parse while editing (save, export), points the editor at the new
    /// blocks covering its lines; closes it when none remain.
    pub fn retarget_block_edit(&mut self) {
        let Some(edit) = self.block_edit.as_mut() else { return };
        let Some(end) = edit.end_line(self.state.editor.lines.len()) else {
            self.discard_block_edit();
            return;
        };
        let anchors = &self.state.doc().parsed.anchors;
        let covered: Vec<usize> = anchors
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                let (a_start, a_end) = anchor_lines(a);
                a_end >= edit.start_line && a_start <= end
            })
            .map(|(ix, _)| ix)
            .collect();
        match (covered.first(), covered.last()) {
            (Some(&first), Some(&last)) => edit.blocks = first..last + 1,
            _ => self.discard_block_edit(),
        }
    }

    fn start_block_motion(&mut self, range: Range<usize>, opening: bool) {
        self.block_motions.retain(|m| m.range.end <= range.start || m.range.start >= range.end);
        if self.state.preferences.reduce_motion {
            return;
        }
        let from = self.preview_list.sizes().get(range.clone()).map_or(0.0, |s| s.iter().sum());
        self.block_motions.push(BlockMotionState { range, from, opening, started: std::time::Instant::now() });
    }

    /// Current eased state of the running transitions; keeps frames coming while any runs.
    pub fn sample_block_motions(&mut self, window: &mut Window) -> Vec<crate::views::preview::BlockMotion> {
        let now = std::time::Instant::now();
        self.block_motions.retain(|m| now.duration_since(m.started) < MOTION);
        if self.block_motions.is_empty() {
            return Vec::new();
        }
        window.request_animation_frame();
        let ease = ease_out_quint();
        self.block_motions.iter().map(|m| {
            let t = now.duration_since(m.started).as_secs_f32() / MOTION.as_secs_f32();
            crate::views::preview::BlockMotion {
                height: crate::views::preview::HeightMotion { range: m.range.clone(), from: m.from, progress: ease(t.clamp(0.0, 1.0)) },
                opening: m.opening,
            }
        }).collect()
    }

    /// Whether the caret is still on the edited lines.
    pub fn block_caret_inside(&self) -> bool {
        let Some(edit) = &self.block_edit else { return true };
        let Some(end) = edit.end_line(self.state.editor.lines.len()) else { return false };
        let line = self.state.editor.cursor().0;
        line >= edit.start_line && line <= end
    }

    /// Handles a press on preview block `ix` (not the open editor). Closes an open
    /// block edit first, so `ix` and `offset` refer to the parse they were hit on.
    pub fn preview_block_press(
        &mut self,
        ix: usize,
        offset: usize,
        click_count: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.lightbox.is_open {
            return;
        }
        let ix = self.exit_block_edit(cx).map_or(ix, |shift| shift.apply(ix));
        let pos = crate::views::preview::PreviewCharPosition::new(ix, offset);
        self.preview_selection = Some(crate::views::preview::PreviewSelection::new(pos, pos));
        self.is_preview_selecting = true;
        self.preview_focused = true;
        self.preview_autoscroll_accum = 0.0;
        window.focus(&self.focus_handle, cx);
        self.preview_mouse_window_pos = Some(window.mouse_position());
        self.trigger_preview_autoscroll_if_needed(cx);
        self.pending_block_click = (click_count == 1 && self.block_click_edits(ix)).then_some((ix, offset, position));
        cx.notify();
    }

    /// Mouse release: a press that stayed in place opens its block.
    pub fn finish_block_click(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some((ix, offset, origin)) = self.pending_block_click.take() else { return };
        let moved = (f32::from(position.x - origin.x)).hypot(f32::from(position.y - origin.y));
        let collapsed = self.preview_selection.is_none_or(|s| s.anchor == s.head);
        if moved < CLICK_SLOP && collapsed {
            self.enter_block_edit(ix, offset, window, cx);
        }
    }

    /// Wraps the edited lines at the preview column width and returns the editor
    /// element builder for the preview list, or `None` when no block is open.
    pub fn block_editor_slot(&mut self, window: &Window, cx: &mut Context<Self>) -> Option<BlockEditorSlot> {
        let edit = self.block_edit.clone()?;
        let display = self.source_display_lines();
        let Some(end) = edit.end_line(display.len()) else {
            // The buffer was replaced under the editor (reload from disk).
            self.discard_block_edit();
            return None;
        };
        let font_size = self.state.preferences.font_size;
        let line_height = zmark_core::config::Preferences::sanitize_line_height(self.state.preferences.line_height);
        let max_column = self.state.preferences.max_column_width;
        let pane_width = f32::from(self.preview_list.scroll_handle().bounds().size.width);
        let column = if pane_width > 1.0 { (pane_width - 64.0).min(max_column) } else { max_column }.max(40.0);
        let text_width = (column - 2.0 * BOX_PAD_X).max(20.0);

        // Animation frames re-render without edits; reuse the shaped rows.
        let text_hash = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            display[edit.start_line..=end].hash(&mut hasher);
            edit.start_offset.hash(&mut hasher);
            hasher.finish() as usize
        };
        let key: BlockLayoutKey = (
            text_hash,
            edit.start_line,
            end,
            text_width.to_bits() as usize,
            font_size.to_bits(),
            line_height.to_bits(),
        );
        let rows = match &self.block_layout {
            Some((cached, rows)) if *cached == key => rows.clone(),
            _ => {
                let mut visual = Vec::new();
                let mut rows = Vec::new();
                let mut line_offset = edit.start_offset;
                let code_tokens = edit.code.then(|| code_block_tokens(&display[edit.start_line..=end]));
                for phys in edit.start_line..=end {
                    let line = &display[phys];
                    let tokens = match &code_tokens {
                        Some(all) => all[phys - edit.start_line].clone(),
                        None => tokenize_markdown_line(line),
                    };
                    let (wrapped, _) = crate::views::editor::wrap_line_to_visual_layout_with_width(
                        window.text_system(), phys, line, &tokens, text_width, font_size,
                    );
                    for v in wrapped {
                        let sub = slice_tokens_for_visual_subline(&tokens, v.char_start, v.char_len);
                        let positions = Arc::new(token_char_positions(window.text_system(), &sub, font_size));
                        rows.push((v.clone(), line_offset + v.char_start, sub, positions));
                        visual.push(v);
                    }
                    line_offset += line.chars().count() + 1;
                }
                let rows: BlockRows = Arc::new(rows);
                self.block_visual = Arc::new(visual);
                self.block_layout = Some((key, rows.clone()));
                rows
            }
        };
        self.block_rows.lock().unwrap().clear();

        let theme = self.state.theme.clone();
        let cursor = self.source_display_cursor();
        let selection = Arc::new(self.source_display_selection_ranges());
        let marked = self.source_marked_range();
        let view = cx.entity().clone();
        let registry = self.block_rows.clone();
        let row_h = calculate_line_row_height(font_size, line_height);

        let build = move || -> AnyElement {
            let row_count = rows.len();
            let row_elements = rows.iter().enumerate().map(|(index, (v, offset, tokens, positions))| {
                let (offset, len) = (*offset, v.char_len);
                let last_of_line = rows.get(index + 1).is_none_or(|next| next.0.physical_line_idx != v.physical_line_idx);
                let active = cursor.0 == v.physical_line_idx
                    && cursor.1 >= v.char_start
                    && (cursor.1 < v.char_start + len || last_of_line);
                let quads: Vec<AnyElement> = selection.iter().filter_map(|range| {
                    let start = range.start.max(offset);
                    let end = range.end.min(offset + len);
                    if start > end || (start == end && !range.is_empty()) { return None; }
                    let (x0, x1) = (positions[start - offset], positions[end - offset]);
                    Some(div().absolute().top_0().left(px(x0)).w(px((x1 - x0).max(2.))).h(px(row_h))
                        .bg(theme.selection_bg()).into_any_element())
                }).collect();
                let mut token_offset = offset;
                let token_elements: Vec<AnyElement> = tokens.iter().map(|tok| {
                    let length = tok.text.chars().count();
                    let selected: Vec<_> = selection.iter().filter_map(|range| {
                        let start = range.start.max(token_offset);
                        let end = range.end.min(token_offset + length);
                        (start < end).then_some(start - token_offset..end - token_offset)
                    }).collect();
                    token_offset += length;
                    crate::views::editor::render_markdown_token_element(tok, &theme, &selected).into_any_element()
                }).collect();
                let underline = marked.as_ref().and_then(|range| {
                    let start = range.start.max(offset);
                    let end = range.end.min(offset + len);
                    (start < end).then(|| {
                        let (x0, x1) = (positions[start - offset], positions[end - offset]);
                        div().absolute().bottom_0().left(px(x0.min(x1))).w(px((x1 - x0).abs().max(1.))).h(px(1.))
                            .bg(theme.text_primary).into_any_element()
                    })
                });
                let caret = active.then(|| {
                    let x = positions[cursor.1.saturating_sub(v.char_start).min(positions.len() - 1)];
                    div().absolute().top(px(((row_h - font_size * 1.2) * 0.5).max(0.0))).left(px(x))
                        .w(px(2.)).h(px(font_size * 1.2)).bg(theme.accent).into_any_element()
                });
                let press_view = view.clone();
                let geometry = (registry.clone(), v.clone(), tokens.clone(), positions.clone());
                div()
                    .debug_selector(move || format!("block_edit_row_{index}"))
                    .relative()
                    .flex()
                    .items_center()
                    .h(px(row_h))
                    .w_full()
                    .text_size(px(font_size))
                    .line_height(relative(line_height))
                    .font_family(crate::platform::mono_font())
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        cx.stop_propagation();
                        press_view.update(cx, |this, cx| this.source_mouse_down_from(event, true, window, cx));
                    })
                    .children(quads)
                    .children(token_elements)
                    .children(underline)
                    .children(caret)
                    .child(canvas(move |bounds, _, _| {
                        let (registry, visual, tokens, positions) = &geometry;
                        let mut rows = registry.lock().unwrap();
                        rows.retain(|row| row.index != index);
                        rows.push(SourceRow {
                            bounds, index, visual: visual.clone(), offset, tokens: tokens.clone(), positions: positions.clone(),
                        });
                    }, |_, _, _, _| {}).absolute().size_full())
                    .into_any_element()
            }).collect::<Vec<_>>();
            debug_assert_eq!(row_elements.len(), row_count);

            let box_view = view.clone();
            let input_view = view.clone();
            div()
                .w_full()
                .px_8()
                .pb_4()
                .child(
                    div()
                        .debug_selector(|| "block_editor".into())
                        .relative()
                        .w(px(column))
                        .max_w_full()
                        .mx_auto()
                        .px(px(BOX_PAD_X))
                        .py(px(BOX_PAD_Y))
                        .rounded(px(6.))
                        .bg(theme.bg)
                        .border_1()
                        .border_color(theme.border)
                        .cursor_text()
                        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                            cx.stop_propagation();
                            box_view.update(cx, |this, cx| this.source_mouse_down_from(event, true, window, cx));
                        })
                        .children(row_elements)
                        .child(canvas(|bounds, _, _| bounds, move |bounds, _, window, cx| {
                            let focus = input_view.read(cx).focus_handle.clone();
                            if input_view.read(cx).source_accepts_input(window, cx) {
                                window.handle_input(&focus, ElementInputHandler::new(bounds, input_view.clone()), cx);
                            }
                            input_view.update(cx, |this, cx| this.source_after_paint(window, cx));
                        }).absolute().size_full()),
                )
                .into_any_element()
        };
        Some(BlockEditorSlot { blocks: edit.blocks.clone(), build: Arc::new(build) })
    }

    /// Up/Down inside the block editor. At the first or last row the caret leaves
    /// the block, which closes it.
    pub fn block_vertical_move(&mut self, up: bool, extend: bool) {
        let Some(edit) = self.block_edit.clone() else { return };
        let visual: Arc<Vec<VisualLine>> = self.block_visual.clone();
        let (line, col) = self.state.editor.cursor();
        let (index, vcol) = crate::editor_engine::physical_to_visual(line, col, &visual);
        let target = if up && index == 0 {
            edit.start_line.checked_sub(1).map(|l| (l, self.state.editor.line_len(l)))
        } else if !up && index + 1 >= visual.len() {
            edit.end_line(self.state.editor.lines.len())
                .map(|e| e + 1)
                .filter(|l| *l < self.state.editor.lines.len())
                .map(|l| (l, 0))
        } else {
            let next = if up { index - 1 } else { index + 1 };
            Some(crate::editor_engine::visual_to_physical(next, vcol, &visual))
        };
        let Some((line, col)) = target else { return };
        if extend {
            self.state.editor.select_to(line, col, true);
        } else {
            self.state.editor.set_cursor(line, col);
        }
    }
}

#[cfg(test)]
mod tests {
    // Not `super::*`: it would pull in `gpui::test`, which shadows the built-in `#[test]`.
    use super::{anchor_lines, code_block_tokens, source_offset_for_rendered, BlockEdit, BlockShift};

    #[test]
    fn rendered_offsets_skip_markup() {
        assert_eq!(source_offset_for_rendered("# Title", "Title", 0), 2);
        assert_eq!(source_offset_for_rendered("# Title", "Title", 5), 7);
        assert_eq!(source_offset_for_rendered("a **bold** c", "a bold c", 2), 4);
        assert_eq!(source_offset_for_rendered("a **bold** c", "a bold c", 7), 11);
        assert_eq!(source_offset_for_rendered("---", "", 0), 0);
    }

    #[test]
    fn code_block_tokens_mute_fences_and_keep_every_line_intact() {
        let lines: Vec<String> = ["```rust", "fn main() {}", "", "// done", "```"].iter().map(|l| l.to_string()).collect();
        let tokens = code_block_tokens(&lines);
        assert_eq!(tokens.len(), lines.len());
        for (line, line_tokens) in lines.iter().zip(&tokens) {
            assert_eq!(line_tokens.iter().map(|t| t.text.as_str()).collect::<String>(), *line);
        }
        assert_eq!(tokens[0][0].kind, crate::views::editor::MarkdownTokenKind::CodeDelimiter);
        assert_eq!(tokens[4][0].kind, crate::views::editor::MarkdownTokenKind::CodeDelimiter);
        assert!(tokens[1].len() > 1, "a rust line should split into several roles");
        assert!(tokens[2].is_empty());
    }

    #[test]
    fn anchor_lines_are_one_based() {
        let first = zmark_core::ast::parse_markdown("# Title\n\n> quote\n> more\n");
        assert_eq!(anchor_lines(&first.anchors[0]), (0, 0));
        assert_eq!(anchor_lines(&first.anchors[1]), (2, 3));
    }

    #[test]
    fn edit_range_follows_line_count_and_shift_moves_later_blocks() {
        let edit = BlockEdit { start_line: 2, tail: 3, start_offset: 0, blocks: 1..2, code: false };
        assert_eq!(edit.end_line(6), Some(2));
        assert_eq!(edit.end_line(8), Some(4));
        assert_eq!(edit.end_line(5), None);
        let shift = BlockShift { after: 2, delta: 1 };
        assert_eq!((shift.apply(0), shift.apply(1), shift.apply(2), shift.apply(5)), (0, 1, 3, 6));
    }
}
