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
    pub content: InlineGroup,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_list: Option<Box<BlockElement>>,
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
        match self {
            BlockElement::Heading { content, .. } => content.plain_text(),
            BlockElement::Paragraph(content) => content.plain_text(),
            BlockElement::FencedCode { code, .. } => code.clone(),
            BlockElement::BlockQuote(elements) => elements
                .iter()
                .map(|el| el.to_plain_text())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
            BlockElement::List { ordered, items } => {
                let mut lines = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let prefix = if let Some(checked) = item.checked {
                        format!("- [{}] ", if checked { "x" } else { " " })
                    } else if *ordered {
                        format!("{}. ", i + 1)
                    } else {
                        "- ".to_string()
                    };
                    let mut item_str = format!("{}{}", prefix, item.content.plain_text());
                    if let Some(sub) = &item.sub_list {
                        let sub_text = sub.to_plain_text();
                        if !sub_text.is_empty() {
                            for line in sub_text.lines() {
                                item_str.push('\n');
                                item_str.push_str("  ");
                                item_str.push_str(line);
                            }
                        }
                    }
                    lines.push(item_str);
                }
                lines.join("\n")
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
                lines.join("\n")
            }
            BlockElement::Rule => String::new(),
        }
    }
}

/// A fully parsed Markdown document with synchronized source anchors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedDocument {
    pub blocks: Vec<BlockElement>,
    pub anchors: Vec<SourceAnchor>,
}

impl ParsedDocument {
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

    while let Some((event, range)) = iter.next() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let start_line = line_index.line_number(range.start);
                let lvl = heading_level_to_u8(level);
                let (inlines, end_range) = collect_inlines_until(&mut iter, &mut |e| {
                    matches!(e, Event::End(TagEnd::Heading(_)))
                });
                let end_offset = end_range.map(|r| r.end).unwrap_or(range.end);
                let end_line = line_index
                    .line_number(end_offset.saturating_sub(1))
                    .max(start_line);

                let block_index = blocks.len();
                blocks.push(BlockElement::Heading {
                    level: lvl,
                    content: inlines,
                });
                anchors.push(SourceAnchor::new(block_index, start_line, end_line));
            }
            Event::Start(Tag::Paragraph) => {
                let start_line = line_index.line_number(range.start);
                let (inlines, end_range) = collect_inlines_until(&mut iter, &mut |e| {
                    matches!(e, Event::End(TagEnd::Paragraph))
                });
                let end_offset = end_range.map(|r| r.end).unwrap_or(range.end);
                let end_line = line_index
                    .line_number(end_offset.saturating_sub(1))
                    .max(start_line);

                let block_index = blocks.len();
                blocks.push(BlockElement::Paragraph(inlines));
                anchors.push(SourceAnchor::new(block_index, start_line, end_line));
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let start_line = line_index.line_number(range.start);
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

                let end_line = line_index
                    .line_number(end_offset.saturating_sub(1))
                    .max(start_line);
                let block_index = blocks.len();
                blocks.push(BlockElement::FencedCode { lang, code });
                anchors.push(SourceAnchor::new(block_index, start_line, end_line));
            }
            Event::Start(Tag::List(first_num)) => {
                let start_line = line_index.line_number(range.start);
                let ordered = first_num.is_some();
                let (items, end_offset) = collect_list_items(&mut iter, range.end, 1);
                let end_line = line_index
                    .line_number(end_offset.saturating_sub(1))
                    .max(start_line);

                let block_index = blocks.len();
                blocks.push(BlockElement::List { ordered, items });
                anchors.push(SourceAnchor::new(block_index, start_line, end_line));
            }
            Event::Start(Tag::Table(_)) => {
                let start_line = line_index.line_number(range.start);
                let (headers, rows, end_offset) = collect_table(&mut iter, range.end);
                let end_line = line_index
                    .line_number(end_offset.saturating_sub(1))
                    .max(start_line);

                let block_index = blocks.len();
                blocks.push(BlockElement::Table { headers, rows });
                anchors.push(SourceAnchor::new(block_index, start_line, end_line));
            }
            Event::Start(Tag::BlockQuote(_)) => {
                let start_line = line_index.line_number(range.start);
                let mut inner_text = String::new();
                let mut end_offset = range.end;
                let mut depth = 1;
                while let Some((sub_ev, sub_range)) = iter.next() {
                    match sub_ev {
                        Event::Start(Tag::BlockQuote(_)) => depth += 1,
                        Event::End(TagEnd::BlockQuote(_)) => {
                            depth -= 1;
                            if depth == 0 {
                                end_offset = sub_range.end;
                                break;
                            }
                        }
                        Event::Text(t) => inner_text.push_str(&t),
                        _ => {}
                    }
                }
                let end_line = line_index
                    .line_number(end_offset.saturating_sub(1))
                    .max(start_line);
                let block_index = blocks.len();
                blocks.push(BlockElement::BlockQuote(vec![BlockElement::Paragraph(
                    InlineGroup(vec![InlineItem::Text(inner_text.trim().to_string())]),
                )]));
                anchors.push(SourceAnchor::new(block_index, start_line, end_line));
            }
            Event::Rule => {
                let line = line_index.line_number(range.start);
                let block_index = blocks.len();
                blocks.push(BlockElement::Rule);
                anchors.push(SourceAnchor::new(block_index, line, line));
            }
            _ => {}
        }
    }

    ParsedDocument { blocks, anchors }
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
    depth: usize,
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
                let mut content = InlineGroup::new();
                let mut sub_list = None;

                while let Some((sub_ev, sub_range)) = iter.peek() {
                    match sub_ev {
                        Event::End(TagEnd::Item) => {
                            end_offset = sub_range.end;
                            iter.next();
                            break;
                        }
                        Event::TaskListMarker(chk) => {
                            checked = Some(*chk);
                            iter.next();
                        }
                        Event::Start(Tag::List(first_num)) => {
                            let ordered = first_num.is_some();
                            let start_range = sub_range.clone();
                            iter.next();
                            if depth < 8 {
                                let (sub_items, sub_end) = collect_list_items(iter, start_range.end, depth + 1);
                                end_offset = sub_end;
                                sub_list = Some(Box::new(BlockElement::List {
                                    ordered,
                                    items: sub_items,
                                }));
                            } else {
                                let mut d = 1;
                                while let Some((deep_ev, _)) = iter.next() {
                                    match deep_ev {
                                        Event::Start(Tag::List(_)) => d += 1,
                                        Event::End(TagEnd::List(_)) => {
                                            d -= 1;
                                            if d == 0 {
                                                break;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) => {
                            iter.next();
                        }
                        _ => {
                            let (item_ev, r) = iter.next().unwrap();
                            end_offset = r.end;
                            handle_inline_event(item_ev, iter, &mut content);
                        }
                    }
                }
                items.push(ListItem { checked, content, sub_list });
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
