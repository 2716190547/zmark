use crate::ast::{parse_markdown, BlockElement, InlineGroup, InlineItem, ParsedDocument};

/// Export a parsed document into a standalone single-file HTML document.
pub fn export_html(doc: &ParsedDocument, title: &str) -> String {
    let mut body_html = String::new();
    for block in &doc.blocks {
        render_block(block, &mut body_html);
    }

    let escaped_title = escape_html(title);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
{css}
</style>
</head>
<body>
<div class="markdown-container">
{body}
</div>
</body>
</html>"#,
        title = escaped_title,
        css = EMBEDDED_TYPOGRAPHY_CSS,
        body = body_html,
    )
}

/// Convenience function to parse and export raw markdown directly.
pub fn export_markdown_to_html(source: &str, title: &str) -> String {
    let doc = parse_markdown(source);
    export_html(&doc, title)
}

fn render_block(block: &BlockElement, out: &mut String) {
    match block {
        BlockElement::Heading { level, content } => {
            out.push_str(&format!("<h{}>", level));
            render_inline_group(content, out);
            out.push_str(&format!("</h{}>\n", level));
        }
        BlockElement::Paragraph(content) => {
            out.push_str("<p>");
            render_inline_group(content, out);
            out.push_str("</p>\n");
        }
        BlockElement::FencedCode { lang, code } => {
            out.push_str("<pre><code");
            if let Some(l) = lang {
                out.push_str(&format!(" class=\"language-{}\"", escape_html(l)));
            }
            out.push_str(">");
            out.push_str(&escape_html(code));
            out.push_str("</code></pre>\n");
        }
        BlockElement::BlockQuote(blocks) => {
            out.push_str("<blockquote>\n");
            for b in blocks {
                render_block(b, out);
            }
            out.push_str("</blockquote>\n");
        }
        BlockElement::List { ordered, items } => {
            let tag = if *ordered { "ol" } else { "ul" };
            out.push_str(&format!("<{}>\n", tag));
            for item in items {
                out.push_str("<li>");
                if let Some(checked) = item.checked {
                    if checked {
                        out.push_str("<input type=\"checkbox\" checked disabled> ");
                    } else {
                        out.push_str("<input type=\"checkbox\" disabled> ");
                    }
                }
                render_inline_group(&item.content, out);
                out.push_str("</li>\n");
            }
            out.push_str(&format!("</{}>\n", tag));
        }
        BlockElement::Table { headers, rows } => {
            out.push_str("<table>\n<thead>\n<tr>\n");
            for h in headers {
                out.push_str("<th>");
                render_inline_group(h, out);
                out.push_str("</th>\n");
            }
            out.push_str("</tr>\n</thead>\n<tbody>\n");
            for row in rows {
                out.push_str("<tr>\n");
                for cell in row {
                    out.push_str("<td>");
                    render_inline_group(cell, out);
                    out.push_str("</td>\n");
                }
                out.push_str("</tr>\n");
            }
            out.push_str("</tbody>\n</table>\n");
        }
        BlockElement::Rule => {
            out.push_str("<hr>\n");
        }
    }
}

fn render_inline_group(group: &InlineGroup, out: &mut String) {
    for item in &group.0 {
        match item {
            InlineItem::Text(s) => out.push_str(&escape_html(s)),
            InlineItem::Bold(s) => {
                out.push_str("<strong>");
                out.push_str(&escape_html(s));
                out.push_str("</strong>");
            }
            InlineItem::Italic(s) => {
                out.push_str("<em>");
                out.push_str(&escape_html(s));
                out.push_str("</em>");
            }
            InlineItem::Strikethrough(s) => {
                out.push_str("<del>");
                out.push_str(&escape_html(s));
                out.push_str("</del>");
            }
            InlineItem::Code(s) => {
                out.push_str("<code>");
                out.push_str(&escape_html(s));
                out.push_str("</code>");
            }
            InlineItem::Link { text, url, title } => {
                out.push_str(&format!("<a href=\"{}\"", escape_html(url)));
                if let Some(t) = title {
                    out.push_str(&format!(" title=\"{}\"", escape_html(t)));
                }
                out.push_str(">");
                out.push_str(&escape_html(text));
                out.push_str("</a>");
            }
            InlineItem::Image { alt, url, title } => {
                out.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\"",
                    escape_html(url),
                    escape_html(alt)
                ));
                if let Some(t) = title {
                    out.push_str(&format!(" title=\"{}\"", escape_html(t)));
                }
                out.push_str(">");
            }
        }
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const EMBEDDED_TYPOGRAPHY_CSS: &str = r#"
:root {
    --bg-color: #ffffff;
    --text-color: #1a1a1a;
    --text-muted: #666666;
    --border-color: #e5e7eb;
    --code-bg: #f3f4f6;
    --accent-color: #0284c7;
}

@media (prefers-color-scheme: dark) {
    :root {
        --bg-color: #121212;
        --text-color: #e2e8f0;
        --text-muted: #94a3b8;
        --border-color: #27272a;
        --code-bg: #1e1e24;
        --accent-color: #38bdf8;
    }
}

body {
    background-color: var(--bg-color);
    color: var(--text-color);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    font-size: 16px;
    line-height: 1.7;
    margin: 0;
    padding: 2rem 1.5rem;
}

.markdown-container {
    max-width: 820px;
    margin: 0 auto;
}

h1, h2, h3, h4, h5, h6 {
    font-weight: 600;
    line-height: 1.3;
    margin-top: 1.8em;
    margin-bottom: 0.6em;
    color: var(--text-color);
}

h1 { font-size: 2.2rem; border-bottom: 1px solid var(--border-color); padding-bottom: 0.3em; }
h2 { font-size: 1.6rem; border-bottom: 1px solid var(--border-color); padding-bottom: 0.25em; }
h3 { font-size: 1.3rem; }

p, ul, ol, blockquote, table, pre {
    margin-top: 0;
    margin-bottom: 1.25rem;
}

a {
    color: var(--accent-color);
    text-decoration: none;
}
a:hover {
    text-decoration: underline;
}

code {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 0.88em;
    background-color: var(--code-bg);
    padding: 0.2em 0.4em;
    border-radius: 4px;
}

pre {
    background-color: var(--code-bg);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
    overflow-x: auto;
}

pre code {
    padding: 0;
    background: transparent;
    font-size: 0.9em;
}

blockquote {
    border-left: 4px solid var(--accent-color);
    padding: 0.5rem 1rem;
    color: var(--text-muted);
    background-color: var(--code-bg);
    border-radius: 0 4px 4px 0;
}

table {
    width: 100%;
    border-collapse: collapse;
    margin-bottom: 1.5rem;
}

th, td {
    padding: 0.6rem 0.8rem;
    border: 1px solid var(--border-color);
    text-align: left;
}

th {
    background-color: var(--code-bg);
    font-weight: 600;
}

hr {
    border: none;
    border-top: 1px solid var(--border-color);
    margin: 2rem 0;
}

@media print {
    body {
        padding: 0;
        background: transparent !important;
        color: #000000 !important;
    }
    .markdown-container {
        max-width: 100%;
    }
    pre, blockquote, table {
        page-break-inside: avoid;
    }
}
"#;
