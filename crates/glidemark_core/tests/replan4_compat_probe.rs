use glidemark_core::ast::parse_markdown;

#[test]
fn test_compat_old_shapes() {
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
        let actual: Vec<String> = doc.blocks.iter().map(|b| b.to_plain_text()).collect();
        assert_eq!(actual, expected, "COMPAT failure on sample '{}'", label);
    }
}
