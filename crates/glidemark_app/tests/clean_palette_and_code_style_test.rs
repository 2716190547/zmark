use glidemark_app::theme::{ThemeColors, ALL_PALETTES};
use gpui::rgb;

/// Ticket E1: Clean Palette & Code Style Test Suite (ADR 0034)
///
/// Verifies:
/// 1. Default palette neutral colors (pure clean stone base, deep carbon text, crisp light code_bg).
/// 2. 8 palettes × 6 syntax tokens matrix (opacity, representative palette tokens match ADR 0034).
/// 3. Canvas header removes dirty red dot indicator.
/// 4. Preview code syntax styling contract (InlineItem::Code uses syntax_inline_code, code block bold tag).

#[test]
fn test_default_palette_neutral_colors() {
    let light = ThemeColors::default_palette(false);
    assert_eq!(
        light.text_primary,
        rgb(0x1f2328).into(),
        "Default light text_primary must be #1f2328 (Deep carbon gray)"
    );
    assert_eq!(
        light.code_bg,
        rgb(0xf6f8fa).into(),
        "Default light code_bg must be #f6f8fa (Clean cool light gray)"
    );
    assert_eq!(
        light.warm_pill_bg(),
        rgb(0xeff1f4).into(),
        "Default light warm_pill_bg must be #eff1f4"
    );
    assert_eq!(
        light.warm_pill_text(),
        rgb(0x1f2328).into(),
        "Default light warm_pill_text must be #1f2328"
    );

    let dark = ThemeColors::default_palette(true);
    assert_eq!(
        dark.syntax_inline_code,
        rgb(0xf3f1ed).into(),
        "Default dark syntax_inline_code must be #f3f1ed"
    );
}

#[test]
fn test_eight_palettes_six_syntax_tokens_matrix() {
    // Assert all 8 palettes across both light and dark modes have fully opaque syntax tokens
    for palette in ALL_PALETTES {
        for &is_dark in &[false, true] {
            let theme = ThemeColors::resolve(palette.id, is_dark);
            let tokens = [
                ("syntax_keyword", theme.syntax_keyword),
                ("syntax_string", theme.syntax_string),
                ("syntax_comment", theme.syntax_comment),
                ("syntax_heading", theme.syntax_heading),
                ("syntax_inline_code", theme.syntax_inline_code),
                ("syntax_link", theme.syntax_link),
            ];

            for (token_name, token_color) in tokens {
                assert_eq!(
                    token_color.a, 1.0,
                    "Palette '{}' (is_dark={}) token '{}' must be fully opaque (a == 1.0)",
                    palette.id, is_dark, token_name
                );
            }
        }
    }

    // Specifically verify representative palette values from ADR 0034 matrix:

    // 1. default light
    let def_light = ThemeColors::resolve("default", false);
    assert_eq!(def_light.syntax_keyword, rgb(0x8250df).into(), "default light syntax_keyword");
    assert_eq!(def_light.syntax_string, rgb(0x1a7f37).into(), "default light syntax_string");
    assert_eq!(def_light.syntax_comment, rgb(0x6e7781).into(), "default light syntax_comment");
    assert_eq!(def_light.syntax_heading, rgb(0x0969da).into(), "default light syntax_heading");
    assert_eq!(def_light.syntax_inline_code, rgb(0x1f2328).into(), "default light syntax_inline_code");
    assert_eq!(def_light.syntax_link, rgb(0x0969da).into(), "default light syntax_link");

    // 2. default dark
    let def_dark = ThemeColors::resolve("default", true);
    assert_eq!(def_dark.syntax_keyword, rgb(0xd2a8ff).into(), "default dark syntax_keyword");
    assert_eq!(def_dark.syntax_string, rgb(0x7ee787).into(), "default dark syntax_string");
    assert_eq!(def_dark.syntax_comment, rgb(0x8b949e).into(), "default dark syntax_comment");
    assert_eq!(def_dark.syntax_heading, rgb(0x58a6ff).into(), "default dark syntax_heading");
    assert_eq!(def_dark.syntax_inline_code, rgb(0xf3f1ed).into(), "default dark syntax_inline_code");
    assert_eq!(def_dark.syntax_link, rgb(0x58a6ff).into(), "default dark syntax_link");

    // 3. github light
    let gh_light = ThemeColors::resolve("github", false);
    assert_eq!(gh_light.syntax_keyword, rgb(0xcf222e).into(), "github light syntax_keyword");
    assert_eq!(gh_light.syntax_string, rgb(0x0a3069).into(), "github light syntax_string");
    assert_eq!(gh_light.syntax_comment, rgb(0x6e7781).into(), "github light syntax_comment");
    assert_eq!(gh_light.syntax_heading, rgb(0x0550ae).into(), "github light syntax_heading");
    assert_eq!(gh_light.syntax_inline_code, rgb(0x24292f).into(), "github light syntax_inline_code");
    assert_eq!(gh_light.syntax_link, rgb(0x0969da).into(), "github light syntax_link");

    // 4. nord dark
    let nord_dark = ThemeColors::resolve("nord", true);
    assert_eq!(nord_dark.syntax_keyword, rgb(0xb48ead).into(), "nord dark syntax_keyword");
    assert_eq!(nord_dark.syntax_string, rgb(0xa3be8c).into(), "nord dark syntax_string");
    assert_eq!(nord_dark.syntax_comment, rgb(0x616e88).into(), "nord dark syntax_comment");
    assert_eq!(nord_dark.syntax_heading, rgb(0x81a1c1).into(), "nord dark syntax_heading");
    assert_eq!(nord_dark.syntax_inline_code, rgb(0xeceff4).into(), "nord dark syntax_inline_code");
    assert_eq!(nord_dark.syntax_link, rgb(0x88c0d0).into(), "nord dark syntax_link");

    // 5. macos light
    let macos_light = ThemeColors::resolve("macos", false);
    assert_eq!(macos_light.syntax_keyword, rgb(0xaf52de).into(), "macos light syntax_keyword");
    assert_eq!(macos_light.syntax_string, rgb(0x34c759).into(), "macos light syntax_string");
    assert_eq!(macos_light.syntax_heading, rgb(0x007aff).into(), "macos light syntax_heading");
    assert_eq!(macos_light.syntax_inline_code, rgb(0x1c1c1e).into(), "macos light syntax_inline_code");

    // 6. claude light
    let claude_light = ThemeColors::resolve("claude", false);
    assert_eq!(claude_light.syntax_keyword, rgb(0xd97706).into(), "claude light syntax_keyword");
    assert_eq!(claude_light.syntax_string, rgb(0x059669).into(), "claude light syntax_string");
    assert_eq!(claude_light.syntax_heading, rgb(0xb45309).into(), "claude light syntax_heading");
    assert_eq!(claude_light.syntax_inline_code, rgb(0x292524).into(), "claude light syntax_inline_code");

    // 7. tokyo-night dark
    let tokyo_dark = ThemeColors::resolve("tokyo-night", true);
    assert_eq!(tokyo_dark.syntax_keyword, rgb(0xbb9af7).into(), "tokyo-night dark syntax_keyword");
    assert_eq!(tokyo_dark.syntax_string, rgb(0x9ece6a).into(), "tokyo-night dark syntax_string");
    assert_eq!(tokyo_dark.syntax_heading, rgb(0x7aa2f7).into(), "tokyo-night dark syntax_heading");
    assert_eq!(tokyo_dark.syntax_inline_code, rgb(0xc0caf5).into(), "tokyo-night dark syntax_inline_code");

    // 8. solarized light
    let solarized_light = ThemeColors::resolve("solarized", false);
    assert_eq!(solarized_light.syntax_keyword, rgb(0x859900).into(), "solarized light syntax_keyword");
    assert_eq!(solarized_light.syntax_string, rgb(0x2aa198).into(), "solarized light syntax_string");
    assert_eq!(solarized_light.syntax_heading, rgb(0x268bd2).into(), "solarized light syntax_heading");
    assert_eq!(solarized_light.syntax_inline_code, rgb(0x073642).into(), "solarized light syntax_inline_code");

    // 9. one-dark dark
    let one_dark = ThemeColors::resolve("one-dark", true);
    assert_eq!(one_dark.syntax_keyword, rgb(0xc678dd).into(), "one-dark dark syntax_keyword");
    assert_eq!(one_dark.syntax_string, rgb(0x98c379).into(), "one-dark dark syntax_string");
    assert_eq!(one_dark.syntax_heading, rgb(0x61afef).into(), "one-dark dark syntax_heading");
    assert_eq!(one_dark.syntax_inline_code, rgb(0xabb2bf).into(), "one-dark dark syntax_inline_code");
}

