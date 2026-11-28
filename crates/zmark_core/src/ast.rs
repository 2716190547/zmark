use std::sync::Arc;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

use crate::anchor::SourceAnchor;
use crate::syntax::{resolve_canonical_language, CodeKey};

/// Line mapping for code block line synchronization and validation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeLineMapping {
    pub payload_range: std::ops::Range<usize>, // byte range in canonical payload
    pub source_line_idx: usize,                // 0-based line index in source
    pub prefix_len: usize,                     // byte length of stripped prefix (e.g. `> ` or `  `)
    pub content_len: usize,                    // byte length of code content on source line
    pub eol_len: usize,                        // 1 for \n, 2 for \r\n, 0 for EOF
    pub is_synthetic_lf: bool,                 // true if trailing LF was synthesized
}

/// A parsed code block description and line mapping in a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCodeBlock {
    pub key: CodeKey,
    pub raw_info: Option<String>,
    pub fence_open_line: usize,
    pub fence_close_line: Option<usize>,
    pub lines: Vec<CodeLineMapping>,
    pub child_path: String,
    pub is_valid: bool,
}

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
        #[serde(default)]
        source_range: Option<std::ops::Range<usize>>,
    },
    /// Explicit inline HTML break (`<br>` / `<br/>`).
    LineBreak,
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
                InlineItem::LineBreak => out.push('\n'),
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
    /// Source byte range of the `[ ]` / `[x]` task marker, when this is a task item.
    #[serde(default)]
    pub marker: Option<std::ops::Range<usize>>,
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
                text: code.to_string(),
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
        BlockElement::TableOfContents | BlockElement::Spacer { .. } | BlockElement::Rule => FlattenResult {
            text: String::new(),
            segments: Vec::new(),
        },
    }
}

mod arc_str_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    pub fn serialize<S>(val: &Arc<str>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(val)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Arc::from(s))
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
        #[serde(with = "arc_str_serde")]
        code: Arc<str>,
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
    /// Standalone `[toc]` / `[[toc]]` marker rendered as an interactive document outline card.
    TableOfContents,
    /// Standalone HTML line break block (`<br>` / `<br/>`) rendered as vertical breathing room.
    Spacer {
        lines: u8,
    },
    Rule,
}

impl BlockElement {
    pub fn to_plain_text(&self) -> String {
        flatten_block(self, "0").text
    }
}

/// A fully parsed Markdown document with synchronized source anchors.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedDocument {
    pub blocks: Vec<BlockElement>,
    pub anchors: Vec<SourceAnchor>,
    #[serde(skip, default)]
    pub code_blocks: Arc<Vec<ParsedCodeBlock>>,
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

    pub fn line_idx(&self, byte_offset: usize) -> usize {
        match self.line_starts.binary_search(&byte_offset) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        }
    }

    pub fn line_str<'a>(&self, line_idx: usize, text: &'a str) -> Option<&'a str> {
        if line_idx >= self.line_starts.len() {
            return None;
        }
        let start = self.line_starts[line_idx];
        if start > text.len() {
            return None;
        }
        let end = if line_idx + 1 < self.line_starts.len() {
            self.line_starts[line_idx + 1]
        } else {
            text.len()
        };
        Some(&text[start..end])
    }
}

/// Parse Markdown text into AST `BlockElement`s, `SourceAnchor`s, and `ParsedCodeBlock`s.
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
    let mut code_blocks = Vec::new();

    let parser = Parser::new_ext(source, options).into_offset_iter();
    let mut iter = parser.peekable();
    let mut block_idx = 0;

    while let Some((ev, range)) = iter.next() {
        let start_line = line_index.line_number(range.start);
        let path = block_idx.to_string();
        if let Some((block, end_offset)) = parse_single_block(
            ev,
            range,
            &mut iter,
            &line_index,
            source,
            &path,
            &mut code_blocks,
        ) {
            let end_line = line_index
                .line_number(end_offset.saturating_sub(1))
                .max(start_line);
            blocks.push(block);
            anchors.push(SourceAnchor::new(block_idx, start_line, end_line));
            block_idx += 1;
        }
    }

    ParsedDocument {
        blocks,
        anchors,
        code_blocks: Arc::new(code_blocks),
    }
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

