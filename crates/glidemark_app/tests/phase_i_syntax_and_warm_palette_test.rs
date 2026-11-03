use glidemark_app::theme::ThemeColors;
use glidemark_app::views::editor::{tokenize_document_lines, MarkdownTokenKind};
use glidemark_app::views::preview::apply_selection_to_highlights;
use glidemark_core::syntax::{
    get_or_parse_code_spans, parse_code_spans, resolve_canonical_language, SyntaxRole,
    MAX_CODE_BLOCK_BYTES,
};
use gpui::HighlightStyle;

/// Test 1: Canonical language alias table and fallbacks per Phase I specification
#[test]
fn test_phase_i_canonical_language_aliases_and_plain_fallbacks() {
    // 1. Canonical aliases table
    let expected_mappings = [
        ("xml", Some("XML")),
        ("XML", Some("XML")),
        ("html", Some("HTML")),
        ("htm", Some("HTML")),
        ("java", Some("Java")),
        ("JAVA", Some("Java")),
        ("python", Some("Python")),
        ("py", Some("Python")),
        ("javascript", Some("JavaScript")),
        ("js", Some("JavaScript")),
        ("json", Some("JSON")),
        ("yaml", Some("YAML")),
        ("yml", Some("YAML")),
        ("sql", Some("SQL")),
        ("shell", Some("Bourne Again Shell (bash)")),
        ("sh", Some("Bourne Again Shell (bash)")),
        ("bash", Some("Bourne Again Shell (bash)")),
        ("rust", Some("Rust")),
        ("rs", Some("Rust")),
    ];

    for (raw, expected) in expected_mappings {
        assert_eq!(
            resolve_canonical_language(Some(raw)),
            expected,
            "Raw language '{}' must resolve to canonical {:?}",
            raw,
            expected
        );
    }

    // 2. Info string with extra parameters, e.g. "```rust,no_run" or "```python title='main.py'"
    assert_eq!(resolve_canonical_language(Some("rust,no_run")), Some("Rust"));
    assert_eq!(resolve_canonical_language(Some("python title='main.py'")), Some("Python"));
    assert_eq!(resolve_canonical_language(Some("  xml  ")), Some("XML"));

    // 3. Plain / empty fallbacks
    assert_eq!(resolve_canonical_language(None), None);
    assert_eq!(resolve_canonical_language(Some("")), None);
    assert_eq!(resolve_canonical_language(Some("   ")), None);
    assert_eq!(resolve_canonical_language(Some("text")), None);
    assert_eq!(resolve_canonical_language(Some("txt")), None);
    assert_eq!(resolve_canonical_language(Some("plain")), None);
    assert_eq!(resolve_canonical_language(Some("plaintext")), None);

    // 4. Unknown languages (e.g. typescript, toml, zsh) return None / Plain per Phase I boundary
    assert_eq!(resolve_canonical_language(Some("typescript")), None);
    assert_eq!(resolve_canonical_language(Some("ts")), None);
    assert_eq!(resolve_canonical_language(Some("toml")), None);
    assert_eq!(resolve_canonical_language(Some("zsh")), None);
    assert_eq!(resolve_canonical_language(Some("unknown_lang")), None);

    // Unknown language code block parses to single Plain span covering entire text
    let code = "const x: number = 42;\nconsole.log(x);";
    let spans = parse_code_spans(code, Some("typescript"));
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].role, SyntaxRole::Plain);
    assert_eq!(spans[0].range, 0..code.len());
}

/// Test 2: Maven XML Tag (Keyword) vs Attribute (Attribute) vs String vs Comment distinction
#[test]
fn test_phase_i_maven_xml_semantic_distinction() {
    let xml = r#"<!-- Maven POM configuration -->
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
    <modelVersion>4.0.0</modelVersion>
    <groupId>org.apache.maven</groupId>
    <artifactId>maven-core</artifactId>
    <version>3.9.0</version>
</project>"#;

    let spans = parse_code_spans(xml, Some("xml"));

    // 1. Comment
    let comment_span = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Comment && xml[s.range.clone()].contains("Maven POM configuration"))
        .expect("Must have Comment span for XML comment");
    assert_eq!(comment_span.role, SyntaxRole::Comment);

    // 2. Tag names (Keyword)
    let project_tags: Vec<_> = spans
        .iter()
        .filter(|s| s.role == SyntaxRole::Keyword && &xml[s.range.clone()] == "project")
        .collect();
    assert!(!project_tags.is_empty(), "Tag 'project' must be Keyword");

    let group_id_tag = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Keyword && &xml[s.range.clone()] == "groupId")
        .expect("Tag 'groupId' must be Keyword");
    assert_eq!(group_id_tag.role, SyntaxRole::Keyword);

    // 3. Attribute names (Attribute)
    let xmlns_attr = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Attribute && &xml[s.range.clone()] == "xmlns")
        .expect("Attribute 'xmlns' must be Attribute");
    assert_eq!(xmlns_attr.role, SyntaxRole::Attribute);

    // 4. Attribute values (String)
    let attr_val = spans
        .iter()
        .find(|s| s.role == SyntaxRole::String && xml[s.range.clone()].contains("http://maven.apache.org/POM/4.0.0"))
        .expect("Attribute string must be String");
    assert_eq!(attr_val.role, SyntaxRole::String);

    // Invariant: contiguous byte reconstruction
    let reconstructed: String = spans.iter().map(|s| &xml[s.range.clone()]).collect();
    assert_eq!(reconstructed, xml, "Reconstructed text must match exactly byte-for-byte");
}

