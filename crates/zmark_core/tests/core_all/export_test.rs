use zmark_core::ast::parse_markdown;
use zmark_core::export::export_html;

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
