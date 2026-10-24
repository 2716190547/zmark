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
                let (items, end_offset) = collect_list_items(&mut iter, range.end);
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

        match ev {
            Event::Text(t) => inlines.push(InlineItem::Text(t.into_string())),
            Event::Code(c) => inlines.push(InlineItem::Code(c.into_string())),
            Event::Start(Tag::Strong) => {
                let (sub, _) = collect_inlines_until(iter, &mut |e| matches!(e, Event::End(TagEnd::Strong)));
                let text = sub.0.into_iter().map(inline_to_string).collect();
                inlines.push(InlineItem::Bold(text));
            }
            Event::Start(Tag::Emphasis) => {
                let (sub, _) = collect_inlines_until(iter, &mut |e| matches!(e, Event::End(TagEnd::Emphasis)));
                let text = sub.0.into_iter().map(inline_to_string).collect();
                inlines.push(InlineItem::Italic(text));
            }
            Event::Start(Tag::Strikethrough) => {
                let (sub, _) = collect_inlines_until(iter, &mut |e| matches!(e, Event::End(TagEnd::Strikethrough)));
                let text = sub.0.into_iter().map(inline_to_string).collect();
                inlines.push(InlineItem::Strikethrough(text));
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
            }
            Event::SoftBreak | Event::HardBreak => {
                inlines.push(InlineItem::Text(" ".to_string()));
            }
            _ => {}
        }
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
                let mut content = InlineGroup::new();
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
                        Event::Text(t) => {
                            content.push(InlineItem::Text(t.clone().into_string()));
                            end_offset = sub_range.end;
                            iter.next();
                        }
                        Event::Code(c) => {
                            content.push(InlineItem::Code(c.clone().into_string()));
                            end_offset = sub_range.end;
                            iter.next();
                        }
                        Event::Start(Tag::Paragraph) => {
                            iter.next();
                        }
                        Event::End(TagEnd::Paragraph) => {
                            iter.next();
                        }
                        _ => {
                            iter.next();
                        }
                    }
                }
                items.push(ListItem { checked, content });
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
