use glidemark_core::ast::{parse_markdown, BlockElement, InlineItem};

#[test]
fn test_parse_headings_and_paragraphs_with_anchors() {
    let source = "# Heading 1\n\nThis is a paragraph with **bold** text.\n\n## Subheading\n\nSecond paragraph.";
    let doc = parse_markdown(source);

    assert_eq!(doc.blocks.len(), 4);
    assert_eq!(doc.anchors.len(), 4);

    // Block 0: Heading 1 on line 1
    match &doc.blocks[0] {
        BlockElement::Heading { level, content } => {
            assert_eq!(*level, 1);
            assert_eq!(content.0, vec![InlineItem::Text("Heading 1".into())]);
        }
        _ => panic!("Expected Heading"),
    }
    assert_eq!(doc.anchors[0].block_index, 0);
    assert_eq!(doc.anchors[0].source_line_start, 1);
    assert_eq!(doc.anchors[0].source_line_end, 1);

    // Block 1: Paragraph on line 3
    match &doc.blocks[1] {
        BlockElement::Paragraph(content) => {
            assert_eq!(content.0.len(), 3);
            assert_eq!(content.0[0], InlineItem::Text("This is a paragraph with ".into()));
            assert_eq!(content.0[1], InlineItem::Bold("bold".into()));
            assert_eq!(content.0[2], InlineItem::Text(" text.".into()));
        }
        _ => panic!("Expected Paragraph"),
    }
    assert_eq!(doc.anchors[1].source_line_start, 3);
    assert_eq!(doc.anchors[1].source_line_end, 3);

    // Block 2: Subheading on line 5
    assert_eq!(doc.anchors[2].source_line_start, 5);
    assert_eq!(doc.anchors[2].source_line_end, 5);

    // Block 3: Second paragraph on line 7
    assert_eq!(doc.anchors[3].source_line_start, 7);
    assert_eq!(doc.anchors[3].source_line_end, 7);
}

#[test]
fn test_parse_fenced_code_block() {
    let source = "```rust\nfn main() {\n    println!(\"hello\");\n}\n```";
    let doc = parse_markdown(source);

    assert_eq!(doc.blocks.len(), 1);
    match &doc.blocks[0] {
        BlockElement::FencedCode { lang, code } => {
            assert_eq!(lang.as_deref(), Some("rust"));
            assert_eq!(code, "fn main() {\n    println!(\"hello\");\n}\n");
        }
        _ => panic!("Expected FencedCode"),
    }
    assert_eq!(doc.anchors[0].source_line_start, 1);
    assert_eq!(doc.anchors[0].source_line_end, 5);
}

#[test]
fn test_parse_gfm_tasklist_and_table() {
    let source = "| Col A | Col B |\n|---|---|\n| 1 | 2 |\n\n- [x] Done\n- [ ] Todo";
    let doc = parse_markdown(source);

    assert_eq!(doc.blocks.len(), 2);

    // Table
    match &doc.blocks[0] {
        BlockElement::Table { headers, rows } => {
            assert_eq!(headers.len(), 2);
            assert_eq!(rows.len(), 1);
        }
        _ => panic!("Expected Table"),
    }
    assert_eq!(doc.anchors[0].source_line_start, 1);
    assert_eq!(doc.anchors[0].source_line_end, 3);

    // Task list
    match &doc.blocks[1] {
        BlockElement::List { ordered, items } => {
            assert!(!*ordered);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].checked, Some(true));
            assert_eq!(items[1].checked, Some(false));
        }
        _ => panic!("Expected List"),
    }
    assert_eq!(doc.anchors[1].source_line_start, 5);
    assert_eq!(doc.anchors[1].source_line_end, 6);
}

#[test]
fn test_parse_empty_and_rule() {
    let empty_doc = parse_markdown("");
    assert!(empty_doc.blocks.is_empty());
    assert!(empty_doc.anchors.is_empty());

    let rule_doc = parse_markdown("---\n\n***");
    assert_eq!(rule_doc.blocks.len(), 2);
    assert_eq!(rule_doc.blocks[0], BlockElement::Rule);
    assert_eq!(rule_doc.blocks[1], BlockElement::Rule);
    assert_eq!(rule_doc.anchors[0].source_line_start, 1);
    assert_eq!(rule_doc.anchors[1].source_line_start, 3);
}

#[test]
fn test_parse_links_and_images() {
    let source = "[Glide](https://github.com/glide \"Glide Title\")\n\n![Logo](./assets/logo.png)";
    let doc = parse_markdown(source);
    assert_eq!(doc.blocks.len(), 2);

    match &doc.blocks[0] {
        BlockElement::Paragraph(p) => {
            assert_eq!(p.0[0], InlineItem::Link {
                text: "Glide".into(),
                url: "https://github.com/glide".into(),
                title: Some("Glide Title".into()),
            });
        }
        _ => panic!("Expected Paragraph with Link"),
    }

    match &doc.blocks[1] {
        BlockElement::Paragraph(p) => {
            assert_eq!(p.0[0], InlineItem::Image {
                alt: "Logo".into(),
                url: "./assets/logo.png".into(),
                title: None,
            });
        }
        _ => panic!("Expected Paragraph with Image"),
    }
}
