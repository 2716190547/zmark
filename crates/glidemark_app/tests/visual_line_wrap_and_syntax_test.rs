use glidemark_app::editor_engine::{
    char_unit_width, physical_to_visual, visual_to_physical, wrap_line_to_visual, EditorEngine,
    VisualLine,
};
use glidemark_app::theme::{ThemeColors, ALL_PALETTES};
use glidemark_app::views::editor::{
    calculate_click_char_col, calculate_editor_left_offset, compute_fenced_code_lines,
    slice_tokens_for_visual_subline, tokenize_code_line, tokenize_markdown_line,
    MarkdownTokenKind,
};
use glidemark_app::views::workspace::calculate_subline_scroll_offset;

#[test]
fn test_visual_line_wrap_cjk_emoji_word_boundary() {
    // 1. char_unit_width verification
    // ASCII 'a', 'Z', '9', ' ' => 1
    assert_eq!(char_unit_width('a'), 1);
    assert_eq!(char_unit_width('Z'), 1);
    assert_eq!(char_unit_width('9'), 1);
    assert_eq!(char_unit_width(' '), 1);

    // CJK '中', '文', '你', '好', 'あ', '한' => 2
    assert_eq!(char_unit_width('中'), 2);
    assert_eq!(char_unit_width('文'), 2);
    assert_eq!(char_unit_width('你'), 2);
    assert_eq!(char_unit_width('好'), 2);
    assert_eq!(char_unit_width('あ'), 2);
    assert_eq!(char_unit_width('한'), 2);

    // Emoji '🦀', '😀' => 2
    assert_eq!(char_unit_width('🦀'), 2);
    assert_eq!(char_unit_width('😀'), 2);

    // Fullwidth '，', '。', '！' => 2
    assert_eq!(char_unit_width('，'), 2);
    assert_eq!(char_unit_width('。'), 2);
    assert_eq!(char_unit_width('！'), 2);

    // Zero-width '\u{200B}', '\u{FEFF}' => 0
    assert_eq!(char_unit_width('\u{200B}'), 0);
    assert_eq!(char_unit_width('\u{FEFF}'), 0);

    // 2. wrap_line_to_visual verification
    // Empty line: 1 visual line with is_first = true, char_len = 0
    let empty = wrap_line_to_visual(0, "", 20);
    assert_eq!(empty.len(), 1);
    assert!(empty[0].is_first);
    assert_eq!(empty[0].subline_idx, 0);
    assert_eq!(empty[0].char_start, 0);
    assert_eq!(empty[0].char_len, 0);

    // Short line fitting max_cols: 1 visual line with is_first = true
    let short_str = "Short line text";
    let short = wrap_line_to_visual(0, short_str, 20);
    assert_eq!(short.len(), 1);
    assert!(short[0].is_first);
    assert_eq!(short[0].subline_idx, 0);
    assert_eq!(short[0].char_start, 0);
    assert_eq!(short[0].char_len, short_str.chars().count());

    // Long English sentence with max_cols = 20: splits into multiple VisualLines
    let long_en = "The quick brown fox jumps over the lazy dog and runs away into the forest.";
    let wrapped_en = wrap_line_to_visual(0, long_en, 20);
    assert!(wrapped_en.len() > 1);
    assert!(wrapped_en[0].is_first);
    assert_eq!(wrapped_en[0].subline_idx, 0);
    for (i, subline) in wrapped_en.iter().enumerate().skip(1) {
        assert!(!subline.is_first);
        assert_eq!(subline.subline_idx, i);
    }
    let total_en_chars: usize = wrapped_en.iter().map(|vl| vl.char_len).sum();
    assert_eq!(total_en_chars, long_en.chars().count());

    // CJK line with max_cols = 10: 5 CJK characters fit in 10 columns (5 * 2 = 10). 6 CJK characters wrap into 2 sublines!
    let cjk_5 = "一二三四五";
    let wrapped_cjk_5 = wrap_line_to_visual(0, cjk_5, 10);
    assert_eq!(wrapped_cjk_5.len(), 1);
    assert_eq!(wrapped_cjk_5[0].char_len, 5);
    assert!(wrapped_cjk_5[0].is_first);

    let cjk_6 = "一二三四五六";
    let wrapped_cjk_6 = wrap_line_to_visual(0, cjk_6, 10);
    assert_eq!(wrapped_cjk_6.len(), 2);
    assert_eq!(wrapped_cjk_6[0].char_len, 5);
    assert!(wrapped_cjk_6[0].is_first);
    assert_eq!(wrapped_cjk_6[1].char_len, 1);
    assert!(!wrapped_cjk_6[1].is_first);

    // Mixed English, CJK, and Emoji with max_cols = 15: wraps cleanly without character truncation or panic
    let mixed = "Hello 你好 🦀 Rust 编程 😀";
    let wrapped_mixed = wrap_line_to_visual(0, mixed, 15);
    assert!(wrapped_mixed.len() >= 2);
    let total_mixed_chars: usize = wrapped_mixed.iter().map(|vl| vl.char_len).sum();
    assert_eq!(total_mixed_chars, mixed.chars().count());
}