/// Test 3: Multi-line continuity across lines (comments and strings)
#[test]
fn test_phase_i_multiline_syntax_continuity() {
    let rust_code = r#"/*
 * Multi-line Rust block comment
 * continuing across lines
 */
pub fn calculate() -> i32 {
    let multiline_str = "hello\nworld\nfoo";
    42
}"#;

    let spans = parse_code_spans(rust_code, Some("rust"));

    // Multi-line comment should span across lines
    let comment_span = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Comment && rust_code[s.range.clone()].contains("Multi-line Rust block comment"))
        .expect("Must have multi-line comment span");
    assert!(rust_code[comment_span.range.clone()].contains("continuing across lines"));

    // fn after multi-line comment must still be recognized as Keyword
    let fn_span = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Keyword && &rust_code[s.range.clone()] == "fn")
        .expect("'fn' must be Keyword");
    assert_eq!(fn_span.role, SyntaxRole::Keyword);

    // Number 42 must be Number
    let num_span = spans
        .iter()
        .find(|s| s.role == SyntaxRole::Number && &rust_code[s.range.clone()] == "42")
        .expect("'42' must be Number");
    assert_eq!(num_span.role, SyntaxRole::Number);
}

/// Test 4: Threshold downgrades to Plain per Phase I boundary
#[test]
fn test_phase_i_threshold_downgrades_to_plain() {
    // 1. > 64 KiB
    let huge_code = "let x = 1;\n".repeat(6000);
    assert!(huge_code.len() > MAX_CODE_BLOCK_BYTES);
    let spans_huge = parse_code_spans(&huge_code, Some("rust"));
    assert_eq!(spans_huge.len(), 1, "Huge block must return exactly 1 span");
    assert_eq!(spans_huge[0].role, SyntaxRole::Plain);
    assert_eq!(spans_huge[0].range, 0..huge_code.len());

    // 2. > 2000 physical lines
    let long_code = "\n".repeat(2005);
    let spans_long = parse_code_spans(&long_code, Some("rust"));
    assert_eq!(spans_long.len(), 1, "Block with > 2000 lines must downgrade to Plain");
    assert_eq!(spans_long[0].role, SyntaxRole::Plain);
    assert_eq!(spans_long[0].range, 0..long_code.len());

    // 3. Any line > 1000 unicode scalars
    let wide_line = format!("let s = \"{}\";\n", "a".repeat(1005));
    let spans_wide = parse_code_spans(&wide_line, Some("rust"));
    assert_eq!(spans_wide.len(), 1, "Block with line > 1000 unicode scalars must downgrade to Plain");
    assert_eq!(spans_wide[0].role, SyntaxRole::Plain);
    assert_eq!(spans_wide[0].range, 0..wide_line.len());
}

/// Test 5: Invariant: UTF-8 contiguous non-overlapping coverage with Tab, CRLF, CJK, and Emoji
#[test]
fn test_phase_i_utf8_contiguous_non_overlapping_coverage() {
    let complex_code = "fn main() {\r\n\tlet cjk = \"你好，世界 🦀\";\r\n\tlet tab_indent = \t123;\r\n}\r\n";
    let spans = parse_code_spans(complex_code, Some("rust"));

    let mut current_offset = 0;
    for span in &spans {
        assert_eq!(
            span.range.start, current_offset,
            "Gaps or overlapping spans detected at byte {}",
            current_offset
        );
        assert!(span.range.end > span.range.start, "Empty span detected");
        assert!(
            complex_code.is_char_boundary(span.range.start),
            "Span start must be a valid UTF-8 char boundary"
        );
        assert!(
            complex_code.is_char_boundary(span.range.end),
            "Span end must be a valid UTF-8 char boundary"
        );
        current_offset = span.range.end;
    }
    assert_eq!(
        current_offset,
        complex_code.len(),
        "Spans must completely cover the full byte range of the code payload"
    );

    let reconstructed: String = spans.iter().map(|s| &complex_code[s.range.clone()]).collect();
    assert_eq!(reconstructed, complex_code);
}

