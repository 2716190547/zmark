use zmark_core::ast::parse_markdown;
use zmark_core::export::export_html;

#[test]
fn exported_links_reject_active_protocols_but_keep_web_mail_and_relative_urls() {
    use zmark_core::ast::{BlockElement, InlineGroup, InlineItem, ParsedDocument};
    for url in ["javascript:alert(1)", "JaVaScRiPt:alert(1)", " javaScript:alert(1)", "java\tscript:alert(1)", "data:text/html,<script>alert(1)</script>", "vbscript:msgbox(1)"] {
        let doc = ParsedDocument { blocks: vec![BlockElement::Paragraph(InlineGroup(vec![InlineItem::Link {
            text: "safe text".into(), url: url.into(), title: None,
        }]))], ..Default::default() };
        let html = export_html(&doc, "test");
        assert!(!html.contains("<a href="), "{url:?}");
        assert!(html.contains("safe text"));
    }
    for url in ["https://example.com/a?x=1&y=2", "http://example.com", "mailto:user@example.com", "#heading", "notes/file.md", "../other.md"] {
        let doc = ParsedDocument { blocks: vec![BlockElement::Paragraph(InlineGroup(vec![InlineItem::Link {
            text: "link".into(), url: url.into(), title: None,
        }]))], ..Default::default() };
        assert!(export_html(&doc, "test").contains("<a href="), "{url}");
    }
}

#[test]
fn test_export_standalone_html() {
    let source = "# Sample Document\n\nThis is a **bold** paragraph.\n\n```rust\nfn main() {}\n```\n\n| H1 | H2 |\n|---|---|\n| A | B |\n\n- [x] Item 1\n- [ ] Item 2";
    let doc = parse_markdown(source);
    let html = export_html(&doc, "Sample Document");

    // Must be valid standalone HTML5
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<title>Sample Document</title>"));
    assert!(html.contains("<style>"));
    assert!(html.contains("@media print"));

    // Check rendered body elements
    assert!(html.contains("<h1>Sample Document</h1>"));
    assert!(html.contains("<strong>bold</strong>"));
    assert!(html.contains("<pre><code class=\"language-rust\">"));
    assert!(html.contains("<table>"));
    assert!(html.contains("<th>H1</th>"));
    assert!(html.contains("<td>A</td>"));
    assert!(html.contains("type=\"checkbox\" checked disabled"));
    assert!(html.contains("type=\"checkbox\" disabled"));
}
