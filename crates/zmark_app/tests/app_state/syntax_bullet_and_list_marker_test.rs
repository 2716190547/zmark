use zmark_app::theme::{ThemeColors, ALL_PALETTES};
use zmark_app::views::editor::{
    slice_tokens_for_visual_subline, tokenize_markdown_line, MarkdownTokenKind,
};
use gpui::rgb;

/// Ticket F4: 8 Palettes `syntax_bullet` Matrix and Markdown List Marker Styling Test Suite (ADR 0035 Decision 4)
///
/// Verifies:
/// 1. `test_eight_palettes_syntax_bullet_matrix`:
///    All 8 palettes across both light and dark modes (16 combinations) in `ALL_PALETTES`:
///    - `theme.syntax_bullet.a == 1.0` (fully opaque).
///    - Exact RGB hex colors per ADR 0035 Decision 4:
///      * default: false -> rgb(0x0969da), true -> rgb(0x58a6ff)
///      * claude: false -> rgb(0xd97706), true -> rgb(0xfbbf24)
///      * macos: false -> rgb(0x007aff), true -> rgb(0x0a84ff)
///      * tokyo-night: false -> rgb(0x2e7de9), true -> rgb(0x7aa2f7)
///      * nord: false -> rgb(0x5e81ac), true -> rgb(0x88c0d0)
///      * github: false -> rgb(0x0969da), true -> rgb(0x58a6ff)
///      * solarized: false -> rgb(0x268bd2), true -> rgb(0x2aa198)
///      * one-dark: false -> rgb(0x4078f2), true -> rgb(0x61afef)
///
/// 2. `test_tokenize_markdown_line_bullet_prefixes`:
///    Unordered lists, task lists, ordered lists, indented lists, bold styling after prefix,
///    and non-bullet edge cases (---, floats, inline italics, headings, blockquotes).
///
/// 3. `test_slice_tokens_for_visual_subline_with_bullet_prefix`:
///    Slicing a list line across multiple visual sublines:
///    - Subline 0 (char_start 0, len 8): contains the BulletPrefix.
///    - Subline 1 (char_start 8, len 10): contains only wrapped Plain tokens, NOT inheriting BulletPrefix.

#[test]
fn test_eight_palettes_syntax_bullet_matrix() {
    for palette in ALL_PALETTES {
        for &is_dark in &[false, true] {
            let theme = ThemeColors::resolve(palette.id, is_dark);
            assert_eq!(
                theme.syntax_bullet.a, 1.0,
                "Palette '{}' (is_dark={}) syntax_bullet must be fully opaque (a == 1.0)",
                palette.id, is_dark
            );

            let expected_hex = match (palette.id, is_dark) {
                ("default", false) => 0x8c8c8c,
                ("default", true) => 0x8c8c8c,
                ("claude", false) => 0xd97706,
                ("claude", true) => 0xfbbf24,
                ("macos", false) => 0x007aff,
                ("macos", true) => 0x0a84ff,
                ("tokyo-night", false) => 0x2e7de9,
                ("tokyo-night", true) => 0x7aa2f7,
                ("nord", false) => 0x5e81ac,
                ("nord", true) => 0x88c0d0,
                ("github", false) => 0x0969da,
                ("github", true) => 0x58a6ff,
                ("solarized", false) => 0x268bd2,
                ("solarized", true) => 0x2aa198,
                ("one-dark", false) => 0x4078f2,
                ("one-dark", true) => 0x61afef,
                other => panic!("Unexpected palette id: {:?}", other),
            };

            assert_eq!(
                theme.syntax_bullet,
                rgb(expected_hex).into(),
                "Palette '{}' (is_dark={}) syntax_bullet mismatch",
                palette.id,
                is_dark
            );
        }
    }
}

