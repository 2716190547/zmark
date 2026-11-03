use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use crate::editor_engine::VisualLine;
use crate::theme::ThemeColors;

pub fn calculate_line_row_height(font_size: f32, line_height: f32) -> f32 {
    let lh = glidemark_core::config::Preferences::sanitize_line_height(line_height);
    font_size * lh
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownTokenKind {
    HeadingPrefix,
    HeadingText,
    BlockquotePrefix,
    BlockquoteText,
    BulletPrefix,
    InlineCode,
    InlineCodeSplitLeft,
    InlineCodeSplitRight,
    Bold,
    LinkText,
    LinkUrl,
    Keyword,
    StringLiteral,
    Comment,
    CodeDelimiter,
    CodeText,
    Plain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownToken {
    pub text: String,
    pub kind: MarkdownTokenKind,
}

enum MatchKind {
    InlineCode(usize, usize),
    Bold(usize, usize),
    Link(usize, usize, usize),
}

fn find_inline_code(line: &str, from: usize) -> Option<(usize, usize)> {
    let mut search_from = from;
    while let Some(rel_start) = line[search_from..].find('`') {
        let start = search_from + rel_start;
        if let Some(rel_end) = line[start + 1..].find('`') {
            if rel_end > 0 {
                let end = start + 1 + rel_end + 1;
                return Some((start, end));
            } else {
                search_from = start + 2;
            }
        } else {
            return None;
        }
    }
    None
}

fn find_bold(line: &str, from: usize) -> Option<(usize, usize)> {
    let mut search_from = from;
    while let Some(rel_start) = line[search_from..].find("**") {
        let start = search_from + rel_start;
        if let Some(rel_end) = line[start + 2..].find("**") {
            if rel_end > 0 {
                let end = start + 2 + rel_end + 2;
                return Some((start, end));
            } else {
                search_from = start + 2;
            }
        } else {
            return None;
        }
    }
    None
}

fn find_link(line: &str, from: usize) -> Option<(usize, usize, usize)> {
    let mut search_from = from;
    while let Some(rel_start) = line[search_from..].find('[') {
        let start = search_from + rel_start;
        if let Some(rel_bracket) = line[start + 1..].find(']') {
            let close_bracket = start + 1 + rel_bracket;
            if line[close_bracket + 1..].starts_with('(') {
                let open_paren = close_bracket + 1;
                if let Some(rel_paren) = line[open_paren + 1..].find(')') {
                    let close_paren = open_paren + 1 + rel_paren;
                    return Some((start, close_bracket + 1, close_paren + 1));
                }
            }
            search_from = close_bracket + 1;
        } else {
            return None;
        }
    }
    None
}

pub fn tokenize_markdown_line(line: &str) -> Vec<MarkdownToken> {
    if line.is_empty() {
        return vec![MarkdownToken {
            text: " ".to_string(),
            kind: MarkdownTokenKind::Plain,
        }];
    }

    let trimmed = line.trim();
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        return vec![MarkdownToken {
            text: line.to_string(),
            kind: MarkdownTokenKind::CodeDelimiter,
        }];
    }

    // Heading: Check if line starts with 1 to 6 '#' followed by a space
    let hash_count = line.chars().take_while(|&c| c == '#').count();
    if hash_count >= 1 && hash_count <= 6 && line[hash_count..].starts_with(' ') {
        let prefix = &line[..hash_count + 1];
        let remainder = &line[hash_count + 1..];
        let mut tokens = vec![MarkdownToken {
            text: prefix.to_string(),
            kind: MarkdownTokenKind::HeadingPrefix,
        }];
        if !remainder.is_empty() {
            tokens.push(MarkdownToken {
                text: remainder.to_string(),
                kind: MarkdownTokenKind::HeadingText,
            });
        }
        return tokens;
    }

    // Blockquote: Check if line starts with '>' (e.g. '> ' or '>')
    if line.starts_with("> ") {
        let prefix = "> ";
        let remainder = &line[2..];
        let mut tokens = vec![MarkdownToken {
            text: prefix.to_string(),
            kind: MarkdownTokenKind::BlockquotePrefix,
        }];
        if !remainder.is_empty() {
            tokens.push(MarkdownToken {
                text: remainder.to_string(),
                kind: MarkdownTokenKind::BlockquoteText,
            });
        }
        return tokens;
    } else if line.starts_with('>') {
        let prefix = ">";
        let remainder = &line[1..];
        let mut tokens = vec![MarkdownToken {
            text: prefix.to_string(),
            kind: MarkdownTokenKind::BlockquotePrefix,
        }];
        if !remainder.is_empty() {
            tokens.push(MarkdownToken {
                text: remainder.to_string(),
                kind: MarkdownTokenKind::BlockquoteText,
            });
        }
        return tokens;
    }

    // List prefixes (task list, unordered list, ordered list)
    let indent_len = line.bytes().take_while(|&b| b == b' ' || b == b'\t').count();
    let trimmed = &line[indent_len..];

    let list_prefix_len = if trimmed.starts_with("- [ ] ")
        || trimmed.starts_with("- [x] ")
        || trimmed.starts_with("- [X] ")
        || trimmed.starts_with("* [ ] ")
        || trimmed.starts_with("* [x] ")
        || trimmed.starts_with("* [X] ")
        || trimmed.starts_with("+ [ ] ")
        || trimmed.starts_with("+ [x] ")
        || trimmed.starts_with("+ [X] ")
    {
        Some(6)
    } else if trimmed == "- [ ]"
        || trimmed == "- [x]"
        || trimmed == "- [X]"
        || trimmed == "* [ ]"
        || trimmed == "* [x]"
        || trimmed == "* [X]"
        || trimmed == "+ [ ]"
        || trimmed == "+ [x]"
        || trimmed == "+ [X]"
    {
        Some(5)
    } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        Some(2)
    } else {
        let digit_count = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
        if digit_count >= 1 && digit_count <= 9 {
            let after_digits = &trimmed[digit_count..];
            if after_digits.starts_with(". ") {
                Some(digit_count + 2)
            } else if after_digits == "." {
                Some(digit_count + 1)
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some(prefix_len) = list_prefix_len {
        let prefix = &trimmed[..prefix_len];
        let remainder = &trimmed[prefix_len..];
        let mut tokens = Vec::new();
        if indent_len > 0 {
            tokens.push(MarkdownToken {
                text: line[..indent_len].to_string(),
                kind: MarkdownTokenKind::Plain,
            });
        }
        tokens.push(MarkdownToken {
            text: prefix.to_string(),
            kind: MarkdownTokenKind::BulletPrefix,
        });
        if !remainder.is_empty() {
            tokens.extend(tokenize_inline(remainder));
        }
        return tokens;
    }

    tokenize_inline(line)
}

pub fn tokenize_inline(line: &str) -> Vec<MarkdownToken> {
    let mut tokens = Vec::new();
    let mut cursor = 0;

    while cursor < line.len() {
        let mut earliest: Option<MatchKind> = None;
        let mut earliest_start = usize::MAX;

        if let Some((start, end)) = find_inline_code(line, cursor) {
            if start < earliest_start {
                earliest_start = start;
                earliest = Some(MatchKind::InlineCode(start, end));
            }
        }
        if let Some((start, end)) = find_bold(line, cursor) {
            if start < earliest_start {
                earliest_start = start;
                earliest = Some(MatchKind::Bold(start, end));
            }
        }
        if let Some((start, text_end, url_end)) = find_link(line, cursor) {
            if start < earliest_start {
                earliest_start = start;
                earliest = Some(MatchKind::Link(start, text_end, url_end));
            }
        }

        if let Some(m) = earliest {
            if earliest_start > cursor {
                tokens.push(MarkdownToken {
                    text: line[cursor..earliest_start].to_string(),
                    kind: MarkdownTokenKind::Plain,
                });
            }
            match m {
                MatchKind::InlineCode(start, end) => {
                    tokens.push(MarkdownToken {
                        text: line[start..end].to_string(),
                        kind: MarkdownTokenKind::InlineCode,
                    });
                    cursor = end;
                }
                MatchKind::Bold(start, end) => {
                    tokens.push(MarkdownToken {
                        text: line[start..end].to_string(),
                        kind: MarkdownTokenKind::Bold,
                    });
                    cursor = end;
                }
                MatchKind::Link(start, text_end, url_end) => {
                    tokens.push(MarkdownToken {
                        text: line[start..text_end].to_string(),
                        kind: MarkdownTokenKind::LinkText,
                    });
                    tokens.push(MarkdownToken {
                        text: line[text_end..url_end].to_string(),
                        kind: MarkdownTokenKind::LinkUrl,
                    });
                    cursor = url_end;
                }
            }
        } else {
            tokens.push(MarkdownToken {
                text: line[cursor..].to_string(),
                kind: MarkdownTokenKind::Plain,
            });
            break;
        }
    }

    tokens
}

pub fn compute_fenced_code_lines(lines: &[String]) -> Vec<bool> {
    let mut in_code = false;
    let mut is_fenced = Vec::with_capacity(lines.len());
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code = !in_code;
            is_fenced.push(true);
        } else {
            is_fenced.push(in_code);
        }
    }
    is_fenced
}

pub fn tokenize_code_line(line: &str) -> Vec<MarkdownToken> {
    if line.is_empty() {
        return vec![MarkdownToken {
            text: " ".to_string(),
            kind: MarkdownTokenKind::CodeText,
        }];
    }

    let trimmed = line.trim();
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        return vec![MarkdownToken {
            text: line.to_string(),
            kind: MarkdownTokenKind::CodeDelimiter,
        }];
    }

    const KEYWORDS: &[&str] = &[
        "fn", "let", "pub", "struct", "impl", "mut", "if", "else", "return", "match",
        "for", "in", "while", "loop", "const", "static", "enum", "type", "use", "mod",
        "trait", "async", "await", "def", "class", "import", "from", "function", "var", "val",
    ];

    let mut tokens = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Comments: //... or #...
        if (chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '/') || chars[i] == '#' {
            let comment_text: String = chars[i..].iter().collect();
            tokens.push(MarkdownToken {
                text: comment_text,
                kind: MarkdownTokenKind::Comment,
            });
            break;
        }

        // Strings: "..." or '...'
        if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            let start = i;
            i += 1;
            let mut escaped = false;
            while i < chars.len() {
                if escaped {
                    escaped = false;
                    i += 1;
                } else if chars[i] == '\\' {
                    escaped = true;
                    i += 1;
                } else if chars[i] == quote {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
            let str_text: String = chars[start..i].iter().collect();
            tokens.push(MarkdownToken {
                text: str_text,
                kind: MarkdownTokenKind::StringLiteral,
            });
            continue;
        }

        // Identifiers / keywords: starts with alphabetic or '_'
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let kind = if KEYWORDS.contains(&word.as_str()) {
                MarkdownTokenKind::Keyword
            } else {
                MarkdownTokenKind::CodeText
            };
            tokens.push(MarkdownToken {
                text: word,
                kind,
            });
            continue;
        }

        // Other characters (whitespace, punctuation, numbers, operators)
        let start = i;
        while i < chars.len()
            && !chars[i].is_alphabetic()
            && chars[i] != '_'
            && chars[i] != '"'
            && chars[i] != '\''
            && !(chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '/')
            && chars[i] != '#'
        {
            i += 1;
        }
        let other: String = chars[start..i].iter().collect();
        tokens.push(MarkdownToken {
            text: other,
            kind: MarkdownTokenKind::CodeText,
        });
    }

    tokens
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveLineSegment {
    Token(MarkdownToken),
    Cursor,
}