#[test]
fn test_bidirectional_coordinate_mapping_and_navigation() {
    let text = "GlideMark is a fast markdown editor with native split view and visual line wrapping support.";
    let vis_lines = wrap_line_to_visual(0, text, 20);
    assert!(vis_lines.len() >= 3);
    let total_len = text.chars().count();

    // Verify roundtrip mapping:
    // physical_to_visual -> visual_to_physical
    for phys_col in 0..=total_len {
        let (vl_idx, vl_col) = physical_to_visual(0, phys_col, &vis_lines);
        let (back_phys_line, back_phys_col) = visual_to_physical(vl_idx, vl_col, &vis_lines);
        assert_eq!(0, back_phys_line);
        assert_eq!(phys_col, back_phys_col);
    }

    // Visual Cursor Navigation:
    // EditorEngine::move_up_visual and EditorEngine::move_down_visual
    let mut editor = EditorEngine::new("The quick brown fox jumps over the lazy dog and runs away into the forest.");
    let max_cols = 20;
    let vis = editor.get_visual_lines(max_cols);
    assert!(vis.len() >= 3);

    // Cursor starts at top subline (line 0, col 0)
    editor.set_cursor(0, 0);
    assert_eq!(editor.cursor_line, 0);
    assert_eq!(editor.cursor_col, 0);

    // Move down into the second visual subline of the same physical line!
    editor.move_down_visual(max_cols);
    assert_eq!(editor.cursor_line, 0);
    assert_eq!(editor.cursor_col, vis[1].char_start);

    // Move down into the third visual subline
    editor.move_down_visual(max_cols);
    assert_eq!(editor.cursor_line, 0);
    assert_eq!(editor.cursor_col, vis[2].char_start);

    // Moving up returns to the second visual subline
    editor.move_up_visual(max_cols);
    assert_eq!(editor.cursor_line, 0);
    assert_eq!(editor.cursor_col, vis[1].char_start);

    // Moving up again returns to the first visual subline!
    editor.move_up_visual(max_cols);
    assert_eq!(editor.cursor_line, 0);
    assert_eq!(editor.cursor_col, 0);
}

#[test]
fn test_visual_line_cache_invalidation_boundaries() {
    let mut editor = EditorEngine::new("first line\nhello world long line here wrapping\nthird line");
    let rev0 = editor.revision;
    assert_eq!(rev0, 0);

    // First computation caches result: cache.is_cached(revision, max_cols) is true.
    let lines1 = editor.get_visual_lines(15);
    assert!(editor.visual_cache.is_cached(rev0, 15));

    // Same revision and max_cols returns cached Arc without recomputation.
    let lines2 = editor.get_visual_lines(15);
    assert!(std::sync::Arc::ptr_eq(&lines1, &lines2));

    // Changing revision (e.g. editor.insert_text("abc") increments revision) causes cache to recompute.
    editor.insert_text("abc");
    assert_eq!(editor.revision, rev0 + 1);
    assert!(!editor.visual_cache.is_cached(editor.revision, 15));

    let lines3 = editor.get_visual_lines(15);
    assert!(!std::sync::Arc::ptr_eq(&lines1, &lines3));
    assert!(editor.visual_cache.is_cached(editor.revision, 15));

    // Changing max_cols causes cache to recompute.
    assert!(!editor.visual_cache.is_cached(editor.revision, 25));
    let lines4 = editor.get_visual_lines(25);
    assert!(!std::sync::Arc::ptr_eq(&lines3, &lines4));
    assert!(editor.visual_cache.is_cached(editor.revision, 25));

    // cache.invalidate() clears the cache.
    editor.visual_cache.invalidate();
    assert!(!editor.visual_cache.is_cached(editor.revision, 25));
}