fn is_toc_marker(inlines: &InlineGroup) -> bool {
    if !inlines.0.iter().all(|item| matches!(item, InlineItem::Text(_))) {
        return false;
    }
    let text = inlines.plain_text();
    let trimmed = text.trim();
    trimmed.eq_ignore_ascii_case("[toc]") || trimmed.eq_ignore_ascii_case("[[toc]]")
}

fn paragraph_or_toc(inlines: InlineGroup, end_offset: usize) -> (BlockElement, usize) {
    if is_toc_marker(&inlines) {
        (BlockElement::TableOfContents, end_offset)
    } else {
        (BlockElement::Paragraph(inlines), end_offset)
    }
}

/// Counts `<br>` / `<br/>` / `<br />` tags if `raw` consists exclusively of break tags and whitespace.
fn parse_html_break_count(raw: &str) -> Option<u8> {
    let mut rest = raw.trim();
    if rest.is_empty() {
        return None;
    }
    let mut count: u8 = 0;
    while !rest.is_empty() {
        if rest.len() < 4 || !rest.as_bytes()[..3].eq_ignore_ascii_case(b"<br") {
            return None;
        }
        let close_rel = rest[3..].find('>')?;
        let inner = rest[3..3 + close_rel].trim();
        if !inner.is_empty() && inner != "/" {
            return None;
        }
        count = count.saturating_add(1);
        rest = rest[3 + close_rel + 1..].trim_start();
    }
    (count > 0).then_some(count.min(4))
}