#[allow(dead_code)]
pub fn build_active_line_segments(tokens: &[MarkdownToken], cursor_col: usize) -> Vec<ActiveLineSegment> {
    let mut segments = Vec::new();
    let mut curr_char_idx = 0;
    let mut cursor_placed = false;

    for tok in tokens {
        let tok_len = tok.text.chars().count();
        if !cursor_placed && cursor_col >= curr_char_idx && cursor_col <= curr_char_idx + tok_len {
            let offset = cursor_col - curr_char_idx;
            let before: String = tok.text.chars().take(offset).collect();
            let after: String = tok.text.chars().skip(offset).collect();

            let (before_kind, after_kind) = if tok.kind == MarkdownTokenKind::InlineCode {
                if !before.is_empty() && !after.is_empty() {
                    (
                        MarkdownTokenKind::InlineCodeSplitLeft,
                        MarkdownTokenKind::InlineCodeSplitRight,
                    )
                } else {
                    (MarkdownTokenKind::InlineCode, MarkdownTokenKind::InlineCode)
                }
            } else {
                (tok.kind.clone(), tok.kind.clone())
            };

            if !before.is_empty() {
                segments.push(ActiveLineSegment::Token(MarkdownToken {
                    text: before,
                    kind: before_kind,
                }));
            }
            segments.push(ActiveLineSegment::Cursor);
            if !after.is_empty() {
                segments.push(ActiveLineSegment::Token(MarkdownToken {
                    text: after,
                    kind: after_kind,
                }));
            }
            cursor_placed = true;
        } else {
            segments.push(ActiveLineSegment::Token(tok.clone()));
        }
        curr_char_idx += tok_len;
    }

    if !cursor_placed {
        segments.push(ActiveLineSegment::Cursor);
    }

    segments
}