#[test]
fn test_fenced_code_context_and_syntax_highlighting() {
    // 1. Test glidemark_app::views::editor::compute_fenced_code_lines
    let markdown_lines = vec![
        "# Title".to_string(),
        "```rust".to_string(),
        "fn main() {".to_string(),
        "    println!(\"hello\");".to_string(),
        "}".to_string(),
        "```".to_string(),
        "Outside text".to_string(),
    ];
    let fenced = compute_fenced_code_lines(&markdown_lines);
    assert_eq!(fenced.len(), 7);
    assert_eq!(fenced[0], false, "Line 0 (# Title) => false");
    assert_eq!(fenced[1], true, "Line 1 (```rust) => true");
    assert_eq!(fenced[2], true, "Line 2 (fn main()...) => true");
    assert_eq!(fenced[3], true, "Line 3 (println...) => true");
    assert_eq!(fenced[4], true, "Line 4 (}} ) => true");
    assert_eq!(fenced[5], true, "Line 5 (```) => true");
    assert_eq!(fenced[6], false, "Line 6 (Outside text) => false");

    // 2. Test glidemark_app::views::editor::tokenize_code_line
    let code_line = "fn main() { let x = \"hello\"; // comment }";
    let tokens = tokenize_code_line(code_line);

    // Verify Keyword ("fn", "let"), StringLiteral ("\"hello\""), Comment ("// comment }")
    let has_fn_kw = tokens
        .iter()
        .any(|t| t.kind == MarkdownTokenKind::Keyword && t.text == "fn");
    let has_let_kw = tokens
        .iter()
        .any(|t| t.kind == MarkdownTokenKind::Keyword && t.text == "let");
    let has_str = tokens
        .iter()
        .any(|t| t.kind == MarkdownTokenKind::StringLiteral && t.text == "\"hello\"");
    let has_comment = tokens
        .iter()
        .any(|t| t.kind == MarkdownTokenKind::Comment && t.text.contains("// comment"));

    assert!(has_fn_kw, "Should tokenize 'fn' as Keyword");
    assert!(has_let_kw, "Should tokenize 'let' as Keyword");
    assert!(has_str, "Should tokenize '\"hello\"' as StringLiteral");
    assert!(has_comment, "Should tokenize '// comment }}' as Comment");

    // 3. Test 8 theme palettes have distinct syntax colors:
    // syntax_keyword, syntax_string, syntax_comment, syntax_heading, syntax_inline_code, syntax_link
    assert_eq!(ALL_PALETTES.len(), 8);
    for palette in ALL_PALETTES {
        for is_dark in [false, true] {
            let theme = ThemeColors::resolve(palette.id, is_dark);
            // Verify all syntax fields are defined and valid alpha (a > 0.0)
            assert!(theme.syntax_keyword.a > 0.0);
            assert!(theme.syntax_string.a > 0.0);
            assert!(theme.syntax_comment.a > 0.0);
            assert!(theme.syntax_heading.a > 0.0);
            assert!(theme.syntax_inline_code.a > 0.0);
            assert!(theme.syntax_link.a > 0.0);

            // Verify they have distinct syntax colors (keyword != comment, string != comment)
            assert_ne!(
                (theme.syntax_keyword.h, theme.syntax_keyword.l),
                (theme.syntax_comment.h, theme.syntax_comment.l),
                "Palette {} dark={} keyword should differ from comment",
                palette.id,
                is_dark
            );
            assert_ne!(
                (theme.syntax_string.h, theme.syntax_string.l),
                (theme.syntax_comment.h, theme.syntax_comment.l),
                "Palette {} dark={} string should differ from comment",
                palette.id,
                is_dark
            );
        }
    }
}