fn parse_single_block<'a, I>(
    ev: Event<'a>,
    range: std::ops::Range<usize>,
    iter: &mut std::iter::Peekable<I>,
    line_index: &LineIndex,
    source: &str,
    path: &str,
    code_blocks: &mut Vec<ParsedCodeBlock>,
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
            Some(paragraph_or_toc(inlines, end_offset))
        }
        Event::Start(Tag::HtmlBlock) => {
            let mut raw = String::new();
            let mut end_offset = range.end;
            while let Some((sub_ev, sub_range)) = iter.peek() {
                match sub_ev {
                    Event::End(TagEnd::HtmlBlock) => {
                        end_offset = sub_range.end;
                        iter.next();
                        break;
                    }
                    Event::Html(t) | Event::Text(t) => {
                        raw.push_str(t);
                        end_offset = sub_range.end;
                        iter.next();
                    }
                    _ => {
                        end_offset = sub_range.end;
                        iter.next();
                    }
                }
            }
            parse_html_break_count(&raw).map(|lines| (BlockElement::Spacer { lines }, end_offset))
        }
        Event::Html(t) => {
            parse_html_break_count(&t).map(|lines| (BlockElement::Spacer { lines }, range.end))
        }
        Event::Start(Tag::CodeBlock(kind)) => {
            let is_indented = matches!(kind, CodeBlockKind::Indented);
            let raw_info = match kind {
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

            let payload: Arc<str> = Arc::from(code);
            let canonical_lang = resolve_canonical_language(raw_info.as_deref());
            let key = CodeKey::new(canonical_lang, payload.clone());

            let start_line = line_index.line_idx(range.start);
            let fence_open_line = start_line;

            let mut payload_lines = Vec::new();
            let mut p_start = 0;
            for (idx, b) in payload.bytes().enumerate() {
                if b == b'\n' {
                    payload_lines.push((p_start..idx + 1, &payload[p_start..idx + 1]));
                    p_start = idx + 1;
                }
            }
            if p_start < payload.len() {
                payload_lines.push((p_start..payload.len(), &payload[p_start..payload.len()]));
            }

            let num_lines = payload_lines.len();
            let mut lines = Vec::with_capacity(num_lines);
            let mut is_valid = true;

            for (k, (payload_range, payload_line)) in payload_lines.into_iter().enumerate() {
                let source_line_idx = if is_indented {
                    start_line + k
                } else {
                    fence_open_line + 1 + k
                };
                let Some(source_line) = line_index.line_str(source_line_idx, source) else {
                    is_valid = false;
                    break;
                };

                let eol_len = if source_line.ends_with("\r\n") {
                    2
                } else if source_line.ends_with('\n') {
                    1
                } else {
                    0
                };

                let source_content = &source_line[..source_line.len() - eol_len];
                let payload_content = payload_line
                    .strip_suffix("\r\n")
                    .or_else(|| payload_line.strip_suffix('\n'))
                    .unwrap_or(payload_line);

                let (prefix_len, content_len, line_valid) = if payload_content.is_empty() {
                    (source_content.len(), 0, true)
                } else if is_indented {
                    if source_content.ends_with(payload_content) {
                        let c_len = payload_content.len();
                        let p_len = source_content.len() - c_len;
                        (p_len, c_len, true)
                    } else if let Some(idx) = source_content.find(payload_content) {
                        (idx, payload_content.len(), true)
                    } else {
                        (0, source_content.len().min(payload_content.len()), false)
                    }
                } else if source_content.ends_with(payload_content) {
                    let c_len = payload_content.len();
                    let p_len = source_content.len() - c_len;
                    (p_len, c_len, true)
                } else {
                    (0, source_content.len().min(payload_content.len()), false)
                };

                if !line_valid {
                    is_valid = false;
                }

                let is_synthetic_lf = payload_line.ends_with('\n') && eol_len == 0;

                lines.push(CodeLineMapping {
                    payload_range,
                    source_line_idx,
                    prefix_len,
                    content_len,
                    eol_len,
                    is_synthetic_lf,
                });
            }

            let fence_close_line = if is_indented {
                None
            } else {
                let candidate_close_line = fence_open_line + 1 + num_lines;
                if let Some(cand_str) = line_index.line_str(candidate_close_line, source) {
                    let cand_trimmed = cand_str.trim();
                    let cand_no_quote = cand_trimmed.trim_start_matches(|c: char| c == '>' || c.is_whitespace());
                    if cand_no_quote.starts_with("```") || cand_no_quote.starts_with("~~~") {
                        Some(candidate_close_line)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            code_blocks.push(ParsedCodeBlock {
                key,
                raw_info: raw_info.clone(),
                fence_open_line,
                fence_close_line,
                lines,
                child_path: path.to_string(),
                is_valid,
            });

            Some((BlockElement::FencedCode { lang: raw_info, code: payload }, end_offset))
        }
        Event::Start(Tag::List(first_num)) => {
            let ordered = first_num.is_some();
            let (items, end_offset) = collect_list_items(
                iter,
                range.end,
                line_index,
                source,
                path,
                code_blocks,
            );
            Some((BlockElement::List { ordered, items }, end_offset))
        }
        Event::Start(Tag::Table(_)) => {
            let (headers, rows, end_offset) = collect_table(iter, range.end);
            Some((BlockElement::Table { headers, rows }, end_offset))
        }
        Event::Start(Tag::BlockQuote(_)) => {
            let mut child_blocks = Vec::new();
            let mut quote_end = range.end;
            let mut child_idx = 0;
            while let Some((sub_ev, sub_range)) = iter.peek() {
                if matches!(sub_ev, Event::End(TagEnd::BlockQuote(_))) {
                    quote_end = sub_range.end;
                    iter.next();
                    break;
                }
                let (next_ev, next_r) = iter.next().unwrap();
                quote_end = next_r.end;
                let child_path = format!("{}.{}", path, child_idx);
                if let Some((b, end_off)) = parse_single_block(
                    next_ev,
                    next_r,
                    iter,
                    line_index,
                    source,
                    &child_path,
                    code_blocks,
                ) {
                    quote_end = end_off;
                    child_blocks.push(b);
                    child_idx += 1;
                }
            }
            Some((BlockElement::BlockQuote(child_blocks), quote_end))
        }
        Event::Rule => Some((BlockElement::Rule, range.end)),
        _ if is_inline_start(&ev) => {
            let mut inlines = InlineGroup::new();
            let mut end_offset = range.end;
            handle_inline_event(ev, range.clone(), iter, &mut inlines);
            while let Some((sub_ev, _)) = iter.peek() {
                if is_inline_start(sub_ev) {
                    let (next_ev, next_r) = iter.next().unwrap();
                    end_offset = next_r.end;
                    handle_inline_event(next_ev, next_r, iter, &mut inlines);
                } else {
                    break;
                }
            }
            if inlines.0.is_empty() {
                None
            } else {
                Some(paragraph_or_toc(inlines, end_offset))
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
    source_range: std::ops::Range<usize>,
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
                source_range: Some(source_range),
            });
            true
        }
        Event::SoftBreak | Event::HardBreak => {
            inlines.push(InlineItem::Text(" ".to_string()));
            true
        }
        Event::InlineHtml(html) | Event::Html(html) => {
            if let Some(count) = parse_html_break_count(&html) {
                for _ in 0..count {
                    inlines.push(InlineItem::LineBreak);
                }
                true
            } else {
                false
            }
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
        last_range = Some(range.clone());
        handle_inline_event(ev, range, iter, &mut inlines);
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
        InlineItem::LineBreak => "\n".to_string(),
    }
}

fn collect_list_items<'a, I>(
    iter: &mut std::iter::Peekable<I>,
    default_end: usize,
    line_index: &LineIndex,
    source: &str,
    parent_path: &str,
    code_blocks: &mut Vec<ParsedCodeBlock>,
) -> (Vec<ListItem>, usize)
where
    I: Iterator<Item = (Event<'a>, std::ops::Range<usize>)>,
{
    let mut items = Vec::new();
    let mut end_offset = default_end;
    let mut item_idx = 0;

    while let Some((ev, range)) = iter.next() {
        match ev {
            Event::End(TagEnd::List(_)) => {
                end_offset = range.end;
                break;
            }
            Event::Start(Tag::Item) => {
                let mut checked = None;
                let mut marker = None;
                let mut body = Vec::new();
                let mut item_end = range.end;
                let mut child_idx = 0;

                while let Some((sub_ev, sub_range)) = iter.peek() {
                    if matches!(sub_ev, Event::End(TagEnd::Item)) {
                        item_end = sub_range.end;
                        iter.next();
                        break;
                    }
                    if let Event::TaskListMarker(chk) = sub_ev {
                        checked = Some(*chk);
                        marker = Some(sub_range.clone());
                        item_end = sub_range.end;
                        iter.next();
                        continue;
                    }
                    let (next_ev, next_r) = iter.next().unwrap();
                    item_end = next_r.end;
                    let child_path = format!("{}.{}.{}", parent_path, item_idx, child_idx);
                    if let Some((b, end_off)) = parse_single_block(
                        next_ev,
                        next_r,
                        iter,
                        line_index,
                        source,
                        &child_path,
                        code_blocks,
                    ) {
                        item_end = end_off;
                        body.push(b);
                        child_idx += 1;
                    }
                }
                items.push(ListItem { checked, body, marker });
                item_idx += 1;
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

    #[test]
    fn test_code_block_mappings_top_level_and_nested() {
        // 1. Top-level Rust block
        let src1 = "```rust\nfn main() {\n    println!(\"hi\");\n}\n```";
        let doc1 = parse_markdown(src1);
        assert_eq!(doc1.code_blocks.len(), 1);
        let cb1 = &doc1.code_blocks[0];
        assert_eq!(cb1.child_path, "0");
        assert_eq!(cb1.raw_info.as_deref(), Some("rust"));
        assert_eq!(cb1.key.language, Some("Rust"));
        assert_eq!(&*cb1.key.payload, "fn main() {\n    println!(\"hi\");\n}\n");
        assert_eq!(cb1.fence_open_line, 0);
        assert_eq!(cb1.fence_close_line, Some(4));
        assert!(cb1.is_valid);
        assert_eq!(cb1.lines.len(), 3);

        // Check Arc sharing with AST BlockElement
        match &doc1.blocks[0] {
            BlockElement::FencedCode { code, .. } => {
                assert!(Arc::ptr_eq(code, &cb1.key.payload), "AST code and CodeKey must share the exact same Arc");
            }
            _ => panic!("Expected FencedCode"),
        }

        // Verify line mappings
        assert_eq!(cb1.lines[0].source_line_idx, 1);
        assert_eq!(cb1.lines[0].prefix_len, 0);
        assert_eq!(cb1.lines[0].content_len, 11); // "fn main() {"
        assert_eq!(cb1.lines[0].eol_len, 1);
        assert!(!cb1.lines[0].is_synthetic_lf);

        assert_eq!(cb1.lines[1].source_line_idx, 2);
        assert_eq!(cb1.lines[1].prefix_len, 0);
        assert_eq!(cb1.lines[1].content_len, 19); // "    println!(\"hi\");"
        assert_eq!(cb1.lines[1].eol_len, 1);

        assert_eq!(cb1.lines[2].source_line_idx, 3);
        assert_eq!(cb1.lines[2].content_len, 1); // "}"
        assert_eq!(cb1.lines[2].eol_len, 1);

        // 2. BlockQuote nested code block
        let src2 = "> ```rust\n> fn a() {}\n> ```";
        let doc2 = parse_markdown(src2);
        assert_eq!(doc2.code_blocks.len(), 1);
        let cb2 = &doc2.code_blocks[0];
        assert_eq!(cb2.child_path, "0.0");
        assert_eq!(cb2.fence_open_line, 0);
        assert_eq!(cb2.fence_close_line, Some(2));
        assert!(cb2.is_valid);
        assert_eq!(cb2.lines.len(), 1);
        assert_eq!(cb2.lines[0].source_line_idx, 1);
        assert_eq!(cb2.lines[0].prefix_len, 2); // "> "
        assert_eq!(cb2.lines[0].content_len, 9); // "fn a() {}"
        assert_eq!(cb2.lines[0].eol_len, 1);

        // 3. List nested code block
        let src3 = "- item\n  ```rust\n  fn b() {}\n  ```";
        let doc3 = parse_markdown(src3);
        assert_eq!(doc3.code_blocks.len(), 1);
        let cb3 = &doc3.code_blocks[0];
        assert_eq!(cb3.child_path, "0.0.1");
        assert_eq!(cb3.fence_open_line, 1);
        assert_eq!(cb3.fence_close_line, Some(3));
        assert!(cb3.is_valid);
        assert_eq!(cb3.lines.len(), 1);
        assert_eq!(cb3.lines[0].source_line_idx, 2);
        assert_eq!(cb3.lines[0].prefix_len, 2); // "  "
        assert_eq!(cb3.lines[0].content_len, 9); // "fn b() {}"

        // 4. CRLF line endings
        let src4 = "```python\r\nx = 1\r\ny = 2\r\n```\r\n";
        let doc4 = parse_markdown(src4);
        assert_eq!(doc4.code_blocks.len(), 1);
        let cb4 = &doc4.code_blocks[0];
        assert_eq!(cb4.key.language, Some("Python"));
        assert!(cb4.is_valid);
        assert_eq!(cb4.lines[0].eol_len, 2);
        assert_eq!(cb4.lines[1].eol_len, 2);

        // 5. Unclosed code block at EOF without trailing newline
        let src5 = "```rust\nfn unclosed() {}";
        let doc5 = parse_markdown(src5);
        assert_eq!(doc5.code_blocks.len(), 1);
        let cb5 = &doc5.code_blocks[0];
        assert_eq!(cb5.fence_open_line, 0);
        assert_eq!(cb5.fence_close_line, None);
        assert_eq!(cb5.lines.len(), 1);
        assert_eq!(cb5.lines[0].eol_len, 0);
        assert!(cb5.is_valid);

        // 6. Blank lines in code block
        let src6 = "```\na\n\nb\n```";
        let doc6 = parse_markdown(src6);
        assert_eq!(doc6.code_blocks.len(), 1);
        let cb6 = &doc6.code_blocks[0];
        assert!(cb6.key.is_plain());
        assert_eq!(cb6.lines.len(), 3);
        assert_eq!(cb6.lines[1].content_len, 0); // blank line
        assert!(cb6.is_valid);

        // 7. Empty code block
        let src7 = "```rust\n```";
        let doc7 = parse_markdown(src7);
        assert_eq!(doc7.code_blocks.len(), 1);
        let cb7 = &doc7.code_blocks[0];
        assert_eq!(cb7.fence_open_line, 0);
        assert_eq!(cb7.fence_close_line, Some(1));
        assert!(cb7.lines.is_empty());
        assert!(cb7.is_valid);

        // 8. Indented code block mapping
        let src8 = "    foo\n    bar\n";
        let doc8 = parse_markdown(src8);
        assert_eq!(doc8.code_blocks.len(), 1);
        let cb8 = &doc8.code_blocks[0];
        assert_eq!(cb8.fence_open_line, 0);
        assert_eq!(cb8.fence_close_line, None);
        assert!(cb8.key.is_plain());
        assert!(cb8.is_valid);
        assert_eq!(cb8.lines.len(), 2);
        assert_eq!(cb8.lines[0].source_line_idx, 0);
        assert_eq!(cb8.lines[0].prefix_len, 4);
        assert_eq!(cb8.lines[0].content_len, 3); // "foo"
        assert_eq!(cb8.lines[1].source_line_idx, 1);
        assert_eq!(cb8.lines[1].prefix_len, 4);
        assert_eq!(cb8.lines[1].content_len, 3); // "bar"
    }
}

/// Locate only the destination of an inline image, after verifying the parser's
/// occurrence and decoded destination. Reference-style and ambiguous syntax
/// deliberately return None so the caller never guesses a source edit.
pub fn image_source_target_range(source: &str, occurrence: std::ops::Range<usize>, expected_url: &str) -> Option<std::ops::Range<usize>> {
    let raw = source.get(occurrence.clone())?;
    let (dest_url, event_range) = Parser::new_ext(raw, Options::all()).into_offset_iter().find_map(|(event, range)| {
        if let Event::Start(Tag::Image { dest_url, .. }) = event { Some((dest_url, range)) } else { None }
    })?;
    if event_range != (0..raw.len()) || dest_url.as_ref() != expected_url { return None; }
    let bytes = raw.as_bytes();
    if !bytes.starts_with(b"![") { return None; }
    let mut i = 2;
    let mut depth = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() && bytes[i + 1].is_ascii_punctuation() => i += 2,
            b'[' => { depth += 1; i += 1; },
            b']' => { depth -= 1; i += 1; if depth == 0 { break; } },
            _ => i += 1,
        }
    }
    if depth != 0 || bytes.get(i) != Some(&b'(') { return None; }
    i += 1;
    while bytes.get(i).is_some_and(|b| b.is_ascii_whitespace() && *b != b'\n' && *b != b'\r') { i += 1; }
    let angle = bytes.get(i) == Some(&b'<');
    if angle { i += 1; }
    let start = i;
    let mut parens = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() && bytes[i + 1].is_ascii_punctuation() => i += 2,
            b'>' if angle => break,
            b'\n' | b'\r' => return None,
            b'(' if !angle => { parens += 1; i += 1; },
            b')' if !angle && parens == 0 => break,
            b')' if !angle => { parens -= 1; i += 1; },
            b if !angle && b.is_ascii_whitespace() && parens == 0 => break,
            _ => i += 1,
        }
    }
    if i == start || i >= bytes.len() || parens != 0 { return None; }
    if angle && bytes[i] != b'>' { return None; }
    let absolute = occurrence.start.checked_add(start)?..occurrence.start.checked_add(i)?;
    source.get(absolute.clone())?;
    Some(absolute)
}
