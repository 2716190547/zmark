use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

use crate::anchor::SourceAnchor;

/// An inline formatted piece of text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InlineItem {
    Text(String),
    Bold(String),
    Italic(String),
    Strikethrough(String),
    Code(String),
    Link {
        text: String,
        url: String,
        title: Option<String>,
    },
    Image {
        alt: String,
        url: String,
        title: Option<String>,
    },
}

/// A collection of inline formatted pieces.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InlineGroup(pub Vec<InlineItem>);

impl InlineGroup {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, item: InlineItem) {
        self.0.push(item);
    }

    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for item in &self.0 {
            match item {
                InlineItem::Text(s)
                | InlineItem::Bold(s)
                | InlineItem::Italic(s)
                | InlineItem::Strikethrough(s)
                | InlineItem::Code(s) => out.push_str(s),
                InlineItem::Link { text, .. } => out.push_str(text),
                InlineItem::Image { alt, .. } => out.push_str(alt),
            }
        }
        out
    }
}

/// A parsed heading outline item for table of contents navigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadingItem {
    pub level: u8,
    pub text: String,
    pub block_index: usize,
}

/// An individual list item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    pub checked: Option<bool>,
    pub body: Vec<BlockElement>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlattenSegment {
    pub child_path: String,
    pub char_range: std::ops::Range<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlattenResult {
    pub text: String,
    pub segments: Vec<FlattenSegment>,
}

fn push_ensuring_single_lf(out: &mut String) {
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

fn indent_text(s: &str) -> String {
    let mut o = String::new();
    for line in s.split_inclusive('\n') {
        o.push_str("  ");
        o.push_str(line);
    }
    o
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

pub fn flatten_block(block: &BlockElement, path: &str) -> FlattenResult {
    match block {
        BlockElement::Heading { content, .. } => {
            let text = content.plain_text();
            let len = char_len(&text);
            FlattenResult {
                segments: if len > 0 {
                    vec![FlattenSegment {
                        child_path: path.to_string(),
                        char_range: 0..len,
                    }]
                } else {
                    Vec::new()
                },
                text,
            }
        }
        BlockElement::Paragraph(content) => {
            let text = content.plain_text();
            let len = char_len(&text);
            FlattenResult {
                segments: if len > 0 {
                    vec![FlattenSegment {
                        child_path: path.to_string(),
                        char_range: 0..len,
                    }]
                } else {
                    Vec::new()
                },
                text,
            }
        }
        BlockElement::FencedCode { code, .. } => {
            let len = char_len(code);
            FlattenResult {
                segments: if len > 0 {
                    vec![FlattenSegment {
                        child_path: path.to_string(),
                        char_range: 0..len,
                    }]
                } else {
                    Vec::new()
                },
                text: code.clone(),
            }
        }
        BlockElement::BlockQuote(children) => {
            let mut out = String::new();
            let mut segs = Vec::new();
            for (i, c) in children.iter().enumerate() {
                let child = flatten_block(c, &format!("{}.{}", path, i));
                if child.text.is_empty() {
                    segs.extend(child.segments);
                    continue;
                }
                push_ensuring_single_lf(&mut out);
                let start = char_len(&out);
                out.push_str(&child.text);
                let shift = start;
                let total = char_len(&out) - start;
                segs.push(FlattenSegment {
                    child_path: format!("{}.{}.all", path, i),
                    char_range: start..start + total,
                });
                segs.extend(child.segments.into_iter().map(|s| FlattenSegment {
                    child_path: s.child_path,
                    char_range: s.char_range.start + shift..s.char_range.end + shift,
                }));
            }
            FlattenResult {
                text: out,
                segments: segs,
            }
        }
        BlockElement::List { ordered, items } => {
            let mut out = String::new();
            let mut segs = Vec::new();
            for (idx, item) in items.iter().enumerate() {
                push_ensuring_single_lf(&mut out);
                let marker = if let Some(checked) = item.checked {
                    format!("- [{}] ", if checked { "x" } else { " " })
                } else if *ordered {
                    format!("{}. ", idx + 1)
                } else {
                    "- ".to_string()
                };
                out.push_str(&marker);
                for (ci, child) in item.body.iter().enumerate() {
                    let child_flat = flatten_block(child, &format!("{}.{}.{}", path, idx, ci));
                    if child_flat.text.is_empty() {
                        let cur = char_len(&out);
                        segs.extend(child_flat.segments.into_iter().map(|s| FlattenSegment {
                            child_path: s.child_path,
                            char_range: cur..cur,
                        }));
                        continue;
                    }
                    if ci > 0 {
                        push_ensuring_single_lf(&mut out);
                    }
                    let start = char_len(&out);
                    if ci == 0 {
                        let mut pieces = child_flat.text.split_inclusive('\n').peekable();
                        let mut first_piece = true;
                        while let Some(piece) = pieces.next() {
                            if first_piece {
                                out.push_str(piece);
                                first_piece = false;
                            } else {
                                out.push_str("  ");
                                out.push_str(piece);
                            }
                        }
                    } else {
                        out.push_str(&indent_text(&child_flat.text));
                    }
                    let total = char_len(&out) - start;
                    segs.push(FlattenSegment {
                        child_path: format!("{}.{}.{}.all", path, idx, ci),
                        char_range: start..start + total,
                    });
                    let child_text = &child_flat.text;
                    let is_first = ci == 0;
                    let map_child_offset = |c: usize| -> usize {
                        let c_clamped = c.min(char_len(child_text));
                        let newlines = child_text.chars().take(c_clamped).filter(|&ch| ch == '\n').count();
                        if is_first {
                            start + c + 2 * newlines
                        } else {
                            start + c + 2 * (newlines + 1)
                        }
                    };
                    segs.extend(child_flat.segments.into_iter().map(|s| FlattenSegment {
                        child_path: s.child_path,
                        char_range: map_child_offset(s.char_range.start)..map_child_offset(s.char_range.end),
                    }));
                }
            }
            FlattenResult {
                text: out,
                segments: segs,
            }
        }
        BlockElement::Table { headers, rows } => {
            let mut lines = Vec::new();
            if !headers.is_empty() {
                let header_line = headers
                    .iter()
                    .map(|h| h.plain_text())
                    .collect::<Vec<_>>()
                    .join("\t");
                lines.push(header_line);
            }
            for row in rows {
                let row_line = row
                    .iter()
                    .map(|c| c.plain_text())
                    .collect::<Vec<_>>()
                    .join("\t");
                lines.push(row_line);
            }
            let text = lines.join("\n");
            let len = char_len(&text);
            FlattenResult {
                segments: if len > 0 {
                    vec![FlattenSegment {
                        child_path: path.to_string(),
                        char_range: 0..len,
                    }]
                } else {
                    Vec::new()
                },
                text,
            }
        }
        BlockElement::Rule => FlattenResult {
            text: String::new(),
            segments: Vec::new(),
        },
    }
}

/// A top-level Markdown block element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockElement {
    Heading {
        level: u8,
        content: InlineGroup,
    },
    Paragraph(InlineGroup),
    FencedCode {
        lang: Option<String>,
        code: String,
    },
    BlockQuote(Vec<BlockElement>),
    List {
        ordered: bool,
        items: Vec<ListItem>,
    },
    Table {
        headers: Vec<InlineGroup>,
        rows: Vec<Vec<InlineGroup>>,
    },
    Rule,
}

impl BlockElement {
    pub fn to_plain_text(&self) -> String {
        flatten_block(self, "0").text
    }
}

/// A fully parsed Markdown document with synchronized source anchors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedDocument {
    pub blocks: Vec<BlockElement>,
    pub anchors: Vec<SourceAnchor>,
}

impl ParsedDocument {
    pub fn to_plain_text(&self) -> String {
        let mut out = String::new();
        for block in &self.blocks {
            let text = block.to_plain_text();
            if !text.is_empty() {
                if !out.is_empty() && !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(&text);
            }
        }
        out
    }
    pub fn headings(&self) -> Vec<HeadingItem> {
        let mut list = Vec::new();
        for (idx, block) in self.blocks.iter().enumerate() {
            if let BlockElement::Heading { level, content } = block {
                list.push(HeadingItem {
                    level: *level,
                    text: content.plain_text(),
                    block_index: idx,
                });
            }
        }
        list
    }

    pub fn extract_plain_text(&self, start_block: usize, end_block: usize) -> String {
        let min = start_block.min(end_block);
        let max = start_block.max(end_block);
        self.extract_plain_text_char_range(min, 0, max, usize::MAX)
    }

    pub fn extract_plain_text_char_range(
        &self,
        start_block: usize,
        start_char: usize,
        end_block: usize,
        end_char: usize,
    ) -> String {
        if self.blocks.is_empty() {
            return String::new();
        }

        // Support reverse dragging: normalize so (from_b, from_c) <= (to_b, to_c)
        let ((from_b, from_c), (to_b, to_c)) = if (start_block, start_char) <= (end_block, end_char) {
            ((start_block, start_char), (end_block, end_char))
        } else {
            ((end_block, end_char), (start_block, start_char))
        };

        if from_b >= self.blocks.len() {
            return String::new();
        }
        let to_b = to_b.min(self.blocks.len() - 1);
        if from_b > to_b {
            return String::new();
        }

        let mut non_empty: Vec<(&BlockElement, String)> = Vec::new();
        for (idx, block) in self.blocks.iter().enumerate().skip(from_b).take(to_b - from_b + 1) {
            let full_text = block.to_plain_text();
            let char_len = full_text.chars().count();

            let (c_start, c_end) = if from_b == to_b {
                (from_c.min(char_len), to_c.min(char_len))
            } else if idx == from_b {
                (from_c.min(char_len), char_len)
            } else if idx == to_b {
                (0, to_c.min(char_len))
            } else {
                (0, char_len)
            };

            let c_start = c_start.min(c_end);
            let text: String = full_text.chars().skip(c_start).take(c_end - c_start).collect();
            if !text.is_empty() {
                non_empty.push((block, text));
            }
        }

        if non_empty.is_empty() {
            return String::new();
        }

        let mut result = String::new();
        for (i, (block, text)) in non_empty.iter().enumerate() {
            if i > 0 {
                let prev_block = non_empty[i - 1].0;
                let sep = if matches!(prev_block, BlockElement::List { .. })
                    && matches!(block, BlockElement::List { .. })
                {
                    if result.ends_with('\n') { "" } else { "\n" }
                } else if result.ends_with('\n') {
                    "\n"
                } else {
                    "\n\n"
                };
                result.push_str(sep);
            }
            result.push_str(text);
        }
        result
    }
}

/// Fast line index mapping byte offsets to 1-based line numbers.
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (idx, &b) in text.as_bytes().iter().enumerate() {
            if b == b'\n' {
                line_starts.push(idx + 1);
            }
        }
        Self { line_starts }
    }

    pub fn line_number(&self, byte_offset: usize) -> usize {
        match self.line_starts.binary_search(&byte_offset) {
            Ok(idx) => idx + 1,
            Err(idx) => idx.max(1),
        }
    }
}