#[test]
fn test_split_view_scroll_wheel_physical_line_lookup() {
    // Simulate a document where physical line 0 wraps into 3 visual lines (indices 0, 1, 2),
    // and physical line 1 wraps into 2 visual lines (indices 3, 4).
    let visual_lines = vec![
        VisualLine {
            physical_line_idx: 0,
            subline_idx: 0,
            char_start: 0,
            char_len: 20,
            is_first: true,
        },
        VisualLine {
            physical_line_idx: 0,
            subline_idx: 1,
            char_start: 20,
            char_len: 20,
            is_first: false,
        },
        VisualLine {
            physical_line_idx: 0,
            subline_idx: 2,
            char_start: 40,
            char_len: 15,
            is_first: false,
        },
        VisualLine {
            physical_line_idx: 1,
            subline_idx: 0,
            char_start: 0,
            char_len: 25,
            is_first: true,
        },
        VisualLine {
            physical_line_idx: 1,
            subline_idx: 1,
            char_start: 25,
            char_len: 10,
            is_first: false,
        },
    ];

    let lookup_physical_line = |vis_idx: usize| -> usize {
        visual_lines
            .get(vis_idx)
            .map(|vl| vl.physical_line_idx + 1)
            .unwrap_or(1)
    };

    assert_eq!(lookup_physical_line(0), 1);
    assert_eq!(lookup_physical_line(1), 1);
    assert_eq!(lookup_physical_line(2), 1);
    assert_eq!(lookup_physical_line(3), 2);
    assert_eq!(lookup_physical_line(4), 2);
    // Boundary out-of-range fallback
    assert_eq!(lookup_physical_line(999), 1);
}

#[test]
fn test_editor_horizontal_scroll_disabled_contract() {
    let editor_src = include_str!("../src/views/editor.rs");
    assert!(
        !editor_src.contains("ListHorizontalSizingBehavior::Unconstrained"),
        "editor.rs must not contain ListHorizontalSizingBehavior::Unconstrained"
    );
    assert!(
        !editor_src.contains(".with_horizontal_sizing_behavior("),
        "editor.rs must not contain .with_horizontal_sizing_behavior("
    );
    assert!(
        !editor_src.contains("whitespace_nowrap()"),
        "editor.rs must not contain whitespace_nowrap()"
    );

    let settings_src = include_str!("../src/views/settings.rs");
    assert!(
        !settings_src.contains(".border_1()"),
        "settings.rs must have zero occurrences of .border_1()"
    );
}

#[test]
fn test_cjk_emoji_click_char_col_and_drawer_offset() {
    let font_size = 14.0;

    // 1. Drawer offset contract:
    // When drawer is open: 240px drawer + 68px editor gutter/padding = 308px
    assert_eq!(calculate_editor_left_offset(true), 308.0);
    // When drawer is collapsed: 68px
    assert_eq!(calculate_editor_left_offset(false), 68.0);

    // 2. ASCII click hit test:
    let ascii_str = "abcdef";
    assert_eq!(calculate_click_char_col(ascii_str, 0.0, font_size), 0);
    assert_eq!(calculate_click_char_col(ascii_str, 4.0, font_size), 0);
    assert_eq!(calculate_click_char_col(ascii_str, 5.0, font_size), 1);
    assert_eq!(calculate_click_char_col(ascii_str, 200.0, font_size), 6);

    // 3. CJK click hit test: each CJK char has unit width 2 (16.8px)
    let cjk_str = "你好世界";
    assert_eq!(calculate_click_char_col(cjk_str, 5.0, font_size), 0);
    assert_eq!(calculate_click_char_col(cjk_str, 10.0, font_size), 1);
    assert_eq!(calculate_click_char_col(cjk_str, 20.0, font_size), 1);
    assert_eq!(calculate_click_char_col(cjk_str, 30.0, font_size), 2);
    assert_eq!(calculate_click_char_col(cjk_str, 40.0, font_size), 2);
    assert_eq!(calculate_click_char_col(cjk_str, 50.0, font_size), 3);
    assert_eq!(calculate_click_char_col(cjk_str, 60.0, font_size), 4);
    assert_eq!(calculate_click_char_col(cjk_str, 100.0, font_size), 4);

    // 4. Emoji click hit test: 🦀 is 2 units, 'R', 'u', 's', 't' are 1 unit each
    let emoji_str = "🦀Rust";
    assert_eq!(calculate_click_char_col(emoji_str, 0.0, font_size), 0);
    assert_eq!(calculate_click_char_col(emoji_str, 8.0, font_size), 0);
    assert_eq!(calculate_click_char_col(emoji_str, 9.0, font_size), 1);
    assert_eq!(calculate_click_char_col(emoji_str, 18.0, font_size), 1);
    assert_eq!(calculate_click_char_col(emoji_str, 22.0, font_size), 2);
}

