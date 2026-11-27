use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use crate::editor_engine::VisualLine;
use crate::theme::ThemeColors;

pub fn calculate_line_row_height(font_size: f32, line_height: f32) -> f32 {
    let lh = zmark_core::config::Preferences::sanitize_line_height(line_height);
    font_size * lh
}

mod tokenizer;
pub use tokenizer::{spans_to_code_tokens, tokenize_inline, tokenize_markdown_line, MarkdownToken, MarkdownTokenKind};

fn render_markdown_token_element(tok: &MarkdownToken, theme: &ThemeColors, selection: &[std::ops::Range<usize>]) -> impl IntoElement {
    let highlights: Vec<_> = selection.iter().map(|range| (
        char_index_to_byte_offset(&tok.text, range.start)..char_index_to_byte_offset(&tok.text, range.end),
        HighlightStyle { color: Some(theme.text_selection_foreground()), background_color: Some(theme.selection_bg()), ..Default::default() },
    )).collect();
    let mut el = div().child(StyledText::new(tok.text.clone()).with_highlights(highlights));
    match tok.kind {
        MarkdownTokenKind::Keyword => {
            el = el.text_color(theme.syntax_keyword);
        }
        MarkdownTokenKind::Function => {
            el = el.text_color(theme.syntax_function);
        }
        MarkdownTokenKind::Attribute => {
            el = el.text_color(theme.syntax_attribute);
        }
        MarkdownTokenKind::StringLiteral => {
            el = el.text_color(theme.syntax_string);
        }
        MarkdownTokenKind::Number => {
            el = el.text_color(theme.syntax_number);
        }
        MarkdownTokenKind::Comment => {
            el = el.text_color(theme.syntax_comment);
        }
        MarkdownTokenKind::HeadingPrefix => {
            // The marker is structure, not emphasis: keep it quiet so the heading
            // text itself carries the weight.
            el = el.text_color(theme.syntax_heading).font_weight(FontWeight::NORMAL);
        }
        MarkdownTokenKind::HeadingText => {
            el = el.text_color(theme.syntax_heading).font_weight(FontWeight::BOLD);
        }
        MarkdownTokenKind::BlockquotePrefix | MarkdownTokenKind::BlockquoteText => {
            el = el.text_color(theme.text_muted);
        }
        MarkdownTokenKind::BulletPrefix => {
            el = el.text_color(theme.syntax_bullet);
        }
        MarkdownTokenKind::InlineCode => {
            el = el.text_color(theme.syntax_emphasis)
                .bg(theme.code_bg)
                .rounded(px(3.))
                .px(px(4.));
        }
        MarkdownTokenKind::InlineCodeSplitLeft => {
            el = el.text_color(theme.syntax_emphasis)
                .bg(theme.code_bg)
                .rounded_l(px(3.))
                .pl(px(4.))
                .pr_0();
        }
        MarkdownTokenKind::InlineCodeSplitRight => {
            el = el.text_color(theme.syntax_emphasis)
                .bg(theme.code_bg)
                .rounded_r(px(3.))
                .pr(px(4.))
                .pl_0();
        }
        MarkdownTokenKind::Bold => {
            el = el.text_color(theme.syntax_emphasis).font_weight(FontWeight::BOLD);
        }
        MarkdownTokenKind::LinkText | MarkdownTokenKind::LinkUrl => {
            el = el.text_color(theme.syntax_link).underline();
        }
        MarkdownTokenKind::CodeDelimiter => {
            el = el.text_color(theme.text_muted);
        }
        MarkdownTokenKind::CodeText => {
            el = el.text_color(theme.syntax_code);
        }
        MarkdownTokenKind::Plain => {
            el = el.text_color(theme.text_primary);
        }
    }
    el
}

pub fn physical_line_tokens(
    index: usize,
    text: &str,
    blocks: &[zmark_core::ast::ParsedCodeBlock],
    ast_in_sync: bool,
    spans: &std::collections::HashMap<zmark_core::syntax::CodeKey, Arc<[zmark_core::syntax::SyntaxSpan]>>,
) -> Vec<MarkdownToken> {
    let Some(block) = blocks.iter().find(|block| index >= block.fence_open_line && index <= block.fence_close_line.unwrap_or_else(|| block.lines.last().map_or(block.fence_open_line, |line| line.source_line_idx))) else {
        return tokenize_markdown_line(text);
    };
    let plain = |kind| vec![MarkdownToken { text: text.to_owned(), kind }];
    if !ast_in_sync { return plain(MarkdownTokenKind::CodeText); }
    if block.child_path != "source-file" && (index == block.fence_open_line || block.fence_close_line == Some(index)) { return plain(MarkdownTokenKind::CodeDelimiter); }
    let Some(mapping) = block.lines.iter().find(|line| line.source_line_idx == index).filter(|_| block.is_valid) else { return plain(MarkdownTokenKind::CodeText); };
    let Some(spans) = spans.get(&block.key) else { return plain(MarkdownTokenKind::CodeText); };
    let line_spans: Vec<_> = spans.iter().filter_map(|span| {
        let start = span.range.start.max(mapping.payload_range.start);
        let end = span.range.end.min(mapping.payload_range.end);
        (start < end).then_some(zmark_core::syntax::SyntaxSpan { range: start.saturating_sub(mapping.payload_range.start)..end.saturating_sub(mapping.payload_range.start), role: span.role })
    }).collect();
    let Some((prefix, code)) = text.get(..mapping.prefix_len).zip(text.get(mapping.prefix_len..)) else { return plain(MarkdownTokenKind::CodeText); };
    let mut tokens = Vec::new();
    if !prefix.is_empty() { tokens.push(MarkdownToken { text: prefix.to_owned(), kind: MarkdownTokenKind::Plain }); }
    tokens.extend(spans_to_code_tokens(code, &line_spans));
    tokens
}

