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
        rgb(0xf6f5ef).into(),
        "Default light code_bg must be #f6f5ef (Warm paper code block per Phase I)"
    );
    let sel_light = light.selection_bg();
    assert!(
        sel_light.h >= 25.0 / 360.0 && sel_light.h <= 45.0 / 360.0,
        "Default light selection_bg must have warm hue per Phase H"
    );
    assert!(
        sel_light.s >= 0.30,
        "Default light selection_bg must have warm saturation per Phase H"
    );
    assert!(
        sel_light.a >= 0.10 && sel_light.a <= 0.20,
        "Default light selection_bg must have subtle warm opacity per Phase H"
    );
    assert_eq!(
        light.selection_text(),
        light.text_primary,
        "Default light selection_text must be text_primary"
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
                ("syntax_attribute", theme.syntax_attribute),
                ("syntax_string", theme.syntax_string),
                ("syntax_number", theme.syntax_number),
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

    // Specifically verify representative palette values from Phase I (I2) warm palette:

    // 1. default light
    let def_light = ThemeColors::resolve("default", false);
    assert_eq!(def_light.syntax_keyword, rgb(0x7d56b3).into(), "default light syntax_keyword");
    assert_eq!(def_light.syntax_attribute, rgb(0x596a88).into(), "default light syntax_attribute");
    assert_eq!(def_light.syntax_string, rgb(0x8a6b12).into(), "default light syntax_string");
    assert_eq!(def_light.syntax_number, rgb(0x995b2b).into(), "default light syntax_number");
    assert_eq!(def_light.syntax_comment, rgb(0x676d65).into(), "default light syntax_comment");
    assert_eq!(def_light.syntax_heading, rgb(0xa4502c).into(), "default light syntax_heading");
    assert_eq!(def_light.syntax_inline_code, rgb(0x1f2328).into(), "default light syntax_inline_code");
    assert_eq!(def_light.syntax_link, rgb(0x0969da).into(), "default light syntax_link");

    // 2. default dark
    let def_dark = ThemeColors::resolve("default", true);
    assert_eq!(def_dark.syntax_keyword, rgb(0xd2a8ff).into(), "default dark syntax_keyword");
    assert_eq!(def_dark.syntax_attribute, rgb(0x9bb7d4).into(), "default dark syntax_attribute");
    assert_eq!(def_dark.syntax_string, rgb(0xe5c07b).into(), "default dark syntax_string");
    assert_eq!(def_dark.syntax_number, rgb(0xf39c6b).into(), "default dark syntax_number");
    assert_eq!(def_dark.syntax_comment, rgb(0xa8a29e).into(), "default dark syntax_comment");
    assert_eq!(def_dark.syntax_heading, rgb(0xd98a5f).into(), "default dark syntax_heading");
    assert_eq!(def_dark.syntax_inline_code, rgb(0xf3f1ed).into(), "default dark syntax_inline_code");
    assert_eq!(def_dark.syntax_link, rgb(0x58a6ff).into(), "default dark syntax_link");

    // 3. github light
    let gh_light = ThemeColors::resolve("github", false);
    assert_eq!(gh_light.syntax_keyword, rgb(0xcf222e).into(), "github light syntax_keyword");
    assert_eq!(gh_light.syntax_attribute, rgb(0x0550ae).into(), "github light syntax_attribute");
    assert_eq!(gh_light.syntax_string, rgb(0x0a3069).into(), "github light syntax_string");
    assert_eq!(gh_light.syntax_number, rgb(0x953800).into(), "github light syntax_number");
    assert_eq!(gh_light.syntax_comment, rgb(0x57606a).into(), "github light syntax_comment");
    assert_eq!(gh_light.syntax_heading, rgb(0x0550ae).into(), "github light syntax_heading");
    assert_eq!(gh_light.syntax_inline_code, rgb(0x24292f).into(), "github light syntax_inline_code");
    assert_eq!(gh_light.syntax_link, rgb(0x0969da).into(), "github light syntax_link");

    // 4. nord dark
    let nord_dark = ThemeColors::resolve("nord", true);
    assert_eq!(nord_dark.syntax_keyword, rgb(0xcdabe1).into(), "nord dark syntax_keyword");
    assert_eq!(nord_dark.syntax_attribute, rgb(0x88c0d0).into(), "nord dark syntax_attribute");
    assert_eq!(nord_dark.syntax_string, rgb(0xbbe2a3).into(), "nord dark syntax_string");
    assert_eq!(nord_dark.syntax_number, rgb(0xebcb8b).into(), "nord dark syntax_number");
    assert_eq!(nord_dark.syntax_comment, rgb(0xb8c5db).into(), "nord dark syntax_comment");
    assert_eq!(nord_dark.syntax_heading, rgb(0x81a1c1).into(), "nord dark syntax_heading");
    assert_eq!(nord_dark.syntax_inline_code, rgb(0xeceff4).into(), "nord dark syntax_inline_code");
    assert_eq!(nord_dark.syntax_link, rgb(0x88c0d0).into(), "nord dark syntax_link");

    // 5. macos light
    let macos_light = ThemeColors::resolve("macos", false);
    assert_eq!(macos_light.syntax_keyword, rgb(0x7b1fa2).into(), "macos light syntax_keyword");
    assert_eq!(macos_light.syntax_attribute, rgb(0x00695c).into(), "macos light syntax_attribute");
    assert_eq!(macos_light.syntax_string, rgb(0x1b5e20).into(), "macos light syntax_string");
    assert_eq!(macos_light.syntax_number, rgb(0xbf360c).into(), "macos light syntax_number");
    assert_eq!(macos_light.syntax_heading, rgb(0x007aff).into(), "macos light syntax_heading");
    assert_eq!(macos_light.syntax_inline_code, rgb(0x1c1c1e).into(), "macos light syntax_inline_code");

    // 6. claude light
    let claude_light = ThemeColors::resolve("claude", false);
    assert_eq!(claude_light.syntax_keyword, rgb(0x7d56b3).into(), "claude light syntax_keyword");
    assert_eq!(claude_light.syntax_attribute, rgb(0x596a88).into(), "claude light syntax_attribute");
    assert_eq!(claude_light.syntax_string, rgb(0x8a6b12).into(), "claude light syntax_string");
    assert_eq!(claude_light.syntax_number, rgb(0x995b2b).into(), "claude light syntax_number");
    assert_eq!(claude_light.syntax_comment, rgb(0x676d65).into(), "claude light syntax_comment");
    assert_eq!(claude_light.syntax_heading, rgb(0xb45309).into(), "claude light syntax_heading");
    assert_eq!(claude_light.syntax_inline_code, rgb(0x292524).into(), "claude light syntax_inline_code");

    // 7. tokyo-night dark
    let tokyo_dark = ThemeColors::resolve("tokyo-night", true);
    assert_eq!(tokyo_dark.syntax_keyword, rgb(0xbb9af7).into(), "tokyo-night dark syntax_keyword");
    assert_eq!(tokyo_dark.syntax_attribute, rgb(0x7dcfff).into(), "tokyo-night dark syntax_attribute");
    assert_eq!(tokyo_dark.syntax_string, rgb(0x9ece6a).into(), "tokyo-night dark syntax_string");
    assert_eq!(tokyo_dark.syntax_number, rgb(0xff9e64).into(), "tokyo-night dark syntax_number");
    assert_eq!(tokyo_dark.syntax_heading, rgb(0x7aa2f7).into(), "tokyo-night dark syntax_heading");
    assert_eq!(tokyo_dark.syntax_inline_code, rgb(0xc0caf5).into(), "tokyo-night dark syntax_inline_code");

    // 8. solarized light
    let solarized_light = ThemeColors::resolve("solarized", false);
    assert_eq!(solarized_light.syntax_keyword, rgb(0x586e00).into(), "solarized light syntax_keyword");
    assert_eq!(solarized_light.syntax_attribute, rgb(0x1a6b63).into(), "solarized light syntax_attribute");
    assert_eq!(solarized_light.syntax_string, rgb(0x19665f).into(), "solarized light syntax_string");
    assert_eq!(solarized_light.syntax_number, rgb(0xb53900).into(), "solarized light syntax_number");
    assert_eq!(solarized_light.syntax_heading, rgb(0x268bd2).into(), "solarized light syntax_heading");
    assert_eq!(solarized_light.syntax_inline_code, rgb(0x073642).into(), "solarized light syntax_inline_code");

    // 9. one-dark dark
    let one_dark = ThemeColors::resolve("one-dark", true);
    assert_eq!(one_dark.syntax_keyword, rgb(0xc678dd).into(), "one-dark dark syntax_keyword");
    assert_eq!(one_dark.syntax_attribute, rgb(0x61afef).into(), "one-dark dark syntax_attribute");
    assert_eq!(one_dark.syntax_string, rgb(0x98c379).into(), "one-dark dark syntax_string");
    assert_eq!(one_dark.syntax_number, rgb(0xd19a66).into(), "one-dark dark syntax_number");
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
    let code_block_section = &preview_src[code_block_pos..];

    assert!(
        code_block_section.contains("theme.syntax_code"),
        "render_code_block must render code text in theme.syntax_code"
    );
    assert!(
        code_block_section.contains("theme.code_bg"),
        "render_code_block must use theme.code_bg for background"
    );
    assert!(
        !code_block_section.contains("lang_display"),
        "render_code_block must not render lang_display header bar (ADR 0036 G3)"
    );
    assert!(
        !code_block_section.contains("border_1"),
        "render_code_block must not have border_1 (ADR 0036 G3)"
    );
}

