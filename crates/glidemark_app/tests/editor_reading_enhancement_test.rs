use glidemark_app::views::editor::{
    calculate_line_row_height, tokenize_markdown_line,
    MarkdownToken, MarkdownTokenKind,
};
use glidemark_app::views::workspace::{adaptive_header_padding, CANVAS_TITLE_LEFT_MARGIN};

/// Requirement 1: Canvas title left margin aligns with content margin (ADR 0036)
/// In all view modes with drawer open, header padding (8px) + title inner padding (8px)
/// equals CANVAS_TITLE_LEFT_MARGIN (16.0px), aligning title text with content margin.
#[test]
fn test_canvas_title_left_margin_aligns_with_content_margin() {
    assert_eq!(CANVAS_TITLE_LEFT_MARGIN, 16.0);

    // 1. Drawer open state (aligns with 16px content margin across all modes):
    let header_pl_open = adaptive_header_padding(true, false, true);
    assert_eq!(header_pl_open, 8.0);
    let title_pill_px = 8.0; // px_2 inner padding
    let actual_title_text_x = header_pl_open + title_pill_px;
    assert_eq!(actual_title_text_x, CANVAS_TITLE_LEFT_MARGIN);

    // 2. Drawer collapsed state (traffic light safety zone):
    let header_pl_collapsed = adaptive_header_padding(true, false, false);
    assert_eq!(header_pl_collapsed, 76.0); // macOS traffic light clearance
    let btn_expand_w = 28.0;
    let btn_gap = 6.0; // gap_1p5
    let actual_collapsed_title_x = header_pl_collapsed + btn_expand_w + btn_gap + title_pill_px;
    assert!(actual_collapsed_title_x >= 110.0, "Title must sit safely past expand button and traffic lights");
}

/// Requirement 2: Dynamic line row height calculation and sanitization fallback
#[test]
fn test_dynamic_line_row_height_calculation() {
    let font_size = 16.0;

    // Normal valid line heights
    let cases = [
        (1.3, 16.0 * 1.3),
        (1.4, 16.0 * 1.4),
        (1.5, 16.0 * 1.5),
        (1.6, 16.0 * 1.6),
        (1.7, 16.0 * 1.7),
    ];

    for (lh, expected) in cases {
        let calculated = calculate_line_row_height(font_size, lh);
        assert!(
            (calculated - expected).abs() < 1e-4,
            "calculate_line_row_height({}, {}) = {}, expected {}",
            font_size,
            lh,
            calculated,
            expected
        );
    }

    // Protection against invalid or out-of-bounds line heights (fallback: 1.6)
    let fallback_expected = 16.0 * 1.6;
    let fallback_cases = [
        0.5,
        2.5,
        f32::NAN,
        f32::INFINITY,
        -1.0,
        100.0,
    ];

    for invalid_lh in fallback_cases {
        let calculated = calculate_line_row_height(font_size, invalid_lh);
        assert!(
            (calculated - fallback_expected).abs() < 1e-4,
            "calculate_line_row_height({}, {}) = {}, expected fallback {}",
            font_size,
            invalid_lh,
            calculated,
            fallback_expected
        );
    }
}

/// Requirement 3: Markdown tokenizer headings (levels 1-6 and invalid heading syntax)
#[test]
fn test_markdown_tokenizer_headings() {
    // Level 1
    let t1 = tokenize_markdown_line("# Heading 1");
    assert_eq!(
        t1,
        vec![
            MarkdownToken {
                text: "# ".to_string(),
                kind: MarkdownTokenKind::HeadingPrefix,
            },
            MarkdownToken {
                text: "Heading 1".to_string(),
                kind: MarkdownTokenKind::HeadingText,
            },
        ]
    );

    // Level 2
    let t2 = tokenize_markdown_line("## Second level");
    assert_eq!(
        t2,
        vec![
            MarkdownToken {
                text: "## ".to_string(),
                kind: MarkdownTokenKind::HeadingPrefix,
            },
            MarkdownToken {
                text: "Second level".to_string(),
                kind: MarkdownTokenKind::HeadingText,
            },
        ]
    );

    // Level 6
    let t6 = tokenize_markdown_line("###### Sixth level");
    assert_eq!(
        t6,
        vec![
            MarkdownToken {
                text: "###### ".to_string(),
                kind: MarkdownTokenKind::HeadingPrefix,
            },
            MarkdownToken {
                text: "Sixth level".to_string(),
                kind: MarkdownTokenKind::HeadingText,
            },
        ]
    );

    // Not a heading: `#` not followed by a space
    let t_not = tokenize_markdown_line("#NotAHeading");
    assert_eq!(
        t_not,
        vec![MarkdownToken {
            text: "#NotAHeading".to_string(),
            kind: MarkdownTokenKind::Plain,
        }]
    );

    // Not a heading: 7 hashes (> 6)
    let t_seven = tokenize_markdown_line("####### Seven hashes");
    assert_eq!(
        t_seven,
        vec![MarkdownToken {
            text: "####### Seven hashes".to_string(),
            kind: MarkdownTokenKind::Plain,
        }]
    );
}

