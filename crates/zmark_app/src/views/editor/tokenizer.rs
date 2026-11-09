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
    Function,
    Attribute,
    StringLiteral,
    Number,
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

pub fn spans_to_code_tokens(
    line: &str,
    spans: &[zmark_core::syntax::SyntaxSpan],
) -> Vec<MarkdownToken> {
    if line.is_empty() {
        return vec![MarkdownToken {
            text: " ".to_string(),
            kind: MarkdownTokenKind::CodeText,
        }];
    }
    if spans.is_empty() {
        return vec![MarkdownToken {
            text: line.to_string(),
            kind: MarkdownTokenKind::CodeText,
        }];
    }

    let mut tokens = Vec::with_capacity(spans.len());
    for span in spans {
        let start = span.range.start.min(line.len());
        let end = span.range.end.min(line.len());
        if start >= end {
            continue;
        }
        let text = line[start..end].to_string();
        let kind = match span.role {
            zmark_core::syntax::SyntaxRole::Plain => MarkdownTokenKind::CodeText,
            zmark_core::syntax::SyntaxRole::Keyword => MarkdownTokenKind::Keyword,
            zmark_core::syntax::SyntaxRole::Function => MarkdownTokenKind::Function,
            zmark_core::syntax::SyntaxRole::Attribute => MarkdownTokenKind::Attribute,
            zmark_core::syntax::SyntaxRole::String => MarkdownTokenKind::StringLiteral,
            zmark_core::syntax::SyntaxRole::Number => MarkdownTokenKind::Number,
            zmark_core::syntax::SyntaxRole::Comment => MarkdownTokenKind::Comment,
        };
        tokens.push(MarkdownToken { text, kind });
    }

    if tokens.is_empty() {
        tokens.push(MarkdownToken {
            text: line.to_string(),
            kind: MarkdownTokenKind::CodeText,
        });
    }
    tokens
}