#[test]
fn test_canvas_header_no_dirty_red_dot() {
    let workspace_src = std::fs::read_to_string("src/views/workspace.rs")
        .or_else(|_| std::fs::read_to_string("crates/glidemark_app/src/views/workspace.rs"))
        .expect("Failed to read workspace.rs");

    let title_pos = workspace_src
        .find("canvas_document_title")
        .expect("workspace.rs must contain canvas_document_title");
    let title_section = &workspace_src[title_pos..std::cmp::min(title_pos + 1200, workspace_src.len())];

    assert!(
        !title_section.contains("when(is_dirty, |el| el.child(div().w(px(6.)).h(px(6.)).rounded_full().bg(theme.accent)"),
        "canvas_document_title must not contain dirty red dot indicator"
    );
    assert!(
        !title_section.contains("rounded_full().bg(theme.accent)"),
        "No red dot indicator should be attached to canvas_document_title"
    );
}

#[test]
fn test_preview_code_syntax_styling_contract() {
    let preview_src = std::fs::read_to_string("src/views/preview.rs")
        .or_else(|_| std::fs::read_to_string("crates/glidemark_app/src/views/preview.rs"))
        .expect("Failed to read preview.rs");

    let code_item_pos = preview_src
        .find("InlineItem::Code")
        .expect("preview.rs must contain InlineItem::Code");
    let code_item_section = &preview_src[code_item_pos..std::cmp::min(code_item_pos + 600, preview_src.len())];

    assert!(
        code_item_section.contains("theme.syntax_inline_code"),
        "InlineItem::Code must use theme.syntax_inline_code for text color"
    );
    assert!(
        code_item_section.contains("theme.code_bg"),
        "InlineItem::Code must use theme.code_bg for background color"
    );

    let code_block_pos = preview_src
        .find("fn render_code_block")
        .expect("preview.rs must contain render_code_block");
    let code_block_section = &preview_src[code_block_pos..std::cmp::min(code_block_pos + 2500, preview_src.len())];

    assert!(
        code_block_section.contains("FontWeight::BOLD"),
        "render_code_block must specify FontWeight::BOLD for language display tag"
    );
    assert!(
        code_block_section.contains("lang_display"),
        "render_code_block must render lang_display"
    );
}