fn render_markdown_token_element(tok: &MarkdownToken, theme: &ThemeColors) -> impl IntoElement {
    let mut el = div().child(tok.text.clone());
    match tok.kind {
        MarkdownTokenKind::Keyword => {
            el = el.text_color(theme.syntax_keyword).font_weight(FontWeight::BOLD);
        }
        MarkdownTokenKind::StringLiteral => {
            el = el.text_color(theme.syntax_string);
        }
        MarkdownTokenKind::Comment => {
            el = el.text_color(theme.syntax_comment).italic();
        }
        MarkdownTokenKind::HeadingPrefix | MarkdownTokenKind::HeadingText => {
            el = el.text_color(theme.syntax_heading).font_weight(FontWeight::BOLD);
        }
        MarkdownTokenKind::BlockquotePrefix | MarkdownTokenKind::BlockquoteText => {
            el = el.text_color(theme.text_muted);
        }
        MarkdownTokenKind::BulletPrefix => {
            el = el.text_color(theme.syntax_bullet);
        }
        MarkdownTokenKind::InlineCode => {
            el = el.text_color(theme.syntax_inline_code)
                .bg(theme.code_bg)
                .rounded(px(3.))
                .px(px(4.));
        }
        MarkdownTokenKind::InlineCodeSplitLeft => {
            el = el.text_color(theme.syntax_inline_code)
                .bg(theme.code_bg)
                .rounded_l(px(3.))
                .pl(px(4.))
                .pr_0();
        }
        MarkdownTokenKind::InlineCodeSplitRight => {
            el = el.text_color(theme.syntax_inline_code)
                .bg(theme.code_bg)
                .rounded_r(px(3.))
                .pr(px(4.))
                .pl_0();
        }
        MarkdownTokenKind::Bold => {
            el = el.text_color(theme.text_primary).font_weight(FontWeight::BOLD);
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

pub const DRAWER_WIDTH: f32 = 250.0;
pub const EDITOR_CONTAINER_PADDING_X: f32 = 16.0; // uniform_list p_4
pub const EDITOR_ROW_PADDING_X: f32 = 4.0;        // row px_1
pub const EDITOR_GUTTER_WIDTH: f32 = 36.0;        // gutter w(px(36.))
pub const EDITOR_GUTTER_MARGIN_RIGHT: f32 = 12.0; // gutter mr_3
pub const EDITOR_TEXT_START_OFFSET: f32 =
    EDITOR_CONTAINER_PADDING_X + EDITOR_ROW_PADDING_X + EDITOR_GUTTER_WIDTH + EDITOR_GUTTER_MARGIN_RIGHT; // 68.0px

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

/// Computes the character column index closest to `x_offset` within `subline_str`
/// using GPUI's text system and actual font metrics.
pub fn calculate_click_char_col_layout(
    text_system: &gpui::WindowTextSystem,
    subline_str: &str,
    x_offset: f32,
    font_size: f32,
) -> usize {
    if subline_str.is_empty() || x_offset <= 0.0 {
        return 0;
    }
    let font = gpui::font("ui-monospace");
    let font_size_px = gpui::px(font_size);
    let run = gpui::TextRun {
        len: subline_str.len(),
        font,
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = text_system.shape_line(gpui::SharedString::from(subline_str.to_string()), font_size_px, &[run], None);
    let target_x = gpui::px(x_offset);
    if target_x >= shaped.width {
        return subline_str.chars().count();
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
                return byte_offset_to_char_index(subline_str, byte_idx);
            }
            prev_idx = glyph.index;
            prev_x = glyph.position.x;
        }
    }

    let byte_idx = if shaped.width - target_x < target_x - prev_x {
        shaped.len
    } else {
        prev_idx
    };
    byte_offset_to_char_index(subline_str, byte_idx)
}

/// Computes the horizontal pixel offset where the cursor should be rendered for `char_col`
/// within `subline_str` using GPUI's text system and actual font metrics.
pub fn calculate_cursor_x_offset_layout(
    text_system: &gpui::WindowTextSystem,
    subline_str: &str,
    char_col: usize,
    font_size: f32,
) -> f32 {
    if subline_str.is_empty() || char_col == 0 {
        return 0.0;
    }
    let byte_idx = char_index_to_byte_offset(subline_str, char_col);
    let font = gpui::font("ui-monospace");
    let font_size_px = gpui::px(font_size);
    let run = gpui::TextRun {
        len: subline_str.len(),
        font,
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = text_system.shape_line(gpui::SharedString::from(subline_str.to_string()), font_size_px, &[run], None);
    f32::from(shaped.x_for_index(byte_idx))
}

#[allow(dead_code)]
pub fn calculate_click_char_col(subline_str: &str, x_offset: f32, font_size: f32) -> usize {
    let base_unit_w = (font_size * 0.6).max(1.0);
    let mut current_x = 0.0;
    for (idx, ch) in subline_str.chars().enumerate() {
        let w = (crate::editor_engine::char_unit_width(ch) as f32) * base_unit_w;
        if current_x + w * 0.5 >= x_offset {
            return idx;
        }
        current_x += w;
    }
    subline_str.chars().count()
}

pub fn calculate_editor_left_offset(drawer_open: bool, _split_preview_left_offset: f32) -> f32 {
    (if drawer_open { DRAWER_WIDTH } else { 0.0 }) + EDITOR_TEXT_START_OFFSET
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
                sliced.push(MarkdownToken {
                    text: sub_text,
                    kind: tok.kind.clone(),
                });
            }
        }
        tok_start = tok_end;
    }
    sliced
}

