use gpui::prelude::*;
use gpui::*;
use glidemark_core::ast::{BlockElement, InlineGroup, InlineItem, ParsedDocument};
use glidemark_core::i18n::I18nStrings;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::theme::ThemeColors;

pub type OnCopyCallback = Arc<dyn Fn(String, u64, &mut Window, &mut App) + Send + Sync>;
pub type OnOpenLightboxCallback = Arc<dyn Fn(std::path::PathBuf, u64, &mut Window, &mut App) + Send + Sync>;
pub type OnPreviewSelectionCallback = Arc<dyn Fn(Option<PreviewSelection>, bool, &mut Window, &mut App) + Send + Sync>;
pub type OnPreviewAutoscrollCallback = Arc<dyn Fn(f32, &mut Window, &mut App) + Send + Sync>;

#[derive(Clone, Default)]
pub struct PreviewCallbacks {
    pub on_copy: Option<OnCopyCallback>,
    pub on_open_lightbox: Option<OnOpenLightboxCallback>,
    pub on_selection_change: Option<OnPreviewSelectionCallback>,
    pub on_autoscroll: Option<OnPreviewAutoscrollCallback>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct PreviewCharPosition {
    pub block_ix: usize,
    pub char_offset: usize,
}

impl PreviewCharPosition {
    pub fn new(block_ix: usize, char_offset: usize) -> Self {
        Self { block_ix, char_offset }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PreviewSelection {
    pub anchor: PreviewCharPosition,
    pub head: PreviewCharPosition,
}

pub type PreviewSelectionRange = PreviewSelection;

impl PreviewSelection {
    pub fn new(anchor: PreviewCharPosition, head: PreviewCharPosition) -> Self {
        Self { anchor, head }
    }

    /// Legacy constructor for block-level ranges.
    #[allow(dead_code)]
    pub fn from_blocks(start_block: usize, end_block: usize) -> Self {
        if start_block <= end_block {
            Self {
                anchor: PreviewCharPosition::new(start_block, 0),
                head: PreviewCharPosition::new(end_block, usize::MAX),
            }
        } else {
            Self {
                anchor: PreviewCharPosition::new(start_block, usize::MAX),
                head: PreviewCharPosition::new(end_block, 0),
            }
        }
    }

    pub fn normalized(&self) -> (PreviewCharPosition, PreviewCharPosition) {
        if self.head < self.anchor {
            (self.head, self.anchor)
        } else {
            (self.anchor, self.head)
        }
    }

    pub fn char_range_for_block(&self, block_ix: usize, block_len: usize) -> Option<std::ops::Range<usize>> {
        let (start, end) = self.normalized();
        if block_ix < start.block_ix || block_ix > end.block_ix {
            return None;
        }
        let (s_char, e_char) = if start.block_ix == end.block_ix {
            (start.char_offset.min(block_len), end.char_offset.min(block_len))
        } else if block_ix == start.block_ix {
            (start.char_offset.min(block_len), block_len)
        } else if block_ix == end.block_ix {
            (0, end.char_offset.min(block_len))
        } else {
            (0, block_len)
        };
        if s_char >= e_char {
            None
        } else {
            Some(s_char..e_char)
        }
    }

    #[allow(dead_code)]
    pub fn contains_block(&self, block_ix: usize) -> bool {
        let (start, end) = self.normalized();
        start.block_ix <= block_ix && block_ix <= end.block_ix
    }

    #[allow(dead_code)]
    pub fn contains(&self, block_ix: usize) -> bool {
        self.contains_block(block_ix)
    }
}

pub fn serialize_preview_selection(parsed: &ParsedDocument, range: PreviewSelection) -> String {
    let (start, end) = range.normalized();
    parsed.extract_plain_text_char_range(
        start.block_ix,
        start.char_offset,
        end.block_ix,
        end.char_offset,
    )
}

pub fn char_range_to_byte_range(s: &str, char_range: std::ops::Range<usize>) -> std::ops::Range<usize> {
    if char_range.start >= char_range.end {
        return 0..0;
    }
    let mut byte_start = None;
    let mut byte_end = None;
    let mut current_char = 0;

    for (b_idx, _) in s.char_indices() {
        if current_char == char_range.start {
            byte_start = Some(b_idx);
        }
        if current_char == char_range.end {
            byte_end = Some(b_idx);
            break;
        }
        current_char += 1;
    }

    let start = byte_start.unwrap_or_else(|| if current_char <= char_range.start { s.len() } else { 0 });
    let end = byte_end.unwrap_or(s.len());
    let start = start.min(end);
    start..end
}

pub fn apply_selection_to_highlights(
    text: &str,
    existing_highlights: &[(std::ops::Range<usize>, HighlightStyle)],
    selection_byte_range: std::ops::Range<usize>,
    selection_bg: Hsla,
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    if selection_byte_range.start >= selection_byte_range.end || selection_byte_range.start >= text.len() {
        return existing_highlights.to_vec();
    }

    let sel_start = selection_byte_range.start.min(text.len());
    let sel_end = selection_byte_range.end.min(text.len());

    let mut split_points = Vec::with_capacity(existing_highlights.len() * 2 + 4);
    split_points.push(0);
    split_points.push(text.len());
    split_points.push(sel_start);
    split_points.push(sel_end);

    for (range, _) in existing_highlights {
        split_points.push(range.start.min(text.len()));
        split_points.push(range.end.min(text.len()));
    }

    split_points.sort_unstable();
    split_points.dedup();

    let mut result: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();

    for i in 0..split_points.len().saturating_sub(1) {
        let p_start = split_points[i];
        let p_end = split_points[i + 1];
        if p_start >= p_end {
            continue;
        }

        let base_style = existing_highlights
            .iter()
            .find(|(r, _)| r.start <= p_start && p_end <= r.end)
            .map(|(_, s)| s);

        let is_selected = sel_start <= p_start && p_end <= sel_end;

        let merged_style = match (base_style, is_selected) {
            (None, false) => None,
            (None, true) => Some(HighlightStyle {
                background_color: Some(selection_bg),
                ..Default::default()
            }),
            (Some(s), false) => Some(s.clone()),
            (Some(s), true) => {
                let mut combined = s.clone();
                combined.background_color = Some(selection_bg);
                Some(combined)
            }
        };

        if let Some(style) = merged_style {
            if let Some((prev_range, prev_style)) = result.last_mut() {
                if prev_range.end == p_start && prev_style == &style {
                    prev_range.end = p_end;
                    continue;
                }
            }
            result.push((p_start..p_end, style));
        }
    }

    result
}

pub fn detect_edge_autoscroll(
    mouse_y: f32,
    container_top: f32,
    container_height: f32,
) -> Option<f32> {
    const EDGE_ZONE: f32 = 40.0;
    if mouse_y < container_top + EDGE_ZONE {
        let factor = ((container_top + EDGE_ZONE - mouse_y) / EDGE_ZONE).clamp(0.1, 1.0);
        Some(-factor)
    } else if mouse_y > container_top + container_height - EDGE_ZONE {
        let factor = ((mouse_y - (container_top + container_height - EDGE_ZONE)) / EDGE_ZONE).clamp(0.1, 1.0);
        Some(factor)
    } else {
        None
    }
}

pub fn calculate_single_line_char_offset(
    text_system: &gpui::WindowTextSystem,
    line: &str,
    x_pos: f32,
    font_size: f32,
) -> usize {
    if line.is_empty() || x_pos <= 0.0 {
        return 0;
    }
    let font = gpui::font("ui-sans-serif, system-ui");
    let font_size_px = gpui::px(font_size);
    let run = gpui::TextRun {
        len: line.len(),
        font,
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = text_system.shape_line(
        gpui::SharedString::from(line.to_string()),
        font_size_px,
        &[run],
        None,
    );
    let target_x = gpui::px(x_pos);
    if target_x >= shaped.width {
        return line.chars().count();
    }

    let mut prev_idx = 0;
    let mut prev_x = gpui::px(0.);
    for run in &shaped.runs {
        for glyph in &run.glyphs {
            if glyph.position.x >= target_x {
                let byte_idx = if glyph.position.x - target_x < target_x - prev_x {
                    glyph.index
                } else {
                    prev_idx
                };
                let clamped = byte_idx.min(line.len());
                return line[..clamped].chars().count();
            }
            prev_idx = glyph.index;
            prev_x = glyph.position.x;
        }
    }
    line.chars().count()
}

pub fn calculate_block_click_char_offset(
    text_system: &gpui::WindowTextSystem,
    block_text: &str,
    x_pos: f32,
    y_pos: f32,
    base_font_size: f32,
) -> usize {
    if block_text.is_empty() {
        return 0;
    }
    let total_chars = block_text.chars().count();
    let lines: Vec<&str> = block_text.split('\n').collect();
    if lines.len() > 1 {
        let line_height = base_font_size * 1.6;
        let line_idx = if y_pos <= 0.0 {
            0
        } else {
            ((y_pos / line_height).floor() as usize).min(lines.len() - 1)
        };
        let mut char_count_before = 0;
        for l in &lines[..line_idx] {
            char_count_before += l.chars().count() + 1;
        }
        let line_offset = calculate_single_line_char_offset(text_system, lines[line_idx], x_pos, base_font_size);
        (char_count_before + line_offset).min(total_chars)
    } else {
        calculate_single_line_char_offset(text_system, block_text, x_pos, base_font_size)
    }
}

pub fn render_preview_pane_with_callbacks(
    parsed: Arc<ParsedDocument>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    max_column_width: f32,
    font_size: f32,
    list_state: &ListState,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    callbacks: PreviewCallbacks,
    selection: Option<PreviewSelection>,
    is_selecting: bool,
) -> impl IntoElement {
    let theme = theme.clone();
    let item_theme = theme.clone();
    let item_parsed = parsed.clone();
    let item_callbacks = callbacks.clone();

    let mut pane = div()
        .flex_1()
        .w_full()
        .min_w(px(0.))
        .min_h(px(0.))
        .h_full()
        .flex()
        .flex_col()
        .bg(theme.surface);

    if let Some(ref on_selection_change) = callbacks.on_selection_change {
        let on_sel_up = on_selection_change.clone();
        pane = pane.on_mouse_up(MouseButton::Left, move |_event, window, cx| {
            if is_selecting {
                on_sel_up(selection, false, window, cx);
            }
        });
    }

    pane.child(
        list(list_state.clone(), move |ix, _window, _cx| {
            let Some(block) = item_parsed.blocks.get(ix) else {
                return div().into_any_element();
            };

            let block_text = block.to_plain_text();
            let block_char_len = block_text.chars().count();
            let selected_char_range = selection.and_then(|s| s.char_range_for_block(ix, block_char_len));

            let mut block_container = div()
                .w_full()
                .max_w(px(max_column_width))
                .mx_auto()
                .child(render_block_element(
                    block,
                    &item_theme,
                    strings,
                    font_size,
                    copied_code_hash,
                    item_callbacks.on_copy.as_ref(),
                    item_callbacks.on_open_lightbox.as_ref(),
                    selected_char_range,
                ));

            if let Some(ref on_selection_change) = item_callbacks.on_selection_change {
                let on_sel_down = on_selection_change.clone();
                let on_sel_move = on_selection_change.clone();
                let on_autoscroll = item_callbacks.on_autoscroll.clone();
                let text_for_down = block_text.clone();
                let text_for_move = block_text.clone();

                block_container = block_container
                    .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
                        let offset = calculate_block_click_char_offset(
                            window.text_system(),
                            &text_for_down,
                            f32::from(event.position.x),
                            f32::from(event.position.y),
                            font_size,
                        );
                        let pos = PreviewCharPosition::new(ix, offset);
                        on_sel_down(Some(PreviewSelection::new(pos, pos)), true, window, cx);
                    })
                    .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                        if is_selecting {
                            let offset = calculate_block_click_char_offset(
                                window.text_system(),
                                &text_for_move,
                                f32::from(event.position.x),
                                f32::from(event.position.y),
                                font_size,
                            );
                            let head = PreviewCharPosition::new(ix, offset);
                            if let Some(sel) = selection {
                                on_sel_move(Some(PreviewSelection::new(sel.anchor, head)), true, window, cx);
                            }
                            if let Some(ref auto_cb) = on_autoscroll {
                                let mouse_y = f32::from(event.position.y);
                                if let Some(vel) = detect_edge_autoscroll(mouse_y, 0.0, 800.0) {
                                    auto_cb(vel, window, cx);
                                }
                            }
                        }
                    });
            }

            div()
                .w_full()
                .px_8()
                .pb_4()
                .child(block_container)
                .into_any_element()
        })
        .flex_1()
        .w_full()
        .min_w(px(0.))
        .min_h(px(0.))
        .pt_6()
        .pb_6(),
    )
}

