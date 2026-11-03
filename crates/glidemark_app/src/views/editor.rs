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

pub fn token_paddings(kind: &MarkdownTokenKind) -> (f32, f32) {
    match kind {
        MarkdownTokenKind::InlineCode => (4.0, 4.0),
        MarkdownTokenKind::InlineCodeSplitLeft => (4.0, 0.0),
        MarkdownTokenKind::InlineCodeSplitRight => (0.0, 4.0),
        _ => (0.0, 0.0),
    }
}

/// Computes the horizontal pixel offset where the cursor should be rendered for `char_col`
/// within a subline comprised of `tokens`, taking into account font shaping and style paddings.
pub fn calculate_cursor_x_offset_tokens(
    text_system: &gpui::WindowTextSystem,
    tokens: &[MarkdownToken],
    char_col: usize,
    font_size: f32,
) -> f32 {
    if tokens.is_empty() || char_col == 0 {
        return 0.0;
    }

    let font = gpui::font("ui-monospace");
    let font_size_px = gpui::px(font_size);
    let mut curr_char_idx = 0;
    let mut curr_x = 0.0;

    for (i, tok) in tokens.iter().enumerate() {
        let tok_char_count = tok.text.chars().count();
        let (left_pad, right_pad) = token_paddings(&tok.kind);
        let is_last_tok = i + 1 == tokens.len();

        let run = gpui::TextRun {
            len: tok.text.len(),
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = text_system.shape_line(
            gpui::SharedString::from(tok.text.clone()),
            font_size_px,
            &[run],
            None,
        );
        let text_w = f32::from(shaped.width);

        if char_col < curr_char_idx + tok_char_count || is_last_tok {
            let offset_in_tok = char_col.saturating_sub(curr_char_idx).min(tok_char_count);
            let byte_idx = char_index_to_byte_offset(&tok.text, offset_in_tok);
            let text_x = if offset_in_tok == tok_char_count {
                text_w
            } else {
                f32::from(shaped.x_for_index(byte_idx))
            };
            let extra_right = if is_last_tok && char_col >= curr_char_idx + tok_char_count {
                right_pad
            } else {
                0.0
            };
            return curr_x + left_pad + text_x + extra_right;
        }

        curr_x += left_pad + text_w + right_pad;
        curr_char_idx += tok_char_count;
    }

    curr_x
}

fn closest_byte_index_in_shaped(shaped: &gpui::ShapedLine, target_x: gpui::Pixels) -> usize {
    if shaped.len == 0 {
        return 0;
    }
    let mut prev_idx = 0;
    let mut prev_x = gpui::px(0.);
    for run in &shaped.runs {
        for glyph in &run.glyphs {
            if glyph.position.x >= target_x {
                return if glyph.position.x - target_x < target_x - prev_x {
                    glyph.index
                } else {
                    prev_idx
                };
            }
            prev_idx = glyph.index;
            prev_x = glyph.position.x;
        }
    }

    if shaped.width - target_x < target_x - prev_x {
        shaped.len
    } else {
        prev_idx
    }
}

/// Computes the character column index closest to `x_offset` within a subline comprised
/// of `tokens`, taking into account font shaping and style paddings.
pub fn calculate_click_char_col_tokens(
    text_system: &gpui::WindowTextSystem,
    tokens: &[MarkdownToken],
    x_offset: f32,
    font_size: f32,
) -> usize {
    if tokens.is_empty() || x_offset <= 0.0 {
        return 0;
    }

    let font = gpui::font("ui-monospace");
    let font_size_px = gpui::px(font_size);
    let mut curr_char_idx = 0;
    let mut curr_x = 0.0;

    for (i, tok) in tokens.iter().enumerate() {
        let tok_char_count = tok.text.chars().count();
        let (left_pad, right_pad) = token_paddings(&tok.kind);
        let is_last_tok = i + 1 == tokens.len();

        let run = gpui::TextRun {
            len: tok.text.len(),
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = text_system.shape_line(
            gpui::SharedString::from(tok.text.clone()),
            font_size_px,
            &[run],
            None,
        );
        let text_w = f32::from(shaped.width);
        let total_tok_w = left_pad + text_w + right_pad;

        if !is_last_tok {
            if x_offset < curr_x + total_tok_w {
                if x_offset <= curr_x + left_pad {
                    return curr_char_idx;
                } else if x_offset >= curr_x + left_pad + text_w {
                    return curr_char_idx + tok_char_count;
                } else {
                    let rel_x = x_offset - (curr_x + left_pad);
                    let byte_idx = closest_byte_index_in_shaped(&shaped, gpui::px(rel_x));
                    let char_offset = byte_offset_to_char_index(&tok.text, byte_idx);
                    return curr_char_idx + char_offset;
                }
            }
        } else {
            // Last token
            if x_offset >= curr_x + total_tok_w {
                return curr_char_idx + tok_char_count;
            } else if x_offset <= curr_x + left_pad {
                return curr_char_idx;
            } else if x_offset >= curr_x + left_pad + text_w {
                return curr_char_idx + tok_char_count;
            } else {
                let rel_x = x_offset - (curr_x + left_pad);
                let byte_idx = closest_byte_index_in_shaped(&shaped, gpui::px(rel_x));
                let char_offset = byte_offset_to_char_index(&tok.text, byte_idx);
                return curr_char_idx + char_offset;
            }
        }

        curr_x += total_tok_w;
        curr_char_idx += tok_char_count;
    }

    curr_char_idx
}

/// Measures actual rendered pixel width for each character in `tokens`, accounting for font
/// shaping advances (such as `\t`) and inline code pill paddings.
pub fn compute_char_pixel_widths(
    text_system: &gpui::WindowTextSystem,
    tokens: &[MarkdownToken],
    font_size: f32,
) -> Vec<f32> {
    let font = gpui::font("ui-monospace");
    let font_size_px = gpui::px(font_size);
    let mut widths = Vec::new();

    for tok in tokens {
        let (left_pad, right_pad) = token_paddings(&tok.kind);
        let chars: Vec<char> = tok.text.chars().collect();
        let char_count = chars.len();
        if char_count == 0 {
            continue;
        }

        let run = gpui::TextRun {
            len: tok.text.len(),
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = text_system.shape_line(
            gpui::SharedString::from(tok.text.clone()),
            font_size_px,
            &[run],
            None,
        );

        for (k, _) in chars.iter().enumerate() {
            let start_byte = char_index_to_byte_offset(&tok.text, k);
            let end_byte = char_index_to_byte_offset(&tok.text, k + 1);
            let start_x = f32::from(shaped.x_for_index(start_byte));
            let end_x = if k + 1 == char_count {
                f32::from(shaped.width)
            } else {
                f32::from(shaped.x_for_index(end_byte))
            };
            let mut w = (end_x - start_x).max(0.0);
            if k == 0 {
                w += left_pad;
            }
            if k + 1 == char_count {
                w += right_pad;
            }
            widths.push(w);
        }
    }

    widths
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

/// Wraps all physical lines of a document into visual lines using actual layout metrics
/// (font advance, Tab width, and token paddings) against `max_width_px`.
#[allow(dead_code)]
pub fn wrap_document_to_visual_layout(
    text_system: &gpui::WindowTextSystem,
    lines: &[String],
    max_width_px: f32,
    font_size: f32,
) -> Vec<VisualLine> {
    let fenced_lines = compute_fenced_code_lines(lines);
    let mut result = Vec::new();
    for (phys_idx, line) in lines.iter().enumerate() {
        let is_fenced = fenced_lines.get(phys_idx).copied().unwrap_or(false);
        let tokens = if is_fenced {
            tokenize_code_line(line)
        } else {
            tokenize_markdown_line(line)
        };
        let wrapped = wrap_line_to_visual_layout(
            text_system,
            phys_idx,
            line,
            &tokens,
            max_width_px,
            font_size,
        );
        result.extend(wrapped);
    }
    if result.is_empty() {
        result.push(VisualLine {
            physical_line_idx: 0,
            subline_idx: 0,
            char_start: 0,
            char_len: 0,
            is_first: true,
        });
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineVisualSpan {
    pub char_start: usize,
    pub char_len: usize,
    pub is_first: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedLineEntry {
    pub text: String,
    pub is_fenced: bool,
    pub spans: Vec<LineVisualSpan>,
    pub natural_width_px: f32,
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
    pub metrics: VisualLayoutMetrics,
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
        tokenize_code_line(line)
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
    CachedLineEntry {
        text: line.to_string(),
        is_fenced,
        spans,
        natural_width_px,
    }
}

fn evaluate_matching_line(
    old: &CachedLineEntry,
    i: usize,
    line: &str,
    is_fenced: bool,
    width_changed: bool,
    max_width_px: f32,
    font_size: f32,
    text_system: &gpui::WindowTextSystem,
) -> (CachedLineEntry, bool) {
    if !width_changed {
        (old.clone(), true)
    } else if old.natural_width_px <= max_width_px {
        (
            CachedLineEntry {
                text: line.to_string(),
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
            shape_single_line(text_system, i, line, is_fenced, max_width_px, font_size),
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
        max_width_px: f32,
        font_size: f32,
    ) -> Vec<VisualLine> {
        let width_bits = max_width_px.round().to_bits();
        let font_bits = font_size.to_bits();

        let font_changed = self.cached_font_bits != font_bits;
        let width_changed = self.cached_width_bits != width_bits;

        self.cached_width_bits = width_bits;
        self.cached_font_bits = font_bits;

        if font_changed {
            self.entries.clear();
        }

        let fenced_lines = compute_fenced_code_lines(lines);
        let mut shaped_count = 0;
        let mut reused_count = 0;

        let old_entries = std::mem::take(&mut self.entries);
        let mut new_entries = Vec::with_capacity(lines.len());

        if !font_changed && old_entries.len() == lines.len() {
            for (i, (line, &is_fenced)) in lines.iter().zip(fenced_lines.iter()).enumerate() {
                let old = &old_entries[i];
                if old.is_fenced == is_fenced && old.text == *line {
                    let (entry, is_reused) = evaluate_matching_line(
                        old,
                        i,
                        line,
                        is_fenced,
                        width_changed,
                        max_width_px,
                        font_size,
                        text_system,
                    );
                    new_entries.push(entry);
                    if is_reused {
                        reused_count += 1;
                    } else {
                        shaped_count += 1;
                    }
                } else {
                    new_entries.push(shape_single_line(
                        text_system,
                        i,
                        line,
                        is_fenced,
                        max_width_px,
                        font_size,
                    ));
                    shaped_count += 1;
                }
            }
        } else if !font_changed && !old_entries.is_empty() {
            let mut prefix = 0;
            while prefix < lines.len()
                && prefix < old_entries.len()
                && old_entries[prefix].is_fenced == fenced_lines[prefix]
                && old_entries[prefix].text == lines[prefix]
            {
                prefix += 1;
            }

            let mut suffix = 0;
            while suffix < (lines.len() - prefix)
                && suffix < (old_entries.len() - prefix)
                && old_entries[old_entries.len() - 1 - suffix].is_fenced
                    == fenced_lines[lines.len() - 1 - suffix]
                && old_entries[old_entries.len() - 1 - suffix].text
                    == lines[lines.len() - 1 - suffix]
            {
                suffix += 1;
            }

            for (i, old) in old_entries[..prefix].iter().enumerate() {
                let (entry, is_reused) = evaluate_matching_line(
                    old,
                    i,
                    &lines[i],
                    fenced_lines[i],
                    width_changed,
                    max_width_px,
                    font_size,
                    text_system,
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
                new_entries.push(shape_single_line(
                    text_system,
                    i,
                    &lines[i],
                    fenced_lines[i],
                    max_width_px,
                    font_size,
                ));
                shaped_count += 1;
            }

            let old_suffix_start = old_entries.len() - suffix;
            for (offset, old) in old_entries[old_suffix_start..].iter().enumerate() {
                let i = middle_end + offset;
                let (entry, is_reused) = evaluate_matching_line(
                    old,
                    i,
                    &lines[i],
                    fenced_lines[i],
                    width_changed,
                    max_width_px,
                    font_size,
                    text_system,
                );
                new_entries.push(entry);
                if is_reused {
                    reused_count += 1;
                } else {
                    shaped_count += 1;
                }
            }
        } else {
            for (i, (line, &is_fenced)) in lines.iter().zip(fenced_lines.iter()).enumerate() {
                new_entries.push(shape_single_line(
                    text_system,
                    i,
                    line,
                    is_fenced,
                    max_width_px,
                    font_size,
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

/// Computes the character column index closest to `x_offset` within `subline_str`
/// using GPUI's text system and actual font metrics.
#[allow(dead_code)]
pub fn calculate_click_char_col_layout(
    text_system: &gpui::WindowTextSystem,
    subline_str: &str,
    x_offset: f32,
    font_size: f32,
) -> usize {
    let tokens = [MarkdownToken {
        text: subline_str.to_string(),
        kind: MarkdownTokenKind::Plain,
    }];
    calculate_click_char_col_tokens(text_system, &tokens, x_offset, font_size)
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
    visual_lines: Arc<Vec<VisualLine>>,
    cursor: (usize, usize),
    theme: &ThemeColors,
    strings: &'static glidemark_core::i18n::I18nStrings,
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

                        let on_click_line = on_click_line.clone();
                        let row_h = calculate_line_row_height(font_size, sanitized_lh);
                        let tokens_click = tokens.clone();

                        let row = div()
                            .flex()
                            .items_center()
                            .h(px(row_h))
                            .px_1()
                            .rounded_sm()
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
                                let x_offset = (f32::from(event.position.x) - editor_left_offset).max(0.0);
                                let rel_col = calculate_click_char_col_tokens(window.text_system(), &tokens_click, x_offset, font_size);
                                on_click_line(vis_idx, rel_col, window, cx);
                            });

                        let styled_row = if is_active_subline {
                            row.bg(list_theme.bg)
                        } else {
                            row.hover(|s| s.bg(list_theme.bg))
                        };

                        let cursor_x = if is_active_subline {
                            let char_count = subline_str.chars().count();
                            let col = rel_cursor_col.min(char_count);
                            calculate_cursor_x_offset_tokens(window.text_system(), &tokens, col, font_size)
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
                .font_family("ui-sans-serif, system-ui")
                .text_color(theme.text_muted)
                .when(is_dirty, |el| {
                    el.child(div().w(px(5.)).h(px(5.)).rounded_full().bg(theme.accent))
                })
                .child(format!("{} {}, {} {}", strings.status_line, cursor_line + 1, strings.status_col, cursor_col + 1)),
        )
}