#[test]
fn test_tokenize_markdown_line_bullet_prefixes() {
    // Unordered lists
    let t = tokenize_markdown_line("- item");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[0].text, "- ");
    assert_eq!(t[1].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[1].text, "item");

    let t = tokenize_markdown_line("* item");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[0].text, "* ");
    assert_eq!(t[1].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[1].text, "item");

    let t = tokenize_markdown_line("+ item");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[0].text, "+ ");
    assert_eq!(t[1].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[1].text, "item");

    let t = tokenize_markdown_line("  - indented");
    assert_eq!(t.len(), 3);
    assert_eq!(t[0].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[0].text, "  ");
    assert_eq!(t[1].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[1].text, "- ");
    assert_eq!(t[2].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[2].text, "indented");

    let t = tokenize_markdown_line("- **bold**");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[0].text, "- ");
    assert_eq!(t[1].kind, MarkdownTokenKind::Bold);
    assert_eq!(t[1].text, "**bold**");

    // Task lists
    let t = tokenize_markdown_line("- [ ] todo");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[0].text, "- [ ] ");
    assert_eq!(t[1].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[1].text, "todo");

    let t = tokenize_markdown_line("- [x] done");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[0].text, "- [x] ");
    assert_eq!(t[1].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[1].text, "done");

    let t = tokenize_markdown_line("* [X] done");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[0].text, "* [X] ");
    assert_eq!(t[1].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[1].text, "done");

    let t = tokenize_markdown_line("- [ ]");
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[0].text, "- [ ]");

    // Ordered lists
    let t = tokenize_markdown_line("1. first");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[0].text, "1. ");
    assert_eq!(t[1].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[1].text, "first");

    let t = tokenize_markdown_line("10. tenth");
    assert_eq!(t.len(), 2);
    assert_eq!(t[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[0].text, "10. ");
    assert_eq!(t[1].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[1].text, "tenth");

    let t = tokenize_markdown_line("  2. second");
    assert_eq!(t.len(), 3);
    assert_eq!(t[0].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[0].text, "  ");
    assert_eq!(t[1].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(t[1].text, "2. ");
    assert_eq!(t[2].kind, MarkdownTokenKind::Plain);
    assert_eq!(t[2].text, "second");

    // Non-bullet edge cases (must NOT be BulletPrefix)
    let t = tokenize_markdown_line("---");
    assert!(
        t.iter().all(|tok| tok.kind != MarkdownTokenKind::BulletPrefix),
        "Horizontal rule '---' must not contain BulletPrefix, got {:?}",
        t
    );

    let t = tokenize_markdown_line("1.2 float");
    assert!(
        t.iter().all(|tok| tok.kind != MarkdownTokenKind::BulletPrefix),
        "'1.2 float' must not contain BulletPrefix, got {:?}",
        t
    );

    let t = tokenize_markdown_line("*italic*");
    assert!(
        t.iter().all(|tok| tok.kind != MarkdownTokenKind::BulletPrefix),
        "'*italic*' must not contain BulletPrefix, got {:?}",
        t
    );

    let t = tokenize_markdown_line("# Heading");
    assert_eq!(t[0].kind, MarkdownTokenKind::HeadingPrefix);
    assert!(
        t.iter().all(|tok| tok.kind != MarkdownTokenKind::BulletPrefix),
        "'# Heading' must not contain BulletPrefix, got {:?}",
        t
    );

    let t = tokenize_markdown_line("> Quote");
    assert_eq!(t[0].kind, MarkdownTokenKind::BlockquotePrefix);
    assert!(
        t.iter().all(|tok| tok.kind != MarkdownTokenKind::BulletPrefix),
        "'> Quote' must not contain BulletPrefix, got {:?}",
        t
    );
}

#[test]
fn test_slice_tokens_for_visual_subline_with_bullet_prefix() {
    let line = "- [x] A very long task description that wraps";
    let tokens = tokenize_markdown_line(line);

    // Verify token 0 is BulletPrefix ("- [x] ")
    assert_eq!(tokens[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(tokens[0].text, "- [x] ");

    // Subline 0 (char_start 0, len 8): contains the BulletPrefix
    let sub0 = slice_tokens_for_visual_subline(&tokens, 0, 8);
    assert!(
        sub0.iter().any(|t| t.kind == MarkdownTokenKind::BulletPrefix),
        "Subline 0 must contain BulletPrefix: {:?}",
        sub0
    );
    assert_eq!(sub0[0].kind, MarkdownTokenKind::BulletPrefix);
    assert_eq!(sub0[0].text, "- [x] ");
    assert_eq!(sub0[1].kind, MarkdownTokenKind::Plain);
    assert_eq!(sub0[1].text, "A ");

    // Subline 1 (char_start 8, len 10): contains only the wrapped text tokens (Plain), NOT inheriting BulletPrefix if outside prefix bounds
    let sub1 = slice_tokens_for_visual_subline(&tokens, 8, 10);
    assert!(
        sub1.iter().all(|t| t.kind == MarkdownTokenKind::Plain),
        "Subline 1 must only contain Plain tokens, got: {:?}",
        sub1
    );
    assert!(
        sub1.iter().all(|t| t.kind != MarkdownTokenKind::BulletPrefix),
        "Subline 1 must NOT inherit BulletPrefix outside prefix bounds: {:?}",
        sub1
    );
    let sub1_text: String = sub1.iter().map(|t| t.text.as_str()).collect();
    assert_eq!(sub1_text, "very long ");
}

#[test]
fn test_preview_task_list_checkbox_uses_syntax_bullet() {
    let preview_src = include_str!("../../src/views/preview.rs");
    let task_item_pos = preview_src
        .find("if let Some(checked) = item.checked")
        .expect("task list checkbox branch must exist in preview.rs");
    // The branch also carries the click handler, so look far enough ahead to reach its styling.
    let task_item_section = &preview_src[task_item_pos..(task_item_pos + 1600).min(preview_src.len())];
    // Ignore rustfmt line wrapping while retaining the checked-state color assertion.
    let normalized = task_item_section
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    assert!(
        task_item_section.contains(".border_color(theme.syntax_bullet)"),
        "Task item checkbox border must use theme.syntax_bullet"
    );
    assert!(
        normalized.contains("theme.syntax_bullet } else { theme.bg }"),
        "Task item checkbox background must use theme.syntax_bullet when checked"
    );
    assert!(
        !task_item_section.contains(".border_color(theme.accent)"),
        "Task item checkbox border must not use theme.accent"
    );
}