fn render_block_element(
    block: &BlockElement,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    base_font_size: f32,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<&OnCopyCallback>,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
    selected_char_range: Option<std::ops::Range<usize>>,
) -> AnyElement {
    match block {
        BlockElement::Heading { level, content } => {
            let (size_mult, border_bottom) = match level {
                1 => (1.8, true),
                2 => (1.45, true),
                3 => (1.25, false),
                _ => (1.1, false),
            };

            let mut el = div()
                .w_full()
                .min_w(px(0.))
                .text_size(px(base_font_size * size_mult))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_primary)
                .pt_3()
                .pb_1();

            if border_bottom {
                el = el.border_b_1().border_color(theme.border);
            }

            el.child(render_styled_inlines(content, theme, base_font_size * size_mult, selected_char_range))
                .into_any_element()
        }
        BlockElement::Paragraph(content) => div()
            .w_full()
            .min_w(px(0.))
            .text_size(px(base_font_size))
            .line_height(relative(1.75))
            .text_color(theme.text_primary)
            .child(render_styled_inlines(content, theme, base_font_size, selected_char_range))
            .into_any_element(),
        BlockElement::FencedCode { lang, code } => {
            if lang.as_deref() == Some("mermaid") {
                render_mermaid_diagram(code, theme, strings, base_font_size, on_open_lightbox)
            } else {
                render_code_block(
                    lang.as_deref(),
                    code,
                    theme,
                    strings,
                    base_font_size,
                    copied_code_hash,
                    on_copy,
                    selected_char_range,
                )
            }
        }
        BlockElement::BlockQuote(blocks) => div()
            .w_full()
            .min_w(px(0.))
            .border_l_4()
            .border_color(theme.accent)
            .bg(theme.surface)
            .p_3()
            .rounded_r_md()
            .flex()
            .flex_col()
            .gap_2()
            .children(
                blocks
                    .iter()
                    .map(|b| render_block_element(b, theme, strings, base_font_size, copied_code_hash, on_copy, on_open_lightbox, selected_char_range.clone())),
            )
            .into_any_element(),
        BlockElement::List { ordered, items } => render_list_element(
            *ordered,
            items,
            theme,
            strings,
            base_font_size,
            0,
            copied_code_hash,
            on_copy,
            on_open_lightbox,
            selected_char_range,
        ),
        BlockElement::Table { headers, rows } => {
            let mut cur_offset = 0;
            let mut header_cells = Vec::new();
            for h in headers {
                let text = h.plain_text();
                let len = text.chars().count();
                let cell_start = cur_offset;
                let cell_end = cur_offset + len;
                let cell_range = selected_char_range.as_ref().and_then(|range| {
                    if range.start < cell_end && range.end > cell_start {
                        let s = range.start.saturating_sub(cell_start).min(len);
                        let e = range.end.saturating_sub(cell_start).min(len);
                        if s < e { Some(s..e) } else { None }
                    } else {
                        None
                    }
                });
                cur_offset += len + 1;
                header_cells.push(
                    div()
                        .flex_1()
                        .min_w(px(120.))
                        .p_3()
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(base_font_size * 0.95))
                        .text_color(theme.text_primary)
                        .child(render_styled_inlines(h, theme, base_font_size * 0.95, cell_range)),
                );
            }

            let mut row_cells = Vec::new();
            for row in rows {
                let mut cells = Vec::new();
                for cell in row {
                    let text = cell.plain_text();
                    let len = text.chars().count();
                    let cell_start = cur_offset;
                    let cell_end = cur_offset + len;
                    let cell_range = selected_char_range.as_ref().and_then(|range| {
                        if range.start < cell_end && range.end > cell_start {
                            let s = range.start.saturating_sub(cell_start).min(len);
                            let e = range.end.saturating_sub(cell_start).min(len);
                            if s < e { Some(s..e) } else { None }
                        } else {
                            None
                        }
                    });
                    cur_offset += len + 1;
                    cells.push(
                        div()
                            .flex_1()
                            .min_w(px(120.))
                            .p_3()
                            .text_size(px(base_font_size * 0.95))
                            .text_color(theme.text_primary)
                            .child(render_styled_inlines(cell, theme, base_font_size * 0.95, cell_range)),
                    );
                }
                row_cells.push(
                    div()
                        .border_b_1()
                        .border_color(theme.border)
                        .flex()
                        .children(cells),
                );
            }

            div()
                .id("preview_table_scroll")
                .w_full()
                .min_w(px(0.))
                .overflow_x_scroll()
                .my_2()
                .child(
                    div()
                        .w_full()
                        .min_w(px(520.))
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .bg(theme.bg)
                                .border_b_1()
                                .border_color(theme.border)
                                .flex()
                                .children(header_cells),
                        )
                        .children(row_cells),
                )
                .into_any_element()
        }
        BlockElement::Rule => div()
            .w_full()
            .my_4()
            .border_b_1()
            .border_color(theme.border)
            .into_any_element(),
    }
}

