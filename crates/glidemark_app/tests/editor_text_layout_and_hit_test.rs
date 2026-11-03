use glidemark_app::editor_engine::{wrap_line_to_visual, wrap_line_to_visual_with_tab_width};
use glidemark_app::views::editor::{
    calculate_click_char_col_tokens, calculate_cursor_x_offset_layout,
    calculate_cursor_x_offset_tokens, slice_tokens_for_visual_subline, token_paddings,
    tokenize_markdown_line, wrap_line_to_visual_layout, MarkdownTokenKind,
};
use gpui::{div, IntoElement, Render, TestAppContext, Window};

struct TestView;

impl Render for TestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

#[gpui::test]
async fn test_reviewer_failure_replication_inline_code_padding_and_hit_test(
    cx: &mut TestAppContext,
) {
    let (_view, cx) = cx.add_window_view(|_window, _cx| TestView);
    cx.run_until_parked();

    cx.update(|window, _app| {
        let text_system = window.text_system();
        let font_size = 14.0;

        // Line with inline code: "abc `code` def"
        let full_line = "abc `code` def";
        let tokens = tokenize_markdown_line(full_line);

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].text, "abc ");
        assert_eq!(tokens[0].kind, MarkdownTokenKind::Plain);
        assert_eq!(tokens[1].text, "`code`");
        assert_eq!(tokens[1].kind, MarkdownTokenKind::InlineCode);
        assert_eq!(tokens[2].text, " def");
        assert_eq!(tokens[2].kind, MarkdownTokenKind::Plain);

        // Character indices:
        // "abc " = chars 0..4 (len 4)
        // "`code`" = chars 4..10 (len 6)
        // " def" = chars 10..14 (len 4)

        // 1. Replicate reviewer's exact reproduction:
        // Plain text shaping shapes the string without pill padding.
        // In monospace 14px, each char advance is 8.4px.
        // At col 10 (the start of " def", right after the inline code):
        let plain_x_at_10 =
            calculate_cursor_x_offset_layout(text_system, full_line, 10, font_size);
        let token_x_at_10 =
            calculate_cursor_x_offset_tokens(text_system, &tokens, 10, font_size);

        // Token 0 text: 4 chars * 8.4 = 33.6px
        // Token 1: 4.0px left pad + 6 chars * 8.4 (50.4px) + 4.0px right pad = 58.4px
        // Expected token_x_at_10 = 33.6 + 58.4 = 92.0px.
        // Plain text at col 10: 10 chars * 8.4 = 84.0px.
        // The drift is EXACTLY 8.0px (4.0px left + 4.0px right padding)!
        let drift = token_x_at_10 - plain_x_at_10;
        assert!(
            (drift - 8.0).abs() < 1e-3,
            "Drift between plain text and token-aware layout must be exactly 8.0px, got {}",
            drift
        );
        assert!((plain_x_at_10 - 84.0).abs() < 1e-3);
        assert!((token_x_at_10 - 92.0).abs() < 1e-3);

        // Inside the inline code at char col 9 (the last '`' character):
        let plain_x_at_9 = calculate_cursor_x_offset_layout(text_system, full_line, 9, font_size);
        let token_x_at_9 = calculate_cursor_x_offset_tokens(text_system, &tokens, 9, font_size);
        // Col 9 has only the left padding (4.0px), right padding has not been added yet.
        assert!((plain_x_at_9 - 75.6).abs() < 1e-3);
        assert!((token_x_at_9 - 79.6).abs() < 1e-3);
        assert!(
            (token_x_at_9 - plain_x_at_9 - 4.0).abs() < 1e-3,
            "Inside inline code, cursor must account for 4.0px left padding"
        );

        // 2. Click hit testing across the inline code pill without 8px drift:
        // Click at x = 0.0 -> hits 'a' (col 0)
        assert_eq!(
            calculate_click_char_col_tokens(text_system, &tokens, 0.0, font_size),
            0
        );

        // Click at x = 35.0 (inside Token 1's left padding, 33.6..37.6px) -> hits col 4 (start of code)
        assert_eq!(
            calculate_click_char_col_tokens(text_system, &tokens, 35.0, font_size),
            4
        );

        // Click at x = 90.0 (inside Token 1's right padding, 88.0..92.0px) -> hits col 10 (start of " def")
        assert_eq!(
            calculate_click_char_col_tokens(text_system, &tokens, 90.0, font_size),
            10
        );

        // Click at x = 92.5 (first char ' ' of " def") -> hits col 10
        assert_eq!(
            calculate_click_char_col_tokens(text_system, &tokens, 92.5, font_size),
            10
        );

        // Click past the end of the line
        assert_eq!(
            calculate_click_char_col_tokens(text_system, &tokens, 500.0, font_size),
            14
        );

        // 3. Strict Reciprocity: every character column 0..=14 must map to an x offset,
        // and clicking at (x + 0.1) must resolve back to that exact same column!
        for col in 0..=14 {
            let x = calculate_cursor_x_offset_tokens(text_system, &tokens, col, font_size);
            let hit = calculate_click_char_col_tokens(text_system, &tokens, x + 0.1, font_size);
            assert_eq!(
                hit, col,
                "Reciprocity failure: col {} at x={} hit back as {}",
                col, x, hit
            );
        }
    });
}