/// Parse Markdown text into AST `BlockElement`s and `SourceAnchor`s.
pub fn parse_markdown(source: &str) -> ParsedDocument {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let line_index = LineIndex::new(source);
    let mut blocks = Vec::new();
    let mut anchors = Vec::new();

    let parser = Parser::new_ext(source, options).into_offset_iter();
    let mut iter = parser.peekable();

    while let Some((ev, range)) = iter.next() {
        let start_line = line_index.line_number(range.start);
        if let Some((block, end_offset)) = parse_single_block(ev, range, &mut iter) {
            let end_line = line_index
                .line_number(end_offset.saturating_sub(1))
                .max(start_line);
            let block_index = blocks.len();
            blocks.push(block);
            anchors.push(SourceAnchor::new(block_index, start_line, end_line));
        }
    }

    ParsedDocument { blocks, anchors }
}

fn is_inline_start(ev: &Event) -> bool {
    matches!(
        ev,
        Event::Text(_)
            | Event::Code(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::FootnoteReference(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::Start(
                Tag::Strong
                    | Tag::Emphasis
                    | Tag::Strikethrough
                    | Tag::Link { .. }
                    | Tag::Image { .. }
            )
    )
}

fn parse_single_block<'a, I>(
    ev: Event<'a>,
    range: std::ops::Range<usize>,
    iter: &mut std::iter::Peekable<I>,
) -> Option<(BlockElement, usize)>
where
    I: Iterator<Item = (Event<'a>, std::ops::Range<usize>)>,
{
    match ev {
        Event::Start(Tag::Heading { level, .. }) => {
            let lvl = heading_level_to_u8(level);
            let (inlines, end_range) =
                collect_inlines_until(iter, &mut |e| matches!(e, Event::End(TagEnd::Heading(_))));
            let end_offset = end_range.map(|r| r.end).unwrap_or(range.end);
            Some((
                BlockElement::Heading {
                    level: lvl,
                    content: inlines,
                },
                end_offset,
            ))
        }
        Event::Start(Tag::Paragraph) => {
            let (inlines, end_range) =
                collect_inlines_until(iter, &mut |e| matches!(e, Event::End(TagEnd::Paragraph)));
            let end_offset = end_range.map(|r| r.end).unwrap_or(range.end);
            Some((BlockElement::Paragraph(inlines), end_offset))
        }
        Event::Start(Tag::CodeBlock(kind)) => {
            let lang = match kind {
                CodeBlockKind::Fenced(l) => {
                    let s = l.trim().to_string();
                    if s.is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                }
                CodeBlockKind::Indented => None,
            };

            let mut code = String::new();
            let mut end_offset = range.end;
            while let Some((sub_ev, sub_range)) = iter.peek() {
                match sub_ev {
                    Event::End(TagEnd::CodeBlock) => {
                        end_offset = sub_range.end;
                        iter.next();
                        break;
                    }
                    Event::Text(t) => {
                        code.push_str(t);
                        end_offset = sub_range.end;
                        iter.next();
                    }
                    _ => {
                        iter.next();
                    }
                }
            }
            Some((BlockElement::FencedCode { lang, code }, end_offset))
        }
        Event::Start(Tag::List(first_num)) => {
            let ordered = first_num.is_some();
            let (items, end_offset) = collect_list_items(iter, range.end);
            Some((BlockElement::List { ordered, items }, end_offset))
        }
        Event::Start(Tag::Table(_)) => {
            let (headers, rows, end_offset) = collect_table(iter, range.end);
            Some((BlockElement::Table { headers, rows }, end_offset))
        }
        Event::Start(Tag::BlockQuote(_)) => {
            let mut child_blocks = Vec::new();
            let mut quote_end = range.end;
            while let Some((sub_ev, sub_range)) = iter.peek() {
                if matches!(sub_ev, Event::End(TagEnd::BlockQuote(_))) {
                    quote_end = sub_range.end;
                    iter.next();
                    break;
                }
                let (next_ev, next_r) = iter.next().unwrap();
                quote_end = next_r.end;
                if let Some((b, end_off)) = parse_single_block(next_ev, next_r, iter) {
                    quote_end = end_off;
                    child_blocks.push(b);
                }
            }
            Some((BlockElement::BlockQuote(child_blocks), quote_end))
        }
        Event::Rule => Some((BlockElement::Rule, range.end)),
        _ if is_inline_start(&ev) => {
            let mut inlines = InlineGroup::new();
            let mut end_offset = range.end;
            handle_inline_event(ev, iter, &mut inlines);
            while let Some((sub_ev, _)) = iter.peek() {
                if is_inline_start(sub_ev) {
                    let (next_ev, next_r) = iter.next().unwrap();
                    end_offset = next_r.end;
                    handle_inline_event(next_ev, iter, &mut inlines);
                } else {
                    break;
                }
            }
            if inlines.0.is_empty() {
                None
            } else {
                Some((BlockElement::Paragraph(inlines), end_offset))
            }
        }
        _ => None,
    }
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn handle_inline_event<'a, I>(
    ev: Event<'a>,
    iter: &mut std::iter::Peekable<I>,
    inlines: &mut InlineGroup,
) -> bool
where
    I: Iterator<Item = (Event<'a>, std::ops::Range<usize>)>,
{
    match ev {
        Event::Text(t) => {
            inlines.push(InlineItem::Text(t.into_string()));
            true
        }
        Event::Code(c) => {
            inlines.push(InlineItem::Code(c.into_string()));
            true
        }
        Event::Start(Tag::Strong) => {
            let (sub, _) = collect_inlines_until(iter, &mut |e| matches!(e, Event::End(TagEnd::Strong)));
            let text = sub.0.into_iter().map(inline_to_string).collect();
            inlines.push(InlineItem::Bold(text));
            true
        }
        Event::Start(Tag::Emphasis) => {
            let (sub, _) = collect_inlines_until(iter, &mut |e| matches!(e, Event::End(TagEnd::Emphasis)));
            let text = sub.0.into_iter().map(inline_to_string).collect();
            inlines.push(InlineItem::Italic(text));
            true
        }
        Event::Start(Tag::Strikethrough) => {
            let (sub, _) = collect_inlines_until(iter, &mut |e| matches!(e, Event::End(TagEnd::Strikethrough)));
            let text = sub.0.into_iter().map(inline_to_string).collect();
            inlines.push(InlineItem::Strikethrough(text));
            true
        }
        Event::Start(Tag::Link { dest_url, title, .. }) => {
            let (sub, _) = collect_inlines_until(iter, &mut |e| matches!(e, Event::End(TagEnd::Link)));
            let text = sub.0.into_iter().map(inline_to_string).collect();
            let title = if title.is_empty() { None } else { Some(title.into_string()) };
            inlines.push(InlineItem::Link {
                text,
                url: dest_url.into_string(),
                title,
            });
            true
        }
        Event::Start(Tag::Image { dest_url, title, .. }) => {
            let (sub, _) = collect_inlines_until(iter, &mut |e| matches!(e, Event::End(TagEnd::Image)));
            let alt = sub.0.into_iter().map(inline_to_string).collect();
            let title = if title.is_empty() { None } else { Some(title.into_string()) };
            inlines.push(InlineItem::Image {
                alt,
                url: dest_url.into_string(),
                title,
            });
            true
        }
        Event::SoftBreak | Event::HardBreak => {
            inlines.push(InlineItem::Text(" ".to_string()));
            true
        }
        _ => false,
    }
}

fn collect_inlines_until<'a, I>(
    iter: &mut std::iter::Peekable<I>,
    is_end: &mut dyn FnMut(&Event<'_>) -> bool,
) -> (InlineGroup, Option<std::ops::Range<usize>>)
where
    I: Iterator<Item = (Event<'a>, std::ops::Range<usize>)>,
{
    let mut inlines = InlineGroup::new();
    let mut last_range = None;

    while let Some((ev, range)) = iter.next() {
        if is_end(&ev) {
            return (inlines, Some(range));
        }
        last_range = Some(range);
        handle_inline_event(ev, iter, &mut inlines);
    }

    (inlines, last_range)
}

fn inline_to_string(item: InlineItem) -> String {
    match item {
        InlineItem::Text(s)
        | InlineItem::Bold(s)
        | InlineItem::Italic(s)
        | InlineItem::Strikethrough(s)
        | InlineItem::Code(s) => s,
        InlineItem::Link { text, .. } => text,
        InlineItem::Image { alt, .. } => alt,
    }
}

fn collect_list_items<'a, I>(
    iter: &mut std::iter::Peekable<I>,
    default_end: usize,
) -> (Vec<ListItem>, usize)
where
    I: Iterator<Item = (Event<'a>, std::ops::Range<usize>)>,
{
    let mut items = Vec::new();
    let mut end_offset = default_end;

    while let Some((ev, range)) = iter.next() {
        match ev {
            Event::End(TagEnd::List(_)) => {
                end_offset = range.end;
                break;
            }
            Event::Start(Tag::Item) => {
                let mut checked = None;
                let mut body = Vec::new();
                let mut item_end = range.end;

                while let Some((sub_ev, sub_range)) = iter.peek() {
                    if matches!(sub_ev, Event::End(TagEnd::Item)) {
                        item_end = sub_range.end;
                        iter.next();
                        break;
                    }
                    if let Event::TaskListMarker(chk) = sub_ev {
                        checked = Some(*chk);
                        item_end = sub_range.end;
                        iter.next();
                        continue;
                    }
                    let (next_ev, next_r) = iter.next().unwrap();
                    item_end = next_r.end;
                    if let Some((b, end_off)) = parse_single_block(next_ev, next_r, iter) {
                        item_end = end_off;
                        body.push(b);
                    }
                }
                items.push(ListItem { checked, body });
                end_offset = item_end;
            }
            _ => {}
        }
    }

    (items, end_offset)
}

fn collect_table<'a, I>(
    iter: &mut std::iter::Peekable<I>,
    default_end: usize,
) -> (Vec<InlineGroup>, Vec<Vec<InlineGroup>>, usize)
where
    I: Iterator<Item = (Event<'a>, std::ops::Range<usize>)>,

{
    let mut headers = Vec::new();
    let mut rows = Vec::new();
    let mut end_offset = default_end;
    let mut in_head = false;
    let mut current_row = Vec::new();

    while let Some((ev, range)) = iter.next() {
        match ev {
            Event::End(TagEnd::Table) => {
                end_offset = range.end;
                break;
            }
            Event::Start(Tag::TableHead) => in_head = true,
            Event::End(TagEnd::TableHead) => in_head = false,
            Event::Start(Tag::TableRow) => current_row.clear(),
            Event::End(TagEnd::TableRow) => {
                if !in_head {
                    rows.push(std::mem::take(&mut current_row));
                }
            }
            Event::Start(Tag::TableCell) => {
                let (cell_inlines, _) = collect_inlines_until(iter, &mut |e| {
                    matches!(e, Event::End(TagEnd::TableCell))
                });
                if in_head {
                    headers.push(cell_inlines);
                } else {
                    current_row.push(cell_inlines);
                }
            }
            _ => {}
        }
    }

    (headers, rows, end_offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinned_expectations_and_flatten_rules() {
        // 1. > first\n>\n> second => first\nsecond
        let doc1 = parse_markdown("> first\n>\n> second");
        assert_eq!(doc1.blocks.len(), 1);
        assert_eq!(doc1.blocks[0].to_plain_text(), "first\nsecond");
        assert_eq!(doc1.to_plain_text(), "first\nsecond");

        // 2. - first para\n\n  second para => - first para\n  second para
        let doc2 = parse_markdown("- first para\n\n  second para");
        assert_eq!(doc2.blocks.len(), 1);
        assert_eq!(doc2.blocks[0].to_plain_text(), "- first para\n  second para");

        // 3. > ```rust\n> fn a() {}\n> ```\n> next => fn a() {}\nnext
        let doc3 = parse_markdown("> ```rust\n> fn a() {}\n> ```\n> next");
        assert_eq!(doc3.blocks.len(), 1);
        assert_eq!(doc3.blocks[0].to_plain_text(), "fn a() {}\nnext");

        // 4. List item with para -> code -> para => - before\n  fn a() {}\n  after
        let doc4 = parse_markdown("- before\n  ```rust\n  fn a() {}\n  ```\n  after");
        assert_eq!(doc4.blocks.len(), 1);
        assert_eq!(doc4.blocks[0].to_plain_text(), "- before\n  fn a() {}\n  after");

        // 5. Code with blank line in list: - x\n  a\n  \n  b\n
        let doc5 = parse_markdown("- x\n  ```\n  a\n\n  b\n  ```");
        assert_eq!(doc5.blocks.len(), 1);
        assert_eq!(doc5.blocks[0].to_plain_text(), "- x\n  a\n  \n  b\n");

        // 6. Code as first child in list: - fn a() {}\n  let b = 1;\n
        let doc6 = parse_markdown("- ```\n  fn a() {}\n  let b = 1;\n  ```");
        assert_eq!(doc6.blocks.len(), 1);
        assert_eq!(doc6.blocks[0].to_plain_text(), "- fn a() {}\n  let b = 1;\n");

        // 7. Adjacent code blocks in quote: a\nb\nafter
        let doc7 = parse_markdown("> ```\n> a\n> ```\n> ```\n> b\n> ```\n> after");
        assert_eq!(doc7.blocks.len(), 1);
        assert_eq!(doc7.blocks[0].to_plain_text(), "a\nb\nafter");
    }

    #[test]
    fn test_flatten_block_segments_mapping() {
        let doc = parse_markdown("- before\n  ```rust\n  fn a() {}\n  ```\n  after");
        let flat = flatten_block(&doc.blocks[0], "0");
        assert_eq!(flat.text, "- before\n  fn a() {}\n  after");
        assert!(!flat.segments.is_empty());
        // Verify every segment char range is within text char boundaries
        let total_chars = flat.text.chars().count();
        for seg in &flat.segments {
            assert!(seg.char_range.start <= seg.char_range.end);
            assert!(seg.char_range.end <= total_chars, "segment {:?} exceeds total chars {}", seg, total_chars);
        }
    }

    #[test]
    fn test_nested_list_body_structure() {
        let doc = parse_markdown("- parent\n  - child 1\n  - child 2");
        assert_eq!(doc.blocks.len(), 1);
        match &doc.blocks[0] {
            BlockElement::List { items, .. } => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].body.len(), 2);
                match &items[0].body[0] {
                    BlockElement::Paragraph(p) => assert_eq!(p.plain_text(), "parent"),
                    _ => panic!("expected paragraph"),
                }
                match &items[0].body[1] {
                    BlockElement::List { items: child_items, .. } => {
                        assert_eq!(child_items.len(), 2);
                    }
                    _ => panic!("expected child list"),
                }
            }
            _ => panic!("expected list"),
        }
    }
}