fn render_styled_inlines(
    group: &InlineGroup,
    theme: &ThemeColors,
    font_size: f32,
    selected_char_range: Option<std::ops::Range<usize>>,
) -> AnyElement {
    let has_image = group.0.iter().any(|item| matches!(item, InlineItem::Image { .. }));
    if has_image {
        return div()
            .w_full()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .gap_2()
            .children(group.0.iter().map(|item| match item {
                InlineItem::Image { alt, url, .. } => div()
                    .w_full()
                    .min_w(px(0.))
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.bg)
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        svg()
                            .path("icons/image.svg")
                            .size(px(18.))
                            .text_color(theme.text_muted),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child(if alt.is_empty() { url.clone() } else { format!("{} ({})", alt, url) }),
                    )
                    .into_any_element(),
                other => {
                    let single = InlineGroup(vec![other.clone()]);
                    render_styled_text_only(&single, theme, font_size, selected_char_range.clone()).into_any_element()
                }
            }))
            .into_any_element();
    }

    render_styled_text_only(group, theme, font_size, selected_char_range).into_any_element()
}

pub fn render_styled_text_only(
    group: &InlineGroup,
    theme: &ThemeColors,
    _font_size: f32,
    selected_char_range: Option<std::ops::Range<usize>>,
) -> StyledText {
    let mut text = String::new();
    let mut highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();

    for item in &group.0 {
        let start = text.len();
        match item {
            InlineItem::Text(s) => {
                text.push_str(s);
            }
            InlineItem::Bold(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Italic(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Strikethrough(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        color: Some(theme.text_muted),
                        strikethrough: Some(StrikethroughStyle {
                            thickness: px(1.),
                            color: Some(theme.text_muted),
                        }),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Code(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        color: Some(theme.syntax_inline_code),
                        background_color: Some(theme.code_bg),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Link { text: link_text, .. } => {
                text.push_str(link_text);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        color: Some(theme.accent),
                        underline: Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(theme.accent),
                            wavy: false,
                        }),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Image { alt, .. } => {
                text.push_str(alt);
            }
        }
    }

    if let Some(char_range) = selected_char_range {
        let byte_range = char_range_to_byte_range(&text, char_range);
        highlights = apply_selection_to_highlights(&text, &highlights, byte_range, theme.selection_bg());
    }

    if text.is_empty() {
        text.push(' ');
    }

    StyledText::new(text).with_highlights(highlights)
}

pub const MAX_DIAGRAM_CACHE: usize = 64;

pub struct DiagramCache {
    entries: std::collections::HashMap<(u64, bool), std::path::PathBuf>,
    order: std::collections::VecDeque<(u64, bool)>,
    max_size: usize,
}

impl DiagramCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
            max_size,
        }
    }

    pub fn get(&mut self, key: &(u64, bool)) -> Option<std::path::PathBuf> {
        let path = self.entries.get(key)?.clone();
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
            self.order.push_back(*key);
        }
        Some(path)
    }

    pub fn insert(&mut self, key: (u64, bool), path: std::path::PathBuf) {
        if self.entries.insert(key, path).is_none() {
            self.order.push_back(key);
            if self.order.len() > self.max_size {
                if let Some(evicted_key) = self.order.pop_front() {
                    if let Some(evicted_path) = self.entries.remove(&evicted_key) {
                        let _ = std::fs::remove_file(evicted_path);
                    }
                }
            }
        } else if let Some(pos) = self.order.iter().position(|k| k == &key) {
            self.order.remove(pos);
            self.order.push_back(key);
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

static DIAGRAM_CACHE: std::sync::LazyLock<std::sync::Mutex<DiagramCache>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(DiagramCache::new(MAX_DIAGRAM_CACHE)));

fn render_mermaid_container(
    path: std::path::PathBuf,
    code_hash: u64,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
) -> AnyElement {
    let mut container = div()
        .id(("mermaid_diagram_container", code_hash))
        .w_full()
        .min_w(px(0.))
        .my_3()
        .rounded_xl()
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .p_4()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .relative()
        .overflow_x_scroll();

    if let Some(cb) = on_open_lightbox {
        let cb = (*cb).clone();
        let p_click = path.clone();
        let cb_btn = cb.clone();
        let p_btn = path.clone();
        let btn = div()
            .id(("mermaid_zoom_btn", code_hash))
            .absolute()
            .top(px(8.))
            .right(px(8.))
            .w(px(28.))
            .h(px(28.))
            .rounded_md()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.selection_bg())
            .hover(|s| s.bg(theme.hover_bg()))
            .tooltip({
                let t = strings.mermaid_zoom_hint.to_string();
                move |_window, cx| cx.new(|_| CodeTooltipView { text: t.clone() }).into()
            })
            .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                cx.stop_propagation(); // 防冒泡到外层容器
                cb_btn(p_btn.clone(), code_hash, window, cx);
            })
            .child(
                svg()
                    .path("icons/maximize.svg")
                    .size(px(14.))
                    .text_color(theme.selection_text()),
            );

        container = container
            .cursor(CursorStyle::PointingHand)
            .hover(|s| s.border_color(theme.accent))
            .on_mouse_down(MouseButton::Left, move |_event, window, app| {
                cb(p_click.clone(), code_hash, window, app);
            })
            .child(btn);
    }

    container
        .child(
            img(path)
                .max_w_full()
        )
        .into_any_element()
}