/// Requirement 4: Markdown tokenizer blockquotes
#[test]
fn test_markdown_tokenizer_blockquotes() {
    // With space: `> Quote text`
    let t1 = tokenize_markdown_line("> Quote text");
    assert_eq!(
        t1,
        vec![
            MarkdownToken {
                text: "> ".to_string(),
                kind: MarkdownTokenKind::BlockquotePrefix,
            },
            MarkdownToken {
                text: "Quote text".to_string(),
                kind: MarkdownTokenKind::BlockquoteText,
            },
        ]
    );

    // Without space: `>Quote text`
    let t2 = tokenize_markdown_line(">Quote text");
    assert_eq!(
        t2,
        vec![
            MarkdownToken {
                text: ">".to_string(),
                kind: MarkdownTokenKind::BlockquotePrefix,
            },
            MarkdownToken {
                text: "Quote text".to_string(),
                kind: MarkdownTokenKind::BlockquoteText,
            },
        ]
    );
}

/// Requirement 5: Markdown tokenizer inline code and bold text
#[test]
fn test_markdown_tokenizer_inline_code_and_bold() {
    let line = "Here is `code` and **bold** text";
    let tokens = tokenize_markdown_line(line);

    // Verify tokens include inline code and bold
    assert!(
        tokens.iter().any(|t| t.kind == MarkdownTokenKind::InlineCode && t.text == "`code`"),
        "Must contain MarkdownTokenKind::InlineCode with text '`code`'. Got: {:?}",
        tokens
    );
    assert!(
        tokens.iter().any(|t| t.kind == MarkdownTokenKind::Bold && t.text == "**bold**"),
        "Must contain MarkdownTokenKind::Bold with text '**bold**'. Got: {:?}",
        tokens
    );

    // Check full reconstructed content equals original string
    let reconstructed: String = tokens.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(reconstructed, line);
}

/// Requirement 6: Markdown tokenizer links
#[test]
fn test_markdown_tokenizer_links() {
    let line = "See [GlideMark](https://glidemark.app) for details";
    let tokens = tokenize_markdown_line(line);

    // Verify tokens include link text and link url
    assert!(
        tokens.iter().any(|t| t.kind == MarkdownTokenKind::LinkText && t.text == "[GlideMark]"),
        "Must contain MarkdownTokenKind::LinkText with text '[GlideMark]'. Got: {:?}",
        tokens
    );
    assert!(
        tokens.iter().any(|t| t.kind == MarkdownTokenKind::LinkUrl && t.text == "(https://glidemark.app)"),
        "Must contain MarkdownTokenKind::LinkUrl with text '(https://glidemark.app)'. Got: {:?}",
        tokens
    );

    // Check full reconstructed content equals original string
    let reconstructed: String = tokens.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(reconstructed, line);
}

/// Requirement 7: Markdown tokenizer empty line and plain text
#[test]
fn test_markdown_tokenizer_empty_and_plain() {
    // Empty line -> non-empty fallback " "
    let empty_tokens = tokenize_markdown_line("");
    assert_eq!(
        empty_tokens,
        vec![MarkdownToken {
            text: " ".to_string(),
            kind: MarkdownTokenKind::Plain,
        }]
    );

    // Plain text without markdown formatting
    let plain_line = "Just standard plain markdown paragraph text without any markers.";
    let plain_tokens = tokenize_markdown_line(plain_line);
    assert_eq!(
        plain_tokens,
        vec![MarkdownToken {
            text: plain_line.to_string(),
            kind: MarkdownTokenKind::Plain,
        }]
    );
}

/// Requirement 8: Markdown tokenization and syntax highlight preservation
///
/// Ensures syntax highlighting tokens preserve full text across various constructs.
#[test]
fn test_markdown_tokenization_preserves_full_text() {
    let test_lines = [
        "### Title",
        "Here is **bold** text",
        "Run `cargo test` now",
        "> Blockquote with nested text",
        "Plain text line without markup",
        "Check [GlideMark](https://glidemark.app) link",
    ];

    for line in test_lines {
        let tokens = tokenize_markdown_line(line);
        let recon: String = tokens.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(recon, line, "Reconstructed text mismatch for {:?}", line);
    }
}

/// Requirement 9: Scroll wheel row height linkage
/// Verifies top_line_index calculation across various line heights
#[test]
fn test_scroll_wheel_row_height_linkage() {
    let font_size = 16.0;
    let line_heights = [1.3, 1.4, 1.5, 1.6, 1.7];
    let scroll_y = 320.0;
    for lh in line_heights {
        let row_h = calculate_line_row_height(font_size, lh);
        let top_line_index = (scroll_y / row_h.max(1.0)).floor() as usize;
        let expected = (320.0 / (font_size * lh)).floor() as usize;
        assert_eq!(
            top_line_index, expected,
            "Mismatch in top_line_index calculation for font_size = {}, lh = {}",
            font_size, lh
        );
    }
}