pub fn render_editor_pane(
    lines: Arc<Vec<String>>,
    visual_lines: Arc<Vec<VisualLine>>,
    cursor: (usize, usize),
    theme: &ThemeColors,
    font_size: f32,
    line_height: f32,
    is_dirty: bool,
    scroll_handle: &UniformListScrollHandle,
    editor_left_offset: f32,
    on_click_line: Arc<dyn Fn(usize, usize, &mut Window, &mut App)>,
) -> impl IntoElement {
    let line_count = visual_lines.len().max(1);
    let (cursor_line, cursor_col) = cursor;
    let theme = theme.clone();
    let list_theme = theme.clone();
    let sanitized_lh = glidemark_core::config::Preferences::sanitize_line_height(line_height);
    let fenced_lines = Arc::new(compute_fenced_code_lines(&lines));

    div()
        .flex_1()
        .min_h(px(0.))
        .h_full()
        .relative()
        .flex()
        .flex_col()
        .bg(theme.surface)
        .child(
            uniform_list("editor_lines", line_count, move |range, window, _cx| {
                range
                    .map(|vis_idx| {
                        let vl = visual_lines.get(vis_idx);
                        let phys_idx = vl.map(|v| v.physical_line_idx).unwrap_or(0);
                        let phys_line_str = lines.get(phys_idx).map(String::as_str).unwrap_or("");
                        let subline_str: String = if let Some(v) = vl {
                            phys_line_str.chars().skip(v.char_start).take(v.char_len).collect()
                        } else {
                            phys_line_str.to_string()
                        };

                        let is_last_subline_of_phys_line = visual_lines
                            .get(vis_idx + 1)
                            .map(|next| next.physical_line_idx != phys_idx)
                            .unwrap_or(true);

                        let is_active_subline = cursor_line == phys_idx
                            && cursor_col >= vl.map(|v| v.char_start).unwrap_or(0)
                            && (cursor_col < vl.map(|v| v.char_start + v.char_len).unwrap_or(0)
                                || is_last_subline_of_phys_line);
                        let rel_cursor_col = cursor_col.saturating_sub(vl.map(|v| v.char_start).unwrap_or(0));

                        let on_click_line = on_click_line.clone();
                        let row_h = calculate_line_row_height(font_size, sanitized_lh);
                        let subline_click = subline_str.clone();

                        let row = div()
                            .flex()
                            .items_center()
                            .h(px(row_h))
                            .px_1()
                            .rounded_sm()
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
                                let x_offset = (f32::from(event.position.x) - editor_left_offset).max(0.0);
                                let rel_col = calculate_click_char_col_layout(window.text_system(), &subline_click, x_offset, font_size);
                                on_click_line(vis_idx, rel_col, window, cx);
                            });

                        let styled_row = if is_active_subline {
                            row.bg(list_theme.bg)
                        } else {
                            row.hover(|s| s.bg(list_theme.bg))
                        };

                        let show_line_num = vl.map(|v| v.is_first).unwrap_or(true);
                        let gutter_text = if show_line_num {
                            format!("{}", phys_idx + 1)
                        } else {
                            String::new()
                        };

                        let is_fenced = fenced_lines.get(phys_idx).copied().unwrap_or(false);
                        let phys_tokens = if is_fenced {
                            tokenize_code_line(phys_line_str)
                        } else {
                            tokenize_markdown_line(phys_line_str)
                        };
                        let (sub_start, sub_len) = if let Some(v) = vl {
                            (v.char_start, v.char_len)
                        } else {
                            (0, phys_line_str.chars().count())
                        };
                        let tokens = slice_tokens_for_visual_subline(&phys_tokens, sub_start, sub_len);

                        let cursor_x = if is_active_subline {
                            let char_count = subline_str.chars().count();
                            let col = rel_cursor_col.min(char_count);
                            calculate_cursor_x_offset_layout(window.text_system(), &subline_str, col, font_size)
                        } else {
                            0.0
                        };

                        let mut text_container = div()
                            .flex_1()
                            .relative()
                            .text_size(px(font_size))
                            .line_height(relative(sanitized_lh))
                            .font_family("ui-monospace")
                            .flex()
                            .items_center()
                            .children(tokens.into_iter().map(|tok| {
                                render_markdown_token_element(&tok, &list_theme).into_any_element()
                            }));

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
                                    .w(px(36.))
                                    .flex_shrink_0()
                                    .mr_3()
                                    .text_right()
                                    .text_size(px(font_size * 0.85))
                                    .line_height(relative(sanitized_lh))
                                    .font_family("ui-monospace")
                                    .text_color(if is_active_subline { list_theme.accent } else { list_theme.text_muted })
                                    .child(gutter_text),
                            )
                            .child(text_container.into_any_element())
                    })
                    .collect()
            })
            .track_scroll(scroll_handle.clone())
            .flex_1()
            .w_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .p_4(),
        )
        .child(
            div()
                .absolute()
                .bottom_3()
                .right_3()
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
                .font_family("ui-monospace")
                .text_color(theme.text_muted)
                .when(is_dirty, |el| {
                    el.child(div().w(px(5.)).h(px(5.)).rounded_full().bg(theme.accent))
                })
                .child(format!("Ln {}, Col {}", cursor_line + 1, cursor_col + 1)),
        )
}