fn render_mermaid_diagram(
    code: &str,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    _base_font_size: f32,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
) -> AnyElement {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    let code_hash = hasher.finish();
    let is_dark = theme.is_dark;

    // Check in-memory memoized cache first (zero-cost on scroll)
    let cached_path = {
        let mut cache = DIAGRAM_CACHE.lock().unwrap();
        cache.get(&(code_hash, is_dark))
    };

    if let Some(path) = cached_path {
        if path.exists() {
            return render_mermaid_container(path, code_hash, theme, strings, on_open_lightbox);
        }
    }

    let result = glidemark_core::mermaid::render_to_svg_themed(code, is_dark);
    match result {
        Ok(svg_content) => {
            let cache_dir = std::env::temp_dir()
                .join("glidemark")
                .join("mermaid");
            let _ = std::fs::create_dir_all(&cache_dir);
            let filename = format!("{:016x}_{}.svg", code_hash, if is_dark { "dark" } else { "light" });
            let path = cache_dir.join(filename);
            let _ = std::fs::write(&path, svg_content);

            {
                let mut cache = DIAGRAM_CACHE.lock().unwrap();
                cache.insert((code_hash, is_dark), path.clone());
            }

            render_mermaid_container(path, code_hash, theme, strings, on_open_lightbox)
        }
        Err(err) => {
            div()
                .w_full()
                .min_w(px(0.))
                .my_3()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg)
                .p_3()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.accent)
                        .child("Mermaid Syntax Error"),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family("ui-monospace")
                        .text_color(theme.text_muted)
                        .child(err),
                )
                .into_any_element()
        }
    }
}

