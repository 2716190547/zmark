use glidemark_app::state::ViewMode;
use glidemark_app::views::editor::{
    build_active_line_segments, calculate_line_row_height, tokenize_markdown_line,
    ActiveLineSegment, MarkdownToken, MarkdownTokenKind,
};
use glidemark_app::views::workspace::canvas_title_offset;

/// Requirement 1: Canvas title offset aligns with editor gutter
/// In Source/Split mode with drawer open, offset is 48.0 to align title with code column 1.
/// In Preview mode, or when drawer is closed, offset is 0.0.
#[test]
fn test_canvas_title_offset_aligns_with_editor_gutter() {
    // 1. Drawer open state (core workflow):
    let header_pl_open = 8.0;
    let title_pill_px = 8.0; // px_2 inner padding
    let editor_code_column_start = 16.0 + 36.0 + 12.0; // 16px padding + 36px line_no + 12px margin = 64.0
    let actual_title_text_x = header_pl_open + canvas_title_offset(ViewMode::Source, true) + title_pill_px;
    assert_eq!(actual_title_text_x, 64.0);
    assert_eq!(actual_title_text_x, editor_code_column_start);

    let split_title_text_x = header_pl_open + canvas_title_offset(ViewMode::Split, true) + title_pill_px;
    assert_eq!(split_title_text_x, editor_code_column_start);

    // 2. Drawer collapsed state (traffic light safety zone):
    let header_pl_collapsed = 76.0; // macOS traffic light clearance
    let btn_expand_w = 28.0;
    let btn_gap = 6.0; // gap_1p5
    let offset_collapsed = canvas_title_offset(ViewMode::Source, false);
    assert_eq!(offset_collapsed, 0.0);
    let actual_collapsed_title_x = header_pl_collapsed + btn_expand_w + btn_gap + offset_collapsed + title_pill_px;
    assert!(actual_collapsed_title_x >= 110.0, "Title must sit safely past expand button and traffic lights");

    // 3. Preview mode offset is 0.0:
    assert_eq!(canvas_title_offset(ViewMode::Preview, true), 0.0);
    assert_eq!(canvas_title_offset(ViewMode::Preview, false), 0.0);
    assert_eq!(canvas_title_offset(ViewMode::Source, false), 0.0);
    assert_eq!(canvas_title_offset(ViewMode::Split, false), 0.0);
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

/// Helper to reconstruct line text from active line segments by ignoring the cursor
fn reconstruct_segments(segments: &[ActiveLineSegment]) -> String {
    segments
        .iter()
        .filter_map(|seg| match seg {
            ActiveLineSegment::Token(tok) => Some(tok.text.as_str()),
            ActiveLineSegment::Cursor => None,
        })
        .collect()
}

/// Requirement 8: Active line token splitting and syntax highlight preservation
///
/// Ensures active line with cursor maintains syntax highlighting for tokens,
/// cleanly splits the active token around the cursor, and preserves the full text.
#[test]
fn test_active_line_token_splitting_and_highlight_preservation() {
    // -----------------------------------------------------------------
    // Test 1: Cursor inside heading prefix:
    // "### Title" with cursor at col 2 -> "##", Cursor, "# ", "Title"
    // -----------------------------------------------------------------
    let line1 = "### Title";
    let tokens1 = tokenize_markdown_line(line1);
    let segments1 = build_active_line_segments(&tokens1, 2);
    assert_eq!(
        segments1,
        vec![
            ActiveLineSegment::Token(MarkdownToken {
                text: "##".to_string(),
                kind: MarkdownTokenKind::HeadingPrefix,
            }),
            ActiveLineSegment::Cursor,
            ActiveLineSegment::Token(MarkdownToken {
                text: "# ".to_string(),
                kind: MarkdownTokenKind::HeadingPrefix,
            }),
            ActiveLineSegment::Token(MarkdownToken {
                text: "Title".to_string(),
                kind: MarkdownTokenKind::HeadingText,
            }),
        ]
    );

    // -----------------------------------------------------------------
    // Test 2: Cursor inside bold text:
    // "Here is **bold** text" with cursor at pos 11 (after "**b")
    // Both before and after cursor tokens maintain MarkdownTokenKind::Bold
    // -----------------------------------------------------------------
    let line2 = "Here is **bold** text";
    let tokens2 = tokenize_markdown_line(line2);
    let segments2 = build_active_line_segments(&tokens2, 11);
    let cursor_idx2 = segments2
        .iter()
        .position(|s| matches!(s, ActiveLineSegment::Cursor))
        .expect("Cursor must be present in segments");
    assert!(cursor_idx2 > 0 && cursor_idx2 < segments2.len() - 1);
    match (&segments2[cursor_idx2 - 1], &segments2[cursor_idx2 + 1]) {
        (ActiveLineSegment::Token(before), ActiveLineSegment::Token(after)) => {
            assert_eq!(before.kind, MarkdownTokenKind::Bold);
            assert_eq!(after.kind, MarkdownTokenKind::Bold);
            assert_eq!(before.text, "**b");
            assert_eq!(after.text, "old**");
        }
        _ => panic!("Expected Bold tokens immediately before and after cursor"),
    }

    // -----------------------------------------------------------------
    // Test 3: Cursor inside inline code:
    // "Run `cargo test` now" with cursor at pos 7 (inside "`cargo test`")
    // Both before and after cursor tokens maintain MarkdownTokenKind::InlineCodeSplitLeft/Right
    // -----------------------------------------------------------------
    let line3 = "Run `cargo test` now";
    let tokens3 = tokenize_markdown_line(line3);
    let segments3 = build_active_line_segments(&tokens3, 7);
    let cursor_idx3 = segments3
        .iter()
        .position(|s| matches!(s, ActiveLineSegment::Cursor))
        .expect("Cursor must be present in segments");
    assert!(cursor_idx3 > 0 && cursor_idx3 < segments3.len() - 1);
    match (&segments3[cursor_idx3 - 1], &segments3[cursor_idx3 + 1]) {
        (ActiveLineSegment::Token(before), ActiveLineSegment::Token(after)) => {
            assert_eq!(before.kind, MarkdownTokenKind::InlineCodeSplitLeft);
            assert_eq!(after.kind, MarkdownTokenKind::InlineCodeSplitRight);
            assert_eq!(before.text, "`ca");
            assert_eq!(after.text, "rgo test`");
        }
        _ => panic!("Expected InlineCodeSplit tokens immediately before and after cursor"),
    }

    // -----------------------------------------------------------------
    // Test 4: Reconstructed text (ignoring Cursor) equals original line text across all cases
    // -----------------------------------------------------------------
    assert_eq!(reconstruct_segments(&segments1), line1);
    assert_eq!(reconstruct_segments(&segments2), line2);
    assert_eq!(reconstruct_segments(&segments3), line3);

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
        let char_len = line.chars().count();
        for col in 0..=char_len {
            let segs = build_active_line_segments(&tokens, col);
            let recon = reconstruct_segments(&segs);
            assert_eq!(
                recon, line,
                "Reconstructed text mismatch for {:?} at col {}",
                line, col
            );
            assert_eq!(
                segs.iter().filter(|s| matches!(s, ActiveLineSegment::Cursor)).count(),
                1,
                "Exactly one cursor expected for {:?} at col {}",
                line, col
            );
        }
    }

    // -----------------------------------------------------------------
    // Test 5: Cursor at start of line (col 0) and at end of line (col == line.len())
    // -----------------------------------------------------------------
    let line5 = "### Title";
    let tokens5 = tokenize_markdown_line(line5);

    // Cursor at start of line (col 0)
    let segments_start = build_active_line_segments(&tokens5, 0);
    assert_eq!(segments_start.first(), Some(&ActiveLineSegment::Cursor));
    assert_eq!(reconstruct_segments(&segments_start), line5);

    // Cursor at end of line (col == line.len())
    let segments_end = build_active_line_segments(&tokens5, line5.len());
    assert_eq!(segments_end.last(), Some(&ActiveLineSegment::Cursor));
    assert_eq!(reconstruct_segments(&segments_end), line5);

    // Cursor on empty line
    let empty_tokens = tokenize_markdown_line("");
    let segments_empty = build_active_line_segments(&empty_tokens, 0);
    assert_eq!(segments_empty.first(), Some(&ActiveLineSegment::Cursor));
    assert_eq!(
        segments_empty.iter().filter(|s| matches!(s, ActiveLineSegment::Cursor)).count(),
        1
    );
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
