use zmark_core::ast::{
    flatten_block, parse_markdown, BlockElement, InlineGroup, InlineItem, ListItem,
};

/// 1. Test 4 pinned copy expectations via direct AST construction and flatten_block / to_plain_text
#[test]
fn test_pinned_copy_expectations_ast_construction() {
    // 1. quote: two paragraphs => "first\nsecond"
    let q = BlockElement::BlockQuote(vec![
        BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("first".into())])),
        BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("second".into())])),
    ]);
    assert_eq!(q.to_plain_text(), "first\nsecond");
    assert_eq!(flatten_block(&q, "0").text, "first\nsecond");

    // 2. list: two paragraphs in one item => "- first para\n  second para"
    let l2 = BlockElement::List {
        ordered: false,
        items: vec![ListItem {
            checked: None,
            body: vec![
                BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text(
                    "first para".into(),
                )])),
                BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text(
                    "second para".into(),
                )])),
            ],
        }],
    };
    assert_eq!(l2.to_plain_text(), "- first para\n  second para");
    assert_eq!(
        flatten_block(&l2, "0").text,
        "- first para\n  second para"
    );

    // 3. quote: code then paragraph => "fn a() {}\nnext"
    let q3 = BlockElement::BlockQuote(vec![
        BlockElement::FencedCode {
            lang: Some("rust".into()),
            code: "fn a() {}\n".into(),
        },
        BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("next".into())])),
    ]);
    assert_eq!(q3.to_plain_text(), "fn a() {}\nnext");
    assert_eq!(flatten_block(&q3, "0").text, "fn a() {}\nnext");

    // 4. list: para -> code -> para => "- before\n  fn a() {}\n  after"
    let l4 = BlockElement::List {
        ordered: false,
        items: vec![ListItem {
            checked: None,
            body: vec![
                BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("before".into())])),
                BlockElement::FencedCode {
                    lang: None,
                    code: "fn a() {}\n".into(),
                },
                BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("after".into())])),
            ],
        }],
    };
    assert_eq!(l4.to_plain_text(), "- before\n  fn a() {}\n  after");
    assert_eq!(
        flatten_block(&l4, "0").text,
        "- before\n  fn a() {}\n  after"
    );
}

/// 2. Test 3 boundary expectations via direct AST construction and flatten_block / to_plain_text
#[test]
fn test_boundary_expectations_ast_construction() {
    // Edge A: code with an internal blank line inside a list item => "- x\n  a\n  \n  b\n"
    let la = BlockElement::List {
        ordered: false,
        items: vec![ListItem {
            checked: None,
            body: vec![
                BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("x".into())])),
                BlockElement::FencedCode {
                    lang: None,
                    code: "a\n\nb\n".into(),
                },
            ],
        }],
    };
    assert_eq!(la.to_plain_text(), "- x\n  a\n  \n  b\n");
    assert_eq!(flatten_block(&la, "0").text, "- x\n  a\n  \n  b\n");

    // Edge B: code block as the FIRST child of a list item => "- fn a() {}\n  let b = 1;\n"
    let lb = BlockElement::List {
        ordered: false,
        items: vec![ListItem {
            checked: None,
            body: vec![BlockElement::FencedCode {
                lang: None,
                code: "fn a() {}\nlet b = 1;\n".into(),
            }],
        }],
    };
    assert_eq!(lb.to_plain_text(), "- fn a() {}\n  let b = 1;\n");
    assert_eq!(
        flatten_block(&lb, "0").text,
        "- fn a() {}\n  let b = 1;\n"
    );

    // Edge C: two adjacent code blocks in a quote => "a\nb\nafter"
    let qc = BlockElement::BlockQuote(vec![
        BlockElement::FencedCode {
            lang: None,
            code: "a\n".into(),
        },
        BlockElement::FencedCode {
            lang: None,
            code: "b\n".into(),
        },
        BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("after".into())])),
    ]);
    assert_eq!(qc.to_plain_text(), "a\nb\nafter");
    assert_eq!(flatten_block(&qc, "0").text, "a\nb\nafter");

    // Edge D: empty code block in list item => "- before\n  after"
    let ld = BlockElement::List {
        ordered: false,
        items: vec![ListItem {
            checked: None,
            body: vec![
                BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("before".into())])),
                BlockElement::FencedCode {
                    lang: None,
                    code: "".into(),
                },
                BlockElement::Paragraph(InlineGroup(vec![InlineItem::Text("after".into())])),
            ],
        }],
    };
    assert_eq!(ld.to_plain_text(), "- before\n  after");
    assert_eq!(flatten_block(&ld, "0").text, "- before\n  after");
}