pub const EDITOR_CONTAINER_PADDING_X: f32 = 28.0; // 32px total with the row inset
pub const EDITOR_ROW_PADDING_X: f32 = 4.0;        // row px_1
pub const EDITOR_GUTTER_MARGIN_RIGHT: f32 = 8.0;

pub fn editor_gutter_width(text_system: &gpui::WindowTextSystem, line_count: usize, font_size: f32) -> f32 {
    let digits = line_count.max(1).to_string().len();
    let text = "8".repeat(digits);
    let run = gpui::TextRun { len: text.len(), font: gpui::font(crate::platform::mono_font()), color: gpui::black(), background_color: None, underline: None, strikethrough: None };
    f32::from(text_system.shape_line(text.into(), gpui::px(font_size * 0.85), &[run], None).width).ceil() + 2.0
}

/// Whether a visual line should render its gutter number.
pub fn gutter_visible(is_first_visual_line: bool, show_line_numbers: bool) -> bool {
    show_line_numbers && is_first_visual_line
}

pub fn editor_text_start_offset(gutter_width: f32) -> f32 {
    EDITOR_CONTAINER_PADDING_X + EDITOR_ROW_PADDING_X + gutter_width + EDITOR_GUTTER_MARGIN_RIGHT
}

pub fn char_index_to_byte_offset(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

pub fn byte_offset_to_char_index(s: &str, byte_offset: usize) -> usize {
    let clamped = byte_offset.min(s.len());
    s[..clamped].chars().count()
}

pub fn token_paddings(kind: &MarkdownTokenKind) -> (f32, f32) {
    match kind {
        MarkdownTokenKind::InlineCode => (4.0, 4.0),
        MarkdownTokenKind::InlineCodeSplitLeft => (4.0, 0.0),
        MarkdownTokenKind::InlineCodeSplitRight => (0.0, 4.0),
        _ => (0.0, 0.0),
    }
}

/// Character boundary positions use the same font weights and pill insets as painting.
pub fn token_char_positions(text_system: &gpui::WindowTextSystem, tokens: &[MarkdownToken], font_size: f32) -> Vec<f32> {
    let count = tokens.iter().map(|token| token.text.chars().count()).sum::<usize>();
    let mut positions = vec![0.0; count + 1];
    let mut index = 0;
    let mut x = 0.0;
    for token in tokens {
        let mut font = gpui::font(crate::platform::mono_font());
        if matches!(token.kind, MarkdownTokenKind::Bold | MarkdownTokenKind::HeadingText) { font.weight = FontWeight::BOLD; }
        let run = TextRun { len: token.text.len(), font, color: gpui::black(), background_color: None, underline: None, strikethrough: None };
        let shaped = text_system.shape_line(token.text.clone().into(), px(font_size), &[run], None);
        let (left, right) = token_paddings(&token.kind);
        for (byte, _) in token.text.char_indices() {
            positions[index] = x + left + f32::from(shaped.x_for_index(byte));
            index += 1;
        }
        x += left + f32::from(shaped.width) + right;
        positions[index] = x;
    }
    positions[0] = 0.0;
    positions
}

pub fn closest_char_position(positions: &[f32], x: f32) -> usize {
    positions.iter().enumerate().min_by(|(_, a), (_, b)| (**a - x).abs().total_cmp(&(**b - x).abs())).map_or(0, |(index, _)| index)
}

pub fn calculate_cursor_x_offset_tokens(text_system: &gpui::WindowTextSystem, tokens: &[MarkdownToken], char_col: usize, font_size: f32) -> f32 {
    let positions = token_char_positions(text_system, tokens, font_size);
    positions[char_col.min(positions.len() - 1)]
}

pub fn calculate_click_char_col_tokens(text_system: &gpui::WindowTextSystem, tokens: &[MarkdownToken], x_offset: f32, font_size: f32) -> usize {
    closest_char_position(&token_char_positions(text_system, tokens, font_size), x_offset)
}

pub fn compute_char_pixel_widths(text_system: &gpui::WindowTextSystem, tokens: &[MarkdownToken], font_size: f32) -> Vec<f32> {
    token_char_positions(text_system, tokens, font_size).windows(2).map(|pair| (pair[1] - pair[0]).max(0.0)).collect()
}

/// Wraps a physical line into visual lines using actual layout metrics (font advance, Tab width,
/// and token paddings) against `max_width_px`, also returning the natural unwrapped width in pixels.
pub fn wrap_line_to_visual_layout_with_width(
    text_system: &gpui::WindowTextSystem,
    physical_line_idx: usize,
    line_str: &str,
    tokens: &[MarkdownToken],
    max_width_px: f32,
    font_size: f32,
) -> (Vec<VisualLine>, f32) {
    let char_widths = compute_char_pixel_widths(text_system, tokens, font_size);
    let natural_width_px: f32 = char_widths.iter().copied().sum();
    let visual_lines = crate::editor_engine::wrap_line_to_visual_measured(
        physical_line_idx,
        line_str,
        max_width_px,
        |idx, ch| {
            char_widths.get(idx).copied().unwrap_or_else(|| {
                (crate::editor_engine::char_unit_width(ch) as f32) * (font_size * 0.6).max(1.0)
            })
        },
    );
    (visual_lines, natural_width_px)
}

/// Wraps a physical line into visual lines using actual layout metrics (font advance, Tab width,
/// and token paddings) against `max_width_px`.
#[allow(dead_code)]
pub fn wrap_line_to_visual_layout(
    text_system: &gpui::WindowTextSystem,
    physical_line_idx: usize,
    line_str: &str,
    tokens: &[MarkdownToken],
    max_width_px: f32,
    font_size: f32,
) -> Vec<VisualLine> {
    wrap_line_to_visual_layout_with_width(
        text_system,
        physical_line_idx,
        line_str,
        tokens,
        max_width_px,
        font_size,
    )
    .0
}



#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineVisualSpan {
    pub char_start: usize,
    pub char_len: usize,
    pub is_first: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedLineEntry {
    /// Fingerprint of the line text. The cache only needs to know whether the
    /// line changed; keeping a second full copy of every line doubled editor
    /// memory for no benefit.
    pub text_hash: u64,
    pub text_len: usize,
    pub is_fenced: bool,
    pub spans: Vec<LineVisualSpan>,
    pub natural_width_px: f32,
}

fn line_fingerprint(line: &str) -> (u64, usize) {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    line.hash(&mut hasher);
    (hasher.finish(), line.len())
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VisualLayoutMetrics {
    pub shaped_lines_count: usize,
    pub reused_lines_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct DocumentLayoutCache {
    pub cached_width_bits: u32,
    pub cached_font_bits: u32,
    pub entries: Vec<CachedLineEntry>,
    pub line_offsets: Arc<Vec<usize>>,
    pub metrics: VisualLayoutMetrics,
}

const MAX_CORE_TEXT_SHAPE_BYTES: usize = 2048;
const MAX_SYNC_SHAPE_LINES: usize = 250;

#[derive(Clone, Copy)]
struct MonoMetrics {
    ascii_width: f32,
    cjk_width: f32,
}

impl MonoMetrics {
    fn measure(text_system: &gpui::WindowTextSystem, font_size: f32) -> Self {
        let run = |len| TextRun {
            len,
            font: gpui::font(crate::platform::mono_font()),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let ascii_sample = "MMMMMMMMMM";
        let cjk_sample = "中中中中中";
        let ascii_width = (f32::from(
            text_system
                .shape_line(ascii_sample.into(), px(font_size), &[run(ascii_sample.len())], None)
                .width,
        ) / 10.0)
            .max((font_size * 0.5).max(1.0));
        let cjk_width = (f32::from(
            text_system
                .shape_line(cjk_sample.into(), px(font_size), &[run(cjk_sample.len())], None)
                .width,
        ) / 5.0)
            .max(ascii_width);
        Self { ascii_width, cjk_width }
    }

    fn char_width(self, ch: char) -> f32 {
        if crate::editor_engine::char_unit_width(ch) >= 2 {
            self.cjk_width
        } else {
            self.ascii_width
        }
    }
}

fn wrap_line_fast_monospace(
    physical_line_idx: usize,
    line: &str,
    is_fenced: bool,
    max_width_px: f32,
    metrics: MonoMetrics,
) -> CachedLineEntry {
    let natural_width_px: f32 = line.chars().map(|ch| metrics.char_width(ch)).sum();
    let visual_lines = crate::editor_engine::wrap_line_to_visual_measured(
        physical_line_idx,
        line,
        max_width_px,
        |_, ch| metrics.char_width(ch),
    );
    let spans = visual_lines
        .into_iter()
        .map(|v| LineVisualSpan {
            char_start: v.char_start,
            char_len: v.char_len,
            is_first: v.is_first,
        })
        .collect();
    let (text_hash, text_len) = line_fingerprint(line);
    CachedLineEntry {
        text_hash,
        text_len,
        is_fenced,
        spans,
        natural_width_px,
    }
}

fn shape_single_line_with_budget(
    text_system: &gpui::WindowTextSystem,
    i: usize,
    line: &str,
    is_fenced: bool,
    total_lines: usize,
    max_width_px: f32,
    font_size: f32,
    mono_metrics: &mut Option<MonoMetrics>,
) -> CachedLineEntry {
    let use_fast_path = line.len() > MAX_CORE_TEXT_SHAPE_BYTES
        || (total_lines > MAX_SYNC_SHAPE_LINES && (is_fenced || i >= MAX_SYNC_SHAPE_LINES));
    if use_fast_path {
        let metrics = *mono_metrics.get_or_insert_with(|| MonoMetrics::measure(text_system, font_size));
        return wrap_line_fast_monospace(i, line, is_fenced, max_width_px, metrics);
    }
    shape_single_line(text_system, i, line, is_fenced, max_width_px, font_size)
}

fn shape_single_line(
    text_system: &gpui::WindowTextSystem,
    i: usize,
    line: &str,
    is_fenced: bool,
    max_width_px: f32,
    font_size: f32,
) -> CachedLineEntry {
    let tokens = if is_fenced {
        vec![MarkdownToken {
            text: line.to_string(),
            kind: MarkdownTokenKind::CodeText,
        }]
    } else {
        tokenize_markdown_line(line)
    };
    let (vl, natural_width_px) = wrap_line_to_visual_layout_with_width(
        text_system,
        i,
        line,
        &tokens,
        max_width_px,
        font_size,
    );
    let spans = vl
        .into_iter()
        .map(|v| LineVisualSpan {
            char_start: v.char_start,
            char_len: v.char_len,
            is_first: v.is_first,
        })
        .collect();
    let (text_hash, text_len) = line_fingerprint(line);
    CachedLineEntry {
        text_hash,
        text_len,
        is_fenced,
        spans,
        natural_width_px,
    }
}

fn evaluate_matching_line(
    old: CachedLineEntry,
    i: usize,
    line: &str,
    is_fenced: bool,
    total_lines: usize,
    width_changed: bool,
    max_width_px: f32,
    font_size: f32,
    text_system: &gpui::WindowTextSystem,
    mono_metrics: &mut Option<MonoMetrics>,
) -> (CachedLineEntry, bool) {
    if !width_changed {
        (old, true)
    } else if old.natural_width_px <= max_width_px {
        (
            CachedLineEntry {
                text_hash: old.text_hash,
                text_len: old.text_len,
                is_fenced,
                spans: vec![LineVisualSpan {
                    char_start: 0,
                    char_len: line.chars().count(),
                    is_first: true,
                }],
                natural_width_px: old.natural_width_px,
            },
            true,
        )
    } else {
        (
            shape_single_line_with_budget(
                text_system,
                i,
                line,
                is_fenced,
                total_lines,
                max_width_px,
                font_size,
                mono_metrics,
            ),
            false,
        )
    }
}

impl DocumentLayoutCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(
        &mut self,
        text_system: &gpui::WindowTextSystem,
        lines: &[String],
        fenced_lines: &[bool],
        max_width_px: f32,
        font_size: f32,
    ) -> Vec<VisualLine> {
        let width_bits = max_width_px.round().to_bits();
        let font_bits = crate::platform::font_layout_key(font_size);

        let font_changed = self.cached_font_bits != font_bits;
        let width_changed = self.cached_width_bits != width_bits;

        self.cached_width_bits = width_bits;
        self.cached_font_bits = font_bits;

        if font_changed {
            self.entries.clear();
        }

        let mut shaped_count = 0;
        let mut reused_count = 0;
        let total_lines = lines.len();
        let mut mono_metrics: Option<MonoMetrics> = None;

        let old_entries = std::mem::take(&mut self.entries);
        let mut new_entries = Vec::with_capacity(lines.len());

        if !font_changed && old_entries.len() == lines.len() {
            for (i, ((line, &is_fenced), old)) in lines
                .iter()
                .zip(fenced_lines.iter())
                .zip(old_entries.into_iter())
                .enumerate()
            {
                let (text_hash, text_len) = line_fingerprint(line);
                if old.is_fenced == is_fenced
                    && old.text_hash == text_hash
                    && old.text_len == text_len
                {
                    let (entry, is_reused) = evaluate_matching_line(
                        old,
                        i,
                        line,
                        is_fenced,
                        total_lines,
                        width_changed,
                        max_width_px,
                        font_size,
                        text_system,
                        &mut mono_metrics,
                    );
                    new_entries.push(entry);
                    if is_reused {
                        reused_count += 1;
                    } else {
                        shaped_count += 1;
                    }
                } else {
                    new_entries.push(shape_single_line_with_budget(
                        text_system,
                        i,
                        line,
                        is_fenced,
                        total_lines,
                        max_width_px,
                        font_size,
                        &mut mono_metrics,
                    ));
                    shaped_count += 1;
                }
            }
        } else if !font_changed && !old_entries.is_empty() {
            let matches = |entry: &CachedLineEntry, line: &str, is_fenced: bool| {
                let (text_hash, text_len) = line_fingerprint(line);
                entry.is_fenced == is_fenced
                    && entry.text_hash == text_hash
                    && entry.text_len == text_len
            };

            let mut prefix = 0;
            while prefix < lines.len()
                && prefix < old_entries.len()
                && matches(&old_entries[prefix], &lines[prefix], fenced_lines[prefix])
            {
                prefix += 1;
            }

            let mut suffix = 0;
            while suffix < (lines.len() - prefix)
                && suffix < (old_entries.len() - prefix)
                && matches(
                    &old_entries[old_entries.len() - 1 - suffix],
                    &lines[lines.len() - 1 - suffix],
                    fenced_lines[lines.len() - 1 - suffix],
                )
            {
                suffix += 1;
            }

            let old_len = old_entries.len();
            let mut old_iter = old_entries.into_iter();
            for (i, old) in old_iter.by_ref().take(prefix).enumerate() {
                let (entry, is_reused) = evaluate_matching_line(
                    old,
                    i,
                    &lines[i],
                    fenced_lines[i],
                    total_lines,
                    width_changed,
                    max_width_px,
                    font_size,
                    text_system,
                    &mut mono_metrics,
                );
                new_entries.push(entry);
                if is_reused {
                    reused_count += 1;
                } else {
                    shaped_count += 1;
                }
            }

            let middle_end = lines.len() - suffix;
            for i in prefix..middle_end {
                new_entries.push(shape_single_line_with_budget(
                    text_system,
                    i,
                    &lines[i],
                    fenced_lines[i],
                    total_lines,
                    max_width_px,
                    font_size,
                    &mut mono_metrics,
                ));
                shaped_count += 1;
            }

            for (offset, old) in old_iter.skip(old_len - prefix - suffix).enumerate() {
                let i = middle_end + offset;
                let (entry, is_reused) = evaluate_matching_line(
                    old,
                    i,
                    &lines[i],
                    fenced_lines[i],
                    total_lines,
                    width_changed,
                    max_width_px,
                    font_size,
                    text_system,
                    &mut mono_metrics,
                );
                new_entries.push(entry);
                if is_reused {
                    reused_count += 1;
                } else {
                    shaped_count += 1;
                }
            }
        } else {
            for (i, (line, &is_fenced)) in lines
                .iter()
                .zip(fenced_lines.iter())
                .enumerate()
            {
                new_entries.push(shape_single_line_with_budget(
                    text_system,
                    i,
                    line,
                    is_fenced,
                    total_lines,
                    max_width_px,
                    font_size,
                    &mut mono_metrics,
                ));
                shaped_count += 1;
            }
        }

        self.metrics = VisualLayoutMetrics {
            shaped_lines_count: shaped_count,
            reused_lines_count: reused_count,
        };
        self.entries = new_entries;

        let mut visual_lines = Vec::new();
        let mut offset = 0;
        self.line_offsets = Arc::new(self.entries.iter().map(|entry| {
            let start = offset;
            offset += entry.spans.last().map_or(0, |span| span.char_start + span.char_len) + 1;
            start
        }).collect());
        for (phys_idx, entry) in self.entries.iter().enumerate() {
            for (subline_idx, span) in entry.spans.iter().enumerate() {
                visual_lines.push(VisualLine {
                    physical_line_idx: phys_idx,
                    subline_idx,
                    char_start: span.char_start,
                    char_len: span.char_len,
                    is_first: span.is_first,
                });
            }
        }
        if visual_lines.is_empty() {
            visual_lines.push(VisualLine {
                physical_line_idx: 0,
                subline_idx: 0,
                char_start: 0,
                char_len: 0,
                is_first: true,
            });
        }
        visual_lines
    }
}

/// Computes the horizontal pixel offset where the cursor should be rendered for `char_col`
/// within `subline_str` using GPUI's text system and actual font metrics.
#[allow(dead_code)]
pub fn calculate_cursor_x_offset_layout(
    text_system: &gpui::WindowTextSystem,
    subline_str: &str,
    char_col: usize,
    font_size: f32,
) -> f32 {
    let tokens = [MarkdownToken {
        text: subline_str.to_string(),
        kind: MarkdownTokenKind::Plain,
    }];
    calculate_cursor_x_offset_tokens(text_system, &tokens, char_col, font_size)
}

pub fn slice_tokens_for_visual_subline(
    tokens: &[MarkdownToken],
    sub_start: usize,
    sub_len: usize,
) -> Vec<MarkdownToken> {
    if tokens.is_empty() || sub_len == 0 {
        return Vec::new();
    }
    let sub_end = sub_start + sub_len;
    let mut sliced = Vec::new();
    let mut tok_start = 0;

    for tok in tokens {
        let tok_char_count = tok.text.chars().count();
        let tok_end = tok_start + tok_char_count;

        if tok_end > sub_start && tok_start < sub_end {
            let c_start = sub_start.saturating_sub(tok_start);
            let c_end = (sub_end.saturating_sub(tok_start)).min(tok_char_count);
            if c_end > c_start {
                let sub_text: String = tok.text.chars().skip(c_start).take(c_end - c_start).collect();
                let kind = match tok.kind {
                    MarkdownTokenKind::InlineCode => {
                        let is_start = c_start == 0;
                        let is_end = c_end == tok_char_count;
                        if is_start && is_end {
                            MarkdownTokenKind::InlineCode
                        } else if is_start {
                            MarkdownTokenKind::InlineCodeSplitLeft
                        } else if is_end {
                            MarkdownTokenKind::InlineCodeSplitRight
                        } else {
                            MarkdownTokenKind::InlineCode
                        }
                    }
                    _ => tok.kind.clone(),
                };
                sliced.push(MarkdownToken {
                    text: sub_text,
                    kind,
                });
            }
        }
        tok_start = tok_end;
    }
    sliced
}

pub fn render_editor_pane(
    lines: Arc<Vec<String>>,
    char_count: usize,
    visual_lines: Arc<Vec<VisualLine>>,
    line_offsets: Arc<Vec<usize>>,
    syntax_cache: Arc<std::sync::Mutex<zmark_core::syntax::SyntaxCache>>,
    code_blocks: Arc<Vec<zmark_core::ast::ParsedCodeBlock>>,
    is_ast_in_sync: bool,
    cursor: (usize, usize),
    theme: &ThemeColors,
    strings: &'static zmark_core::i18n::I18nStrings,
    font_size: f32,
    line_height: f32,
    show_line_numbers: bool,
    scroll_handle: &UniformListScrollHandle,
    gutter_width: f32,
    on_click_line: Arc<dyn Fn(usize, usize, &mut Window, &mut App)>,
    source_view: Entity<crate::views::workspace::WorkspaceView>,
    selection_ranges: Arc<Vec<std::ops::Range<usize>>>,
    marked_range: Option<std::ops::Range<usize>>,
    failures: &[crate::syntax::VisibleSyntaxFailure],
) -> impl IntoElement {
    let line_count = visual_lines.len().max(1);
    let (cursor_line, cursor_col) = cursor;
    let theme = theme.clone();
    let list_theme = theme.clone();
    let sanitized_lh = zmark_core::config::Preferences::sanitize_line_height(line_height);

    let list_visual_lines = visual_lines.clone();
    let list_lines = lines.clone();
    let list_source = source_view.clone();

    div()
        .flex_1()
        .min_h(px(0.))
        .h_full()
        .relative()
        .flex()
        .flex_col()
        .bg(theme.surface)
        .cursor_text()
        // Rows stop propagation, so this only receives presses outside any row: the side
        // padding, the gaps and the space under the last line. They select the nearest text.
        .on_mouse_down(MouseButton::Left, {
            let source = list_source.clone();
            move |event, window, cx| {
                source.update(cx, |this, cx| this.source_mouse_down(event, window, cx));
            }
        })
        .child(
            uniform_list("editor_lines", line_count, move |range, window, _cx| {
                let plain_text = list_source.read(_cx).state.doc().path.as_ref()
                    .is_some_and(|p| !zmark_core::file_ops::is_markdown(p));
                let mut block_spans: std::collections::HashMap<zmark_core::syntax::CodeKey, Arc<[zmark_core::syntax::SyntaxSpan]>> = std::collections::HashMap::new();
                if is_ast_in_sync && !code_blocks.is_empty() {
                    let min_idx = range.start.min(list_visual_lines.len().saturating_sub(1));
                    let max_idx = range.end.min(list_visual_lines.len());
                    let start_p = list_visual_lines.get(min_idx).map(|v| v.physical_line_idx).unwrap_or(0);
                    let end_p = list_visual_lines.get(max_idx.saturating_sub(1)).map(|v| v.physical_line_idx).unwrap_or(usize::MAX);

                    let mut cache = syntax_cache.lock().unwrap();
                    for b in code_blocks.iter() {
                        let b_start = b.fence_open_line;
                        let b_end = b.fence_close_line.unwrap_or_else(|| {
                            b.lines.last().map(|l| l.source_line_idx).unwrap_or(b_start)
                        });
                        if b_start <= end_p && b_end >= start_p {
                            if let Some(spans) = cache.get(&b.key) {
                                block_spans.insert(b.key.clone(), spans);
                            }
                        }
                    }
                }

                let mut cached_phys_tokens: Option<(usize, Vec<MarkdownToken>)> = None;
                range
                    .map(|vis_idx| {
                        let vl = list_visual_lines.get(vis_idx);
                        let phys_idx = vl.map(|v| v.physical_line_idx).unwrap_or(0);
                        let phys_line_str = list_lines.get(phys_idx).map(String::as_str).unwrap_or("");
                        let is_last_subline_of_phys_line = list_visual_lines
                            .get(vis_idx + 1)
                            .map(|next| next.physical_line_idx != phys_idx)
                            .unwrap_or(true);

                        let is_active_subline = cursor_line == phys_idx
                            && cursor_col >= vl.map(|v| v.char_start).unwrap_or(0)
                            && (cursor_col < vl.map(|v| v.char_start + v.char_len).unwrap_or(0)
                                || is_last_subline_of_phys_line);
                        let rel_cursor_col = cursor_col.saturating_sub(vl.map(|v| v.char_start).unwrap_or(0));

                        let show_line_num = gutter_visible(
                            vl.map(|v| v.is_first).unwrap_or(true),
                            show_line_numbers,
                        );
                        let gutter_text = if show_line_num {
                            format!("{}", phys_idx + 1)
                        } else {
                            String::new()
                        };

                        let phys_tokens = match &cached_phys_tokens {
                            Some((cached_idx, tokens)) if *cached_idx == phys_idx => tokens,
                            _ => {
                                let tokens = if plain_text && (!is_ast_in_sync || code_blocks.is_empty()) {
                                    vec![MarkdownToken { text: phys_line_str.to_owned(), kind: MarkdownTokenKind::Plain }]
                                } else {
                                    physical_line_tokens(phys_idx, phys_line_str, &code_blocks, is_ast_in_sync, &block_spans)
                                };
                                cached_phys_tokens = Some((phys_idx, tokens));
                                &cached_phys_tokens.as_ref().unwrap().1
                            }
                        };
                        let (sub_start, sub_len) = if let Some(v) = vl {
                            (v.char_start, v.char_len)
                        } else {
                            (0, phys_line_str.chars().count())
                        };
                        let tokens = slice_tokens_for_visual_subline(phys_tokens, sub_start, sub_len);

                        let row_h = calculate_line_row_height(font_size, sanitized_lh);
                        let click_left = list_source.clone();
                        let click_middle = list_source.clone();
                        let click_right = list_source.clone();

                        let row = div()
                            .debug_selector(move || format!("source_row_{vis_idx}"))
                            .flex()
                            .items_center()
                            .h(px(row_h))
                            .px_1()
                            .rounded_sm()
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                cx.stop_propagation();
                                click_left.update(cx, |this, cx| this.source_mouse_down(event, window, cx));
                            })
                            .on_mouse_down(MouseButton::Middle, move |event, window, cx| {
                                cx.stop_propagation();
                                click_middle.update(cx, |this, cx| this.source_mouse_down(event, window, cx));
                            })
                            .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                                cx.stop_propagation();
                                click_right.update(cx, |this, cx| this.source_mouse_down(event, window, cx));
                            });

                        #[cfg(feature = "test-observation")]
                        crate::views::preview::obs::log_editor_tokens(phys_idx, &tokens);

                        let styled_row = if is_active_subline {
                            row.bg(list_theme.bg)
                        } else {
                            row.hover(|s| s.bg(list_theme.bg))
                        };

                        let positions = Arc::new(token_char_positions(window.text_system(), &tokens, font_size));
                        let cursor_x = positions[rel_cursor_col.min(positions.len() - 1)];

                        let row_offset = line_offsets[phys_idx] + sub_start;
                        let visual = vl.cloned().unwrap_or(VisualLine { physical_line_idx: phys_idx, subline_idx: 0, char_start: 0, char_len: sub_len, is_first: true });
                        let geometry_tokens = tokens.clone();
                        let geometry_positions = positions.clone();
                        let geometry_source = list_source.clone();
                        let selection_quads: Vec<_> = selection_ranges.iter().filter_map(|range| {
                            let start = range.start.max(row_offset);
                            let end = range.end.min(row_offset + sub_len);
                            if start > end || (start == end && !range.is_empty()) { return None; }
                            let x0 = positions[start - row_offset];
                            let x1 = positions[end - row_offset];
                            Some(div().debug_selector(move || format!("source_selection_{vis_idx}_{start}_{end}"))
                                .absolute().top_0().left(px(x0)).w(px((x1 - x0).max(2.))).h(px(row_h))
                                .bg(list_theme.selection_bg()).into_any_element())
                        }).collect();
                        let mut text_container = div()
                            .debug_selector(|| format!("editor_text_{phys_idx}"))
                            .flex_1()
                            .min_w(px(0.))
                            .h(px(row_h))
                            .relative()
                            .text_size(px(font_size))
                            .line_height(relative(sanitized_lh))
                            .font_family(crate::platform::mono_font())
                            .flex()
                            .items_center()
                            .children(selection_quads)
                            .children({
                                let mut token_offset = row_offset;
                                tokens.into_iter().map(|tok| {
                                    let length = tok.text.chars().count();
                                    let selected: Vec<_> = selection_ranges.iter().filter_map(|range| {
                                            let start = range.start.max(token_offset);
                                            let end = range.end.min(token_offset + length);
                                            (start < end).then_some(start.saturating_sub(token_offset)..end.saturating_sub(token_offset))
                                        }).collect();
                                    token_offset += length;
                                    render_markdown_token_element(&tok, &list_theme, &selected).into_any_element()
                                }).collect::<Vec<_>>()
                            })
                            .children(marked_range.as_ref().and_then(|range| {
                                let start = range.start.max(row_offset);
                                let end = range.end.min(row_offset + sub_len);
                                if start >= end { return None; }
                                let x0 = positions[start - row_offset];
                                let x1 = positions[end - row_offset];
                                Some(div().debug_selector(move || format!("source_marked_{vis_idx}_{start}_{end}"))
                                    .absolute().bottom_0().left(px(x0.min(x1))).w(px((x1 - x0).abs().max(1.))).h(px(1.))
                                    .bg(list_theme.text_primary).into_any_element())
                            }))
                            .child(canvas(move |bounds, _, cx| {
                                let source = geometry_source.read(cx);
                                let mut rows = source.source_rows.lock().unwrap();
                                rows.retain(|row| row.index != vis_idx);
                                rows.push(crate::views::workspace::SourceRow {
                                    bounds, index: vis_idx, visual: visual.clone(), offset: row_offset, tokens: geometry_tokens.clone(), positions: geometry_positions.clone(),
                                });
                            }, |_, _, _, _| {}).absolute().size_full());

                        if is_active_subline {
                            text_container = text_container.child(
                                div()
                                    .absolute()
                                    .top(px(((row_h - font_size * 1.2) * 0.5).max(0.0)))
                                    .left(px(cursor_x))
                                    .w(px(2.))
                                    .h(px(font_size * 1.2))
                                    .bg(list_theme.accent),
                            );
                        }

                        styled_row
                            .child(
                                div()
                                    .debug_selector(|| format!("editor_gutter_{phys_idx}"))
                                    .w(px(gutter_width))
                                    .flex_shrink_0()
                                    .mr(px(EDITOR_GUTTER_MARGIN_RIGHT))
                                    .text_right()
                                    .text_size(px(font_size * 0.85))
                                    .line_height(relative(sanitized_lh))
                                    .font_family(crate::platform::mono_font())
                                    .text_color(if is_active_subline {
                                        list_theme.accent
                                    } else {
                                        list_theme.text_muted.opacity(0.55)
                                    })
                                    .child(gutter_text),
                            )
                            .child(text_container.into_any_element())
                    })
                    .collect()
            })
            .track_scroll(scroll_handle)
            .flex_1()
            .w_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .px(px(EDITOR_CONTAINER_PADDING_X))
            .py_4(),
        )
        .child(canvas(|bounds, _, _| bounds, move |bounds, _, window, cx| {
            let focus = source_view.read(cx).focus_handle.clone();
            if source_view.read(cx).source_accepts_input(window, cx) {
                window.handle_input(&focus, ElementInputHandler::new(bounds, source_view.clone()), cx);
            }
            source_view.update(cx, |this, cx| this.source_after_paint(window, cx));
        }).absolute().size_full())
        .child(
            div()
                .absolute()
                .bottom_3()
                .right_5()
                .flex()
                .items_center()
                .gap_2()
                .when(!failures.is_empty(), |el| {
                    let first = &failures[0];
                    let badge_text = if failures.len() == 1 {
                        format!("L{} {} · {}", first.source_line, first.reason, strings.syntax_plain_readable)
                    } else {
                        format!("L{} {} · {} (+{})", first.source_line, first.reason, strings.syntax_plain_readable, failures.len() - 1)
                    };
                    let tooltip_text = failures
                        .iter()
                        .map(|f| format!("L{} {} · {}", f.source_line, f.reason, strings.syntax_plain_readable))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let on_click = on_click_line.clone();
                    let target_vis = visual_lines.iter().position(|vl| vl.physical_line_idx == first.source_line - 1).unwrap_or(0);
                    el.child(
                        div()
                            .id("editor_syntax_failure_badge")
                            .debug_selector(|| "editor_syntax_failure_badge".into())
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_1()
                            .rounded_full()
                            .bg(theme.bg)
                            .border_1()
                            .border_color(theme.border)
                            .text_size(px(10.))
                            .font_family(crate::platform::ui_font())
                            .text_color(theme.text_muted)
                            .tooltip(move |_window, cx| {
                                let t = tooltip_text.clone();
                                cx.new(|_| crate::views::workspace::TooltipView { text: t }).into()
                            })
                            .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                cx.stop_propagation();
                                on_click(target_vis, 0, window, cx);
                            })
                            .child(badge_text),
                    )
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .px_2()
                        .py_1()
                        .rounded_full()
                        .bg(theme.bg)
                        .border_1()
                        .border_color(theme.border)
                        .text_size(px(10.))
                        .font_family(crate::platform::ui_font())
                        .text_color(theme.text_muted)
                        .child(format!(
                            "{} {}  ·  {} {}",
                            strings.status_line,
                            cursor_line + 1,
                            char_count,
                            strings.status_chars
                        )),
                ),
        )
}








#[cfg(test)]
mod gutter_tests {
    use super::gutter_visible;

    #[test]
    fn line_numbers_follow_the_preference_and_first_subline() {
        assert!(gutter_visible(true, true));
        assert!(!gutter_visible(false, true));
        assert!(!gutter_visible(true, false));
        assert!(!gutter_visible(false, false));
    }
}