#[test]
fn test_cross_visual_line_syntax_inheritance_and_subline_scroll() {
    // 1. Cross-visual-line bold inheritance:
    let line = "**bold text that wraps across lines**";
    let phys_tokens = tokenize_markdown_line(line);
    assert_eq!(phys_tokens.len(), 1);
    assert_eq!(phys_tokens[0].kind, MarkdownTokenKind::Bold);

    // Subline 0: first 15 chars "**bold text tha"
    let sub0 = slice_tokens_for_visual_subline(&phys_tokens, 0, 15);
    assert_eq!(sub0.len(), 1);
    assert_eq!(sub0[0].kind, MarkdownTokenKind::Bold);
    assert_eq!(sub0[0].text, "**bold text tha");

    // Subline 1: remaining chars "t wraps across lines**"
    let sub1 = slice_tokens_for_visual_subline(&phys_tokens, 15, line.chars().count() - 15);
    assert_eq!(sub1.len(), 1);
    assert_eq!(sub1[0].kind, MarkdownTokenKind::Bold);
    assert_eq!(sub1[0].text, "t wraps across lines**");

    // 2. Cross-visual-line heading inheritance:
    let heading_line = "# Very long heading that wraps onto multiple visual lines";
    let head_tokens = tokenize_markdown_line(heading_line);
    assert_eq!(head_tokens.len(), 2);
    assert_eq!(head_tokens[0].kind, MarkdownTokenKind::HeadingPrefix);
    assert_eq!(head_tokens[1].kind, MarkdownTokenKind::HeadingText);

    // Subline 0: covers "# Very long heading" (char 0 to 19)
    let head_sub0 = slice_tokens_for_visual_subline(&head_tokens, 0, 19);
    assert_eq!(head_sub0.len(), 2);
    assert_eq!(head_sub0[0].kind, MarkdownTokenKind::HeadingPrefix);
    assert_eq!(head_sub0[1].kind, MarkdownTokenKind::HeadingText);

    // Subline 1: covers " that wraps onto multiple visual lines" (char 19 to end)
    let head_sub1 = slice_tokens_for_visual_subline(&head_tokens, 19, heading_line.chars().count() - 19);
    assert_eq!(head_sub1.len(), 1);
    assert_eq!(head_sub1[0].kind, MarkdownTokenKind::HeadingText);

    // 3. Subline scroll progress calculation:
    let offset_sub0 = calculate_subline_scroll_offset(0.0, 0, 5);
    let offset_sub1 = calculate_subline_scroll_offset(0.0, 1, 5);
    let offset_sub2 = calculate_subline_scroll_offset(0.0, 2, 5);
    let offset_sub3 = calculate_subline_scroll_offset(0.0, 3, 5);
    let offset_sub4 = calculate_subline_scroll_offset(0.0, 4, 5);

    assert_eq!(offset_sub0, 0.0);
    assert!(offset_sub1 > offset_sub0);
    assert!(offset_sub2 > offset_sub1);
    assert!(offset_sub3 > offset_sub2);
    assert!(offset_sub4 > offset_sub3);
    assert!(offset_sub4 <= 100.0);
}