#[test]
fn test_ticket_g0_selection_tokens() {
    for palette in ALL_PALETTES {
        for &is_dark in &[false, true] {
            let theme = ThemeColors::resolve(palette.id, is_dark);
            let sel_bg = theme.selection_bg();
            let hov_bg = theme.hover_bg();
            let sel_txt = theme.selection_text();

            if palette.id == "default" || palette.id == "claude" {
                assert!(
                    sel_bg.h >= 25.0 / 360.0 && sel_bg.h <= 45.0 / 360.0,
                    "selection_bg must have warm hue between 25° and 45° for {} (is_dark={}), got {}",
                    palette.id, is_dark, sel_bg.h * 360.0
                );
                assert!(
                    sel_bg.s >= 0.30,
                    "selection_bg must have warm saturation >= 0.30 for {} (is_dark={}), got {}",
                    palette.id, is_dark, sel_bg.s
                );
                assert!(
                    sel_bg.a >= 0.10 && sel_bg.a <= 0.22,
                    "selection_bg must have subtle warm opacity for {} (is_dark={}), got {}",
                    palette.id, is_dark, sel_bg.a
                );
            } else {
                assert_eq!(
                    sel_bg,
                    theme.text_primary.opacity(0.07),
                    "selection_bg must derive text_primary.opacity(0.07) for {} (is_dark={})",
                    palette.id, is_dark
                );
            }
            assert_eq!(
                hov_bg,
                theme.text_primary.opacity(0.04),
                "hover_bg must derive text_primary.opacity(0.04) for {} (is_dark={})",
                palette.id, is_dark
            );
            assert_eq!(
                sel_txt,
                theme.text_primary,
                "selection_text must match text_primary for {} (is_dark={})",
                palette.id, is_dark
            );
        }
    }
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_luminance(r: u8, g: u8, b: u8) -> f64 {
    let r_lin = srgb_to_linear(r as f64 / 255.0);
    let g_lin = srgb_to_linear(g as f64 / 255.0);
    let b_lin = srgb_to_linear(b as f64 / 255.0);
    0.2126 * r_lin + 0.7152 * g_lin + 0.0722 * b_lin
}

fn wcag_contrast(l1: f64, l2: f64) -> f64 {
    let lighter = l1.max(l2);
    let darker = l1.min(l2);
    (lighter + 0.05) / (darker + 0.05)
}

#[test]
fn test_ticket_g0_syntax_code_matrix_and_contrast() {
    // 1. Assert all 8 palettes across both light and dark modes have fully opaque syntax_code
    for palette in ALL_PALETTES {
        for &is_dark in &[false, true] {
            let theme = ThemeColors::resolve(palette.id, is_dark);
            assert_eq!(
                theme.syntax_code.a, 1.0,
                "Palette '{}' (is_dark={}) syntax_code must be fully opaque",
                palette.id, is_dark
            );
        }
    }

    // 2. Default palette Miaoyan olive gold values and WCAG contrast >= 4.5:1
    let def_light = ThemeColors::resolve("default", false);
    assert_eq!(
        def_light.syntax_code,
        rgb(0x6b6b1e).into(),
        "Default light syntax_code must be Miaoyan olive gold #6b6b1e"
    );
    let def_light_code_lum = srgb_luminance(0x6b, 0x6b, 0x1e);
    let def_light_bg_lum = srgb_luminance(0xf5, 0xf4, 0xf0);
    let cr_light = wcag_contrast(def_light_code_lum, def_light_bg_lum);
    assert!(
        cr_light >= 4.5,
        "Default light syntax_code contrast ratio must be >= 4.5:1, got {:.2}",
        cr_light
    );

    let def_dark = ThemeColors::resolve("default", true);
    assert_eq!(
        def_dark.syntax_code,
        rgb(0xcfc458).into(),
        "Default dark syntax_code must be Miaoyan olive gold dark #cfc458"
    );
    let def_dark_code_lum = srgb_luminance(0xcf, 0xc4, 0x58);
    let def_dark_bg_lum = srgb_luminance(0x18, 0x17, 0x16);
    let cr_dark = wcag_contrast(def_dark_code_lum, def_dark_bg_lum);
    assert!(
        cr_dark >= 4.5,
        "Default dark syntax_code contrast ratio must be >= 4.5:1, got {:.2}",
        cr_dark
    );

    // 3. Verify representative palette values from 8 palettes 16 colors matrix
    assert_eq!(ThemeColors::resolve("claude", false).syntax_code, rgb(0x6b4e1b).into());
    assert_eq!(ThemeColors::resolve("claude", true).syntax_code, rgb(0xe5b567).into());

    assert_eq!(ThemeColors::resolve("macos", false).syntax_code, rgb(0x1b4d89).into());
    assert_eq!(ThemeColors::resolve("macos", true).syntax_code, rgb(0x70b0ff).into());

    assert_eq!(ThemeColors::resolve("tokyo-night", false).syntax_code, rgb(0x2e4374).into());
    assert_eq!(ThemeColors::resolve("tokyo-night", true).syntax_code, rgb(0x8ba4e8).into());

    assert_eq!(ThemeColors::resolve("nord", false).syntax_code, rgb(0x3b5373).into());
    assert_eq!(ThemeColors::resolve("nord", true).syntax_code, rgb(0x98c4d8).into());

    assert_eq!(ThemeColors::resolve("github", false).syntax_code, rgb(0x24292f).into());
    assert_eq!(ThemeColors::resolve("github", true).syntax_code, rgb(0xd0d7de).into());

    assert_eq!(ThemeColors::resolve("solarized", false).syntax_code, rgb(0x475b62).into());
    assert_eq!(ThemeColors::resolve("solarized", true).syntax_code, rgb(0x93a1a1).into());

    assert_eq!(ThemeColors::resolve("one-dark", false).syntax_code, rgb(0x383a42).into());
    assert_eq!(ThemeColors::resolve("one-dark", true).syntax_code, rgb(0xabb2bf).into());
}

fn hsla_luminance(c: gpui::Hsla) -> f64 {
    let rgba = c.to_rgb();
    let r_lin = srgb_to_linear(rgba.r as f64);
    let g_lin = srgb_to_linear(rgba.g as f64);
    let b_lin = srgb_to_linear(rgba.b as f64);
    0.2126 * r_lin + 0.7152 * g_lin + 0.0722 * b_lin
}

#[test]
fn test_eight_palettes_text_primary_bg_contrast_ratio() {
    for palette in ALL_PALETTES {
        for &is_dark in &[false, true] {
            let theme = ThemeColors::resolve(palette.id, is_dark);
            let text_lum = hsla_luminance(theme.text_primary);
            let bg_lum = hsla_luminance(theme.bg);
            let cr = wcag_contrast(text_lum, bg_lum);
            assert!(
                cr >= 4.5,
                "Palette '{}' (is_dark={}) text_primary vs bg contrast ratio must be >= 4.5:1, got {:.2}",
                palette.id, is_dark, cr
            );
        }
    }
}

#[test]
fn test_all_palettes_all_syntax_tokens_contrast_ratio() {
    use glidemark_core::syntax::SyntaxRole;

    let roles = [
        SyntaxRole::Plain,
        SyntaxRole::Keyword,
        SyntaxRole::Attribute,
        SyntaxRole::String,
        SyntaxRole::Number,
        SyntaxRole::Comment,
    ];

    let mut failures = Vec::new();

    for palette in ALL_PALETTES {
        for &is_dark in &[false, true] {
            let theme = ThemeColors::resolve(palette.id, is_dark);
            let bg_lum = hsla_luminance(theme.code_bg);

            for role in roles {
                let fg_color = theme.syntax_color_for_role(role);
                let fg_lum = hsla_luminance(fg_color);
                let cr = wcag_contrast(fg_lum, bg_lum);
                if cr < 4.5 {
                    failures.push(format!(
                        "Palette '{}' (is_dark={}) role '{:?}' vs code_bg contrast ratio was {:.2} < 4.5",
                        palette.id, is_dark, role, cr
                    ));
                }
            }
        }
    }

    assert!(failures.is_empty(), "Contrast ratio failures:\n{}", failures.join("\n"));
}