/// Test 6: Bounded LRU cache and theme independence
#[test]
fn test_phase_i_bounded_lru_cache_and_theme_independence() {
    let code = "def greet(name: str) -> str:\n    return f'Hello, {name}'";

    // 1. First call parses and inserts into cache
    let spans1 = get_or_parse_code_spans(code, Some("python"));

    // 2. Second call returns the cached Arc without reparsing
    let spans2 = get_or_parse_code_spans(code, Some("python"));
    assert!(
        std::sync::Arc::ptr_eq(&spans1, &spans2),
        "Identical code and language must return the exact same cached Arc"
    );

    // 3. Theme change does not invalidate syntax parse cache:
    // Color mapping is a pure projection from ThemeColors::syntax_color_for_role
    let theme_light = ThemeColors::default_palette(false);
    let theme_dark = ThemeColors::default_palette(true);

    for span in spans1.iter() {
        let color_light = theme_light.syntax_color_for_role(span.role);
        let color_dark = theme_dark.syntax_color_for_role(span.role);
        assert_eq!(color_light.a, 1.0);
        assert_eq!(color_dark.a, 1.0);
    }
}

/// Test 7: Preview and Editor consistency on the same code block
#[test]
fn test_phase_i_preview_and_editor_syntax_consistency() {
    let markdown_doc = [
        "# Sample Document",
        "```xml",
        "<project xmlns=\"http://maven.apache.org\">",
        "    <groupId>org.apache.maven</groupId>",
        "</project>",
        "```",
        "End of document",
    ];

    // 1. Editor tokenization
    let editor_tokens = tokenize_document_lines(&markdown_doc);
    assert_eq!(editor_tokens.len(), 7);

    // Line 1: ```xml delimiter
    assert_eq!(editor_tokens[1][0].kind, MarkdownTokenKind::CodeDelimiter);

    // Line 2: <project xmlns="http://maven.apache.org">
    let line2_tokens = &editor_tokens[2];
    assert!(
        line2_tokens.iter().any(|t| t.kind == MarkdownTokenKind::Keyword && t.text == "project"),
        "Editor must recognize 'project' tag as Keyword"
    );
    assert!(
        line2_tokens.iter().any(|t| t.kind == MarkdownTokenKind::Attribute && t.text == "xmlns"),
        "Editor must recognize 'xmlns' as Attribute"
    );
    assert!(
        line2_tokens.iter().any(|t| t.kind == MarkdownTokenKind::StringLiteral && t.text.contains("maven.apache.org")),
        "Editor must recognize attribute value as StringLiteral"
    );

    // Line 3: <groupId>org.apache.maven</groupId>
    let line3_tokens = &editor_tokens[3];
    assert!(
        line3_tokens.iter().any(|t| t.kind == MarkdownTokenKind::Keyword && t.text == "groupId"),
        "Editor must recognize 'groupId' tag as Keyword"
    );

    // 2. Preview semantic spans
    let xml_payload = "<project xmlns=\"http://maven.apache.org\">\n    <groupId>org.apache.maven</groupId>\n</project>";
    let preview_spans = get_or_parse_code_spans(xml_payload, Some("xml"));

    assert!(
        preview_spans.iter().any(|s| s.role == SyntaxRole::Keyword && &xml_payload[s.range.clone()] == "project"),
        "Preview must recognize 'project' tag as Keyword"
    );
    assert!(
        preview_spans.iter().any(|s| s.role == SyntaxRole::Attribute && &xml_payload[s.range.clone()] == "xmlns"),
        "Preview must recognize 'xmlns' as Attribute"
    );

    // 3. Selection overlay without wiping foreground colors
    let theme = ThemeColors::default_palette(false);
    let syntax_highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = preview_spans
        .iter()
        .map(|s| {
            (
                s.range.clone(),
                HighlightStyle {
                    color: Some(theme.syntax_color_for_role(s.role)),
                    ..Default::default()
                },
            )
        })
        .collect();

    // Select middle range 5..25
    let selected = apply_selection_to_highlights(xml_payload, &syntax_highlights, 5..25, theme.selection_bg());
    assert!(!selected.is_empty());

    // Highlight spans inside the selection must have both background_color AND color preserved
    for (range, style) in &selected {
        if range.start >= 5 && range.end <= 25 {
            assert!(style.background_color.is_some(), "Selection background must be set");
            assert!(style.color.is_some(), "Syntax foreground color must NOT be wiped by selection");
        }
    }
}