/// 3. Test 4 pinned copy expectations via parse_markdown + flatten_block
#[test]
fn test_pinned_copy_expectations_markdown_parse() {
    // 1. quote: two paragraphs
    let doc1 = parse_markdown("> first\n>\n> second");
    assert_eq!(doc1.blocks[0].to_plain_text(), "first\nsecond");
    assert_eq!(flatten_block(&doc1.blocks[0], "0").text, "first\nsecond");

    // 2. list: two paragraphs in one item
    let doc2 = parse_markdown("- first para\n\n  second para");
    assert_eq!(
        doc2.blocks[0].to_plain_text(),
        "- first para\n  second para"
    );
    assert_eq!(
        flatten_block(&doc2.blocks[0], "0").text,
        "- first para\n  second para"
    );

    // 3. quote: code then paragraph
    let doc3 = parse_markdown("> ```rust\n> fn a() {}\n> ```\n> next");
    assert_eq!(doc3.blocks[0].to_plain_text(), "fn a() {}\nnext");
    assert_eq!(flatten_block(&doc3.blocks[0], "0").text, "fn a() {}\nnext");

    // 4. list: para -> code -> para
    let doc4 = parse_markdown("- before\n\n  ```rust\n  fn a() {}\n  ```\n\n  after");
    assert_eq!(
        doc4.blocks[0].to_plain_text(),
        "- before\n  fn a() {}\n  after"
    );
    assert_eq!(
        flatten_block(&doc4.blocks[0], "0").text,
        "- before\n  fn a() {}\n  after"
    );
}

/// 4. Test 3 boundary expectations via parse_markdown + flatten_block
#[test]
fn test_boundary_expectations_markdown_parse() {
    // Edge A: code with blank line in list => "- x\n  a\n  \n  b\n"
    let doc_a = parse_markdown("- x\n\n  ```\n  a\n\n  b\n  ```\n");
    assert_eq!(doc_a.blocks[0].to_plain_text(), "- x\n  a\n  \n  b\n");
    assert_eq!(flatten_block(&doc_a.blocks[0], "0").text, "- x\n  a\n  \n  b\n");

    // Edge B: code as first child in list => "- fn a() {}\n  let b = 1;\n"
    let doc_b = parse_markdown("- ```\n  fn a() {}\n  let b = 1;\n  ```\n");
    assert_eq!(
        doc_b.blocks[0].to_plain_text(),
        "- fn a() {}\n  let b = 1;\n"
    );
    assert_eq!(
        flatten_block(&doc_b.blocks[0], "0").text,
        "- fn a() {}\n  let b = 1;\n"
    );

    // Edge C: adjacent code blocks in quote => "a\nb\nafter"
    let doc_c = parse_markdown("> ```\n> a\n> ```\n> ```\n> b\n> ```\n> after");
    assert_eq!(doc_c.blocks[0].to_plain_text(), "a\nb\nafter");
    assert_eq!(flatten_block(&doc_c.blocks[0], "0").text, "a\nb\nafter");

    // Edge D: empty code block in list item
    let doc_d = parse_markdown("- before\n\n  ```\n  ```\n\n  after");
    assert_eq!(doc_d.blocks[0].to_plain_text(), "- before\n  after");
    assert_eq!(flatten_block(&doc_d.blocks[0], "0").text, "- before\n  after");
}

/// 5. Test 8 backward-compatible single-paragraph shapes matching replan4_compat_probe.rs
#[test]
fn test_compat_old_shapes_all() {
    let samples: [(&str, &str, Vec<&str>); 8] = [
        ("plain item", "- item", vec!["- item"]),
        (
            "item + sublist",
            "- item\n  - sub a\n  - sub b",
            vec!["- item\n  - sub a\n  - sub b"],
        ),
        (
            "ordered + sublist",
            "1. one\n2. two\n   - s1",
            vec!["1. one\n2. two\n  - s1"],
        ),
        (
            "checked item",
            "- [x] done\n- [ ] todo",
            vec!["- [x] done\n- [ ] todo"],
        ),
        ("single-para quote", "> quoted text", vec!["quoted text"]),
        ("two separate quotes", "> a\n\n> b", vec!["a", "b"]),
        (
            "soft-wrapped item",
            "- a paragraph that wraps\n  onto a second source line",
            vec!["- a paragraph that wraps onto a second source line"],
        ),
        (
            "item with inline code",
            "- use `cargo test` then\n  - nested `x`",
            vec!["- use cargo test then\n  - nested x"],
        ),
    ];

    for (label, src, expected) in samples {
        let doc = parse_markdown(src);
        let actual_plain: Vec<String> = doc.blocks.iter().map(|b| b.to_plain_text()).collect();
        assert_eq!(actual_plain, expected, "COMPAT failure on sample '{}' (to_plain_text)", label);
        let actual_flat: Vec<String> = doc.blocks.iter().enumerate().map(|(i, b)| flatten_block(b, &i.to_string()).text).collect();
        assert_eq!(actual_flat, expected, "COMPAT failure on sample '{}' (flatten_block)", label);
    }
}