#[gpui::test]
async fn test_tab_actual_advance_and_layout_wrapping(cx: &mut TestAppContext) {
    let (_view, cx) = cx.add_window_view(|_window, _cx| TestView);
    cx.run_until_parked();

    cx.update(|window, _app| {
        let text_system = window.text_system();
        let font_size = 14.0;
        let font = gpui::font("ui-monospace");
        let font_size_px = gpui::px(font_size);

        // 1. Verify GPUI font shaping actual advance for '\t'
        let run_tab = gpui::TextRun {
            len: 1,
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped_tab = text_system.shape_line(
            gpui::SharedString::from("\t".to_string()),
            font_size_px,
            &[run_tab],
            None,
        );

        let run_space = gpui::TextRun {
            len: 1,
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped_space = text_system.shape_line(
            gpui::SharedString::from(" ".to_string()),
            font_size_px,
            &[run_space],
            None,
        );

        let tab_w = f32::from(shaped_tab.width);
        let space_w = f32::from(shaped_space.width);

        // In GPUI HarfBuzz shaping, '\t' actual advance is 8.4px (1 space advance), NOT 33.6px!
        assert!(
            (tab_w - space_w).abs() < 1e-3,
            "GPUI font shaping '\t' advance ({}) must match space advance ({})",
            tab_w,
            space_w
        );
        assert_eq!(tab_w, 8.4);

        // 2. Tab measurement in layout wrapping vs character column wrapping
        let line_with_tabs = "a\tb\tc\td\te";
        let tokens = tokenize_markdown_line(line_with_tabs);

        // In character units with char_unit_width('\t') = 4:
        // 5 letters (5) + 4 tabs (16) = 21 columns
        let col_wrapped = wrap_line_to_visual(0, line_with_tabs, 10);
        assert!(
            col_wrapped.len() > 1,
            "Column wrapping overestimates tabs as 4 columns each"
        );

        // In layout wrapping with actual GPUI font metrics:
        // 5 letters (5 * 8.4) + 4 tabs (4 * 8.4) = 9 * 8.4 = 75.6px
        // With max_width_px = 80.0px, all 9 characters fit in 1 visual line!
        let layout_wrapped =
            wrap_line_to_visual_layout(text_system, 0, line_with_tabs, &tokens, 80.0, font_size);
        assert_eq!(
            layout_wrapped.len(), 1,
            "Layout wrapping must use actual 8.4px advance for tabs and fit in 1 line"
        );
        assert_eq!(layout_wrapped[0].char_len, line_with_tabs.chars().count());

        // 3. Editor engine measured wrapping
        let engine_tab1 = wrap_line_to_visual_with_tab_width(0, line_with_tabs, 10, 1);
        assert_eq!(
            engine_tab1.len(), 1,
            "wrap_line_to_visual_with_tab_width(1) must fit in 10 cols"
        );
    });
}

#[gpui::test]
async fn test_cjk_mixed_text_and_multiple_inline_code_tokens(cx: &mut TestAppContext) {
    let (_view, cx) = cx.add_window_view(|_window, _cx| TestView);
    cx.run_until_parked();

    cx.update(|window, _app| {
        let text_system = window.text_system();
        let font_size = 14.0;

        // Line with CJK and two inline code pills:
        // "前缀 `alpha` 桥接 `beta` 后缀"
        let line = "前缀 `alpha` 桥接 `beta` 后缀";
        let tokens = tokenize_markdown_line(line);

        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[1].kind, MarkdownTokenKind::InlineCode);
        assert_eq!(tokens[3].kind, MarkdownTokenKind::InlineCode);

        // Two inline code pills => 2 * 8.0px = 16.0px total padding added
        let total_chars = line.chars().count();
        let plain_x_end =
            calculate_cursor_x_offset_layout(text_system, line, total_chars, font_size);
        let token_x_end =
            calculate_cursor_x_offset_tokens(text_system, &tokens, total_chars, font_size);
        let total_drift = token_x_end - plain_x_end;
        assert!(
            (total_drift - 16.0).abs() < 1e-3,
            "Two inline code pills must add exactly 16.0px padding, got {}",
            total_drift
        );

        // Full reciprocity across every character of the CJK + multiple inline code line
        for col in 0..=total_chars {
            let x = calculate_cursor_x_offset_tokens(text_system, &tokens, col, font_size);
            let hit = calculate_click_char_col_tokens(text_system, &tokens, x + 0.1, font_size);
            assert_eq!(
                hit, col,
                "CJK + multi-code reciprocity failure at col {}: x={}, hit={}",
                col, x, hit
            );
        }

        // Token split variants verification:
        // Subline slicing splits an inline code token cleanly into split variants
        // "前缀 `alpha`" = 10 chars. Let's slice right inside "`alpha`" (sub_start=0, sub_len=6)
        let sub0 = slice_tokens_for_visual_subline(&tokens, 0, 6);
        assert_eq!(sub0.len(), 2);
        assert_eq!(sub0[0].text, "前缀 ");
        assert_eq!(sub0[1].text, "`al");
        assert_eq!(sub0[1].kind, MarkdownTokenKind::InlineCodeSplitLeft);
        assert_eq!(token_paddings(&sub0[1].kind), (4.0, 0.0));

        let sub1 = slice_tokens_for_visual_subline(&tokens, 6, total_chars - 6);
        assert_eq!(sub1[0].text, "pha`");
        assert_eq!(sub1[0].kind, MarkdownTokenKind::InlineCodeSplitRight);
        assert_eq!(token_paddings(&sub1[0].kind), (0.0, 4.0));
    });
}