struct CodeTooltipView {
    text: String,
}

impl Render for CodeTooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgba(0x18181bf2))
            .border_1()
            .border_color(rgba(0x3f3f4640))
            .text_size(px(11.))
            .text_color(rgb(0xf4f4f5))
            .font_family("ui-sans-serif, system-ui")
            .shadow_md()
            .child(self.text.clone())
    }
}

fn render_list_element(
    ordered: bool,
    items: &[glidemark_core::ast::ListItem],
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    base_font_size: f32,
    depth: usize,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<&OnCopyCallback>,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
    selected_char_range: Option<std::ops::Range<usize>>,
) -> AnyElement {
    let mut cur_offset = 0;
    div()
        .w_full()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .gap_1p5()
        .children(items.iter().enumerate().map(|(idx, item)| {
            let prefix_len = if item.checked.is_some() {
                6
            } else if ordered {
                format!("{}. ", idx + 1).chars().count()
            } else {
                2
            };
            let content_text = item.content.plain_text();
            let content_len = content_text.chars().count();
            let item_start = cur_offset + prefix_len;
            let item_end = item_start + content_len;

            let item_range = selected_char_range.as_ref().and_then(|range| {
                if range.start < item_end && range.end > item_start {
                    let s = range.start.saturating_sub(item_start).min(content_len);
                    let e = range.end.saturating_sub(item_start).min(content_len);
                    if s < e { Some(s..e) } else { None }
                } else {
                    None
                }
            });

            cur_offset += prefix_len + content_len + 1;

            div()
                .w_full()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .w_full()
                        .min_w(px(0.))
                        .flex()
                        .items_start()
                        .gap_2()
                        .child(if let Some(checked) = item.checked {
                            div()
                                .mt_1()
                                .w(px(14.))
                                .h(px(14.))
                                .rounded_sm()
                                .border_1()
                                .border_color(theme.syntax_bullet)
                                .bg(if checked { theme.syntax_bullet } else { theme.bg })
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(if checked {
                                    svg()
                                        .path("icons/check.svg")
                                        .size(px(9.))
                                        .text_color(rgb(0xffffff))
                                        .into_any_element()
                                } else {
                                    div().into_any_element()
                                })
                        } else if ordered {
                            div()
                                .flex_shrink_0()
                                .w(px(20.))
                                .text_right()
                                .text_size(px(base_font_size))
                                .font_family("ui-monospace")
                                .text_color(theme.syntax_bullet)
                                .child(format!("{}.", idx + 1))
                        } else {
                            let bullet = match depth % 3 {
                                0 => "–",
                                1 => "◦",
                                _ => "▪",
                            };
                            div()
                                .flex_shrink_0()
                                .w(px(12.))
                                .text_center()
                                .text_size(px(base_font_size))
                                .text_color(theme.syntax_bullet)
                                .child(bullet)
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(px(base_font_size))
                                .line_height(relative(1.6))
                                .text_color(theme.text_primary)
                                .child(render_styled_inlines(&item.content, theme, base_font_size, item_range)),
                        ),
                )
                .children(item.sub_list.as_ref().map(|sub| {
                    div()
                        .pl_5()
                        .pt_1()
                        .child(if let BlockElement::List { ordered: sub_ordered, items: sub_items } = sub.as_ref() {
                            render_list_element(
                                *sub_ordered,
                                sub_items,
                                theme,
                                strings,
                                base_font_size,
                                depth + 1,
                                copied_code_hash,
                                on_copy,
                                on_open_lightbox,
                                selected_char_range.clone(),
                            )
                        } else {
                            render_block_element(sub, theme, strings, base_font_size, copied_code_hash, on_copy, on_open_lightbox, selected_char_range.clone())
                        })
                }))
        }))
        .into_any_element()
}

