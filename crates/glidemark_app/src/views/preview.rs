use gpui::prelude::*;
use gpui::*;
use glidemark_core::ast::{BlockElement, InlineGroup, InlineItem, ParsedDocument};

use crate::theme::ThemeColors;

pub fn render_preview_pane(
    parsed: &ParsedDocument,
    theme: &ThemeColors,
    _max_column_width: f32,
    font_size: f32,
    scroll_handle: &ScrollHandle,
    title: &str,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_h(px(0.))
        .h_full()
        .flex()
        .flex_col()
        .bg(theme.bg)
        .child(
            div()
                .h(px(32.))
                .px_4()
                .bg(theme.surface)
                .border_b_1()
                .border_color(theme.border)
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_muted)
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(format!("{} blocks", parsed.blocks.len())),
                ),
        )
        .child(
            div()
                .id("preview_scroll_container")
                .track_scroll(scroll_handle)
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scroll()
                .px_8()
                .py_6()
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .children(
                            parsed
                                .blocks
                                .iter()
                                .map(|block| render_block_element(block, theme, font_size)),
                        ),
                ),
        )
}

fn render_block_element(
    block: &BlockElement,
    theme: &ThemeColors,
    base_font_size: f32,
) -> AnyElement {
    match block {
        BlockElement::Heading { level, content } => {
            let (size_mult, border_bottom) = match level {
                1 => (2.0, true),
                2 => (1.5, true),
                3 => (1.25, false),
                _ => (1.1, false),
            };

            let mut el = div()
                .text_size(px(base_font_size * size_mult))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_primary)
                .pt_3()
                .pb_1()
                .flex()
                .flex_wrap()
                .gap_1();

            if border_bottom {
                el = el.border_b_1().border_color(theme.border);
            }

            el.children(render_inline_elements(content, theme, base_font_size * size_mult))
                .into_any_element()
        }
        BlockElement::Paragraph(content) => div()
            .text_size(px(base_font_size))
            .line_height(relative(1.7))
            .text_color(theme.text_primary)
            .flex()
            .flex_wrap()
            .gap_1()
            .children(render_inline_elements(content, theme, base_font_size))
            .into_any_element(),
        BlockElement::FencedCode { lang, code } => div()
            .w_full()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(theme.code_bg)
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_1()
                    .bg(theme.surface)
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_family("ui-monospace")
                            .text_color(theme.text_muted)
                            .child(lang.clone().unwrap_or_else(|| "text".to_string())),
                    ),
            )
            .child(
                div()
                    .id("fenced_code_scroll")
                    .p_4()
                    .overflow_x_scroll()
                    .font_family("ui-monospace")
                    .text_size(px(base_font_size * 0.9))
                    .line_height(relative(1.5))
                    .text_color(theme.text_primary)
                    .children(code.lines().map(|line| {
                        div().child(if line.is_empty() { " ".to_string() } else { line.to_string() })
                    })),
            )
            .into_any_element(),
        BlockElement::BlockQuote(blocks) => div()
            .w_full()
            .border_l_4()
            .border_color(theme.accent)
            .bg(theme.surface)
            .p_3()
            .rounded_r_md()
            .flex()
            .flex_col()
            .gap_2()
            .children(
                blocks
                    .iter()
                    .map(|b| render_block_element(b, theme, base_font_size)),
            )
            .into_any_element(),
        BlockElement::List { ordered, items } => div()
            .w_full()
            .flex()
            .flex_col()
            .gap_1()
            .children(items.iter().enumerate().map(|(idx, item)| {
                div()
                    .flex()
                    .items_start()
                    .gap_2()
                    .child(if let Some(checked) = item.checked {
                        div()
                            .mt_1()
                            .w(px(14.))
                            .h(px(14.))
                            .rounded_sm()
                            .border_1()
                            .border_color(theme.accent)
                            .bg(if checked { theme.accent } else { theme.bg })
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(if checked {
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(0xffffff))
                                    .child("✓")
                            } else {
                                div().child("")
                            })
                    } else if *ordered {
                        div()
                            .text_size(px(base_font_size))
                            .font_family("ui-monospace")
                            .text_color(theme.text_muted)
                            .child(format!("{}.", idx + 1))
                    } else {
                        div()
                            .text_size(px(base_font_size))
                            .text_color(theme.accent)
                            .child("•")
                    })
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(base_font_size))
                            .line_height(relative(1.6))
                            .text_color(theme.text_primary)
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .children(render_inline_elements(
                                &item.content,
                                theme,
                                base_font_size,
                            )),
                    )
            }))
            .into_any_element(),
        BlockElement::Table { headers, rows } => div()
            .w_full()
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(
                div()
                    .bg(theme.surface)
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .children(headers.iter().map(|h| {
                        div()
                            .flex_1()
                            .p_2()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(base_font_size * 0.95))
                            .text_color(theme.text_primary)
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .children(render_inline_elements(h, theme, base_font_size * 0.95))
                    })),
            )
            .children(rows.iter().map(|row| {
                div()
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .children(row.iter().map(|cell| {
                        div()
                            .flex_1()
                            .p_2()
                            .text_size(px(base_font_size * 0.95))
                            .text_color(theme.text_primary)
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .children(render_inline_elements(cell, theme, base_font_size * 0.95))
                    }))
            }))
            .into_any_element(),
        BlockElement::Rule => div()
            .w_full()
            .my_4()
            .border_b_1()
            .border_color(theme.border)
            .into_any_element(),
    }
}

fn render_inline_elements(
    group: &InlineGroup,
    theme: &ThemeColors,
    font_size: f32,
) -> Vec<AnyElement> {
    group
        .0
        .iter()
        .map(|item| match item {
            InlineItem::Text(s) => div().text_size(px(font_size)).child(s.clone()).into_any_element(),
            InlineItem::Bold(s) => div()
                .text_size(px(font_size))
                .font_weight(FontWeight::BOLD)
                .child(s.clone())
                .into_any_element(),
            InlineItem::Italic(s) => div()
                .text_size(px(font_size))
                .child(s.clone())
                .into_any_element(),
            InlineItem::Strikethrough(s) => div()
                .text_size(px(font_size))
                .line_through()
                .text_color(theme.text_muted)
                .child(s.clone())
                .into_any_element(),
            InlineItem::Code(s) => div()
                .px_1()
                .rounded_sm()
                .bg(theme.code_bg)
                .font_family("ui-monospace")
                .text_size(px(font_size * 0.9))
                .text_color(theme.accent)
                .child(s.clone())
                .into_any_element(),
            InlineItem::Link { text, url: _, title: _ } => div()
                .text_size(px(font_size))
                .text_color(theme.accent)
                .underline()
                .cursor_pointer()
                .child(text.clone())
                .into_any_element(),
            InlineItem::Image { alt, url, title: _ } => div()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .bg(theme.surface)
                .flex()
                .items_center()
                .gap_2()
                .child(div().text_size(px(14.)).child("🖼️"))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(format!("{} ({})", alt, url)),
                )
                .into_any_element(),
        })
        .collect()
}