/// Determines whether a code block should soft-wrap (`whitespace_normal`)
/// or retain horizontal scroll (`overflow_x_scroll().whitespace_nowrap()`).
///
/// Under ADR 0036 §3 specification:
/// - `Some(l)` trimmed and lowercased: non-empty and `!= "text"` returns `true` (soft-wrap).
/// - `None`, `""`, `"text"` (case and whitespace-insensitive) return `false` (retain horizontal scroll for ASCII trees).
/// - Note: Mermaid diagrams do not enter this function as they are rendered
///   as interactive diagram widgets with pan/zoom lightbox capabilities.
pub fn should_wrap_code_block(lang: Option<&str>) -> bool {
    match lang {
        None => false,
        Some(l) => {
            let s = l.trim().to_lowercase();
            !s.is_empty() && s != "text"
        }
    }
}

fn render_code_block(
    lang: Option<&str>,
    code: &str,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    base_font_size: f32,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<&OnCopyCallback>,
    selected_char_range: Option<std::ops::Range<usize>>,
) -> AnyElement {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    let hash = hasher.finish();
    let is_copied = copied_code_hash
        .map(|(h, t)| h == hash && t.elapsed() < std::time::Duration::from_millis(1200))
        .unwrap_or(false);

    let on_copy_cb = on_copy.cloned();
    let code_to_copy = code.to_string();
    let code_text = if code.is_empty() { " ".to_string() } else { code.to_string() };

    let should_wrap = should_wrap_code_block(lang);

    let group_name: SharedString = format!("code_block_{}", hash).into();

    let mut highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
    if let Some(char_range) = selected_char_range {
        let byte_range = char_range_to_byte_range(&code_text, char_range);
        if byte_range.start < byte_range.end {
            highlights.push((
                byte_range,
                HighlightStyle {
                    background_color: Some(theme.selection_bg()),
                    ..Default::default()
                },
            ));
        }
    }

    let styled_text = if highlights.is_empty() {
        StyledText::new(code_text)
    } else {
        StyledText::new(code_text).with_highlights(highlights)
    };

    let mut content = div()
        .id(SharedString::from(format!("code_content_scroll_{}", hash)))
        .w_full()
        .min_w(px(0.))
        .p_3()
        .text_size(px(base_font_size * 0.85))
        .font_family("ui-monospace")
        .line_height(relative(1.5))
        .text_color(theme.syntax_code)
        .child(styled_text);

    if should_wrap {
        content = content.whitespace_normal();
    } else {
        content = content.overflow_x_scroll().whitespace_nowrap();
    }

    let copy_btn = div()
        .id(SharedString::from(format!("copy_code_btn_{}", hash)))
        .absolute()
        .top(px(8.))
        .right(px(8.))
        .w(px(26.))
        .h(px(26.))
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .opacity(if is_copied { 1.0 } else { 0.0 })
        .group_hover(group_name.clone(), |s| s.opacity(1.0))
        .bg(theme.selection_bg())
        .hover(|s| s.bg(theme.hover_bg()))
        .tooltip({
            let is_copied = is_copied;
            move |_window, cx| {
                cx.new(|_| CodeTooltipView {
                    text: if is_copied {
                        strings.copied.to_string()
                    } else {
                        strings.copy_code.to_string()
                    },
                })
                .into()
            }
        })
        .on_click(move |_, window, cx| {
            if let Some(ref cb) = on_copy_cb {
                cb(code_to_copy.clone(), hash, window, cx);
            }
        })
        .child(
            svg()
                .path(if is_copied {
                    "icons/check.svg"
                } else {
                    "icons/copy.svg"
                })
                .size(px(13.))
                .text_color(if is_copied {
                    theme.accent
                } else {
                    theme.selection_text()
                }),
        );

    div()
        .id(SharedString::from(format!("code_block_{}", hash)))
        .group(group_name)
        .relative()
        .w_full()
        .min_w(px(0.))
        .rounded_md()
        .bg(theme.code_bg)
        .child(content)
        .child(copy_btn)
        .into_any_element()
}
