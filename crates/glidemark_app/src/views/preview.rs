use gpui::prelude::*;
use gpui::*;
use glidemark_core::ast::{BlockElement, InlineGroup, InlineItem, ParsedDocument};

use crate::theme::ThemeColors;

pub fn render_preview_pane(
    parsed: &ParsedDocument,
    theme: &ThemeColors,
    max_column_width: f32,
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
        .bg(theme.surface)
        .child(
            div()
                .h(px(32.))
                .px_4()
                .bg(theme.bg)
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
                        .max_w(px(max_column_width))
                        .mx_auto()
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
                1 => (1.8, true),
                2 => (1.45, true),
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
                .items_center()
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
            .items_center()
            .gap_1()
            .children(render_inline_elements(content, theme, base_font_size))
            .into_any_element(),
        BlockElement::FencedCode { lang, code } => {
            if lang.as_deref() == Some("mermaid") {
                render_mermaid_diagram(code, theme, base_font_size)
            } else {
                render_code_block(lang.as_deref(), code, theme, base_font_size)
            }
        }
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
                            .items_center()
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
                            .items_center()
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
                            .items_center()
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
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .border_1()
                .border_color(theme.border)
                .bg(theme.code_bg)
                .font_family("ui-monospace")
                .text_size(px(font_size * 0.88))
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

fn render_mermaid_diagram(code: &str, theme: &ThemeColors, base_font_size: f32) -> AnyElement {
    let diagram = glidemark_core::mermaid::MermaidDiagram::parse(code);

    div()
        .w_full()
        .rounded_xl()
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .overflow_hidden()
        .flex()
        .flex_col()
        // Header
        .child(
            div()
                .h(px(34.))
                .px_4()
                .bg(theme.bg)
                .border_b_1()
                .border_color(theme.border)
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(6.))
                                .h(px(6.))
                                .rounded_full()
                                .bg(theme.accent),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.text_primary)
                                .child("Mermaid 流程拓扑图"),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(theme.surface)
                                .border_1()
                                .border_color(theme.border)
                                .text_size(px(10.))
                                .font_family("ui-monospace")
                                .text_color(theme.text_muted)
                                .child(diagram.direction.clone()),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(format!("{} 阶段  {} 节点", diagram.stages.len(), diagram.nodes.len())),
                ),
        )
        // Topological Diagram Stages
        .child(
            div()
                .p_6()
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .children(diagram.stages.iter().enumerate().map(|(stage_idx, stage_nodes)| {
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .when(stage_idx > 0, |el| {
                            el.child(
                                div()
                                    .w(px(22.))
                                    .h(px(22.))
                                    .rounded_full()
                                    .bg(theme.bg)
                                    .border_1()
                                    .border_color(theme.border)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(11.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.accent)
                                    .child("↓"),
                            )
                        })
                        .child(
                            div()
                                .w_full()
                                .flex()
                                .flex_wrap()
                                .justify_center()
                                .gap_3()
                                .children(stage_nodes.iter().map(|node| {
                                    render_mermaid_node_card(node, theme, base_font_size)
                                })),
                        )
                })),
        )
        .into_any_element()
}

fn render_mermaid_node_card(
    node: &glidemark_core::mermaid::MermaidNode,
    theme: &ThemeColors,
    base_font_size: f32,
) -> AnyElement {
    let (bg_color, border_color, accent_tag) = match node.class_name.as_deref() {
        Some("step") => (theme.surface, theme.accent, "步骤"),
        Some("run") => (theme.code_bg, rgb(0x22c55e).into(), "执行"),
        Some("check") => (theme.surface, rgb(0xf59e0b).into(), "校验"),
        _ => (theme.bg, theme.border, "阶段"),
    };

    let shape_icon = match node.shape {
        glidemark_core::mermaid::MermaidShape::Database => "🗄️",
        glidemark_core::mermaid::MermaidShape::Diamond => "◇",
        glidemark_core::mermaid::MermaidShape::Rounded => "○",
        glidemark_core::mermaid::MermaidShape::Rectangle => "■",
    };

    div()
        .min_w(px(200.))
        .max_w(px(380.))
        .p_3()
        .bg(bg_color)
        .rounded_lg()
        .border_1()
        .border_color(border_color)
        .flex()
        .flex_col()
        .gap_1p5()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .child(div().text_size(px(10.)).child(shape_icon))
                        .child(
                            div()
                                .text_size(px(10.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .font_family("ui-monospace")
                                .text_color(theme.accent)
                                .child(node.id.clone()),
                        ),
                )
                .child(
                    div()
                        .px_1p5()
                        .py_0p5()
                        .rounded_xs()
                        .bg(theme.bg)
                        .border_1()
                        .border_color(theme.border)
                        .text_size(px(9.))
                        .text_color(theme.text_muted)
                        .child(accent_tag),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .children(node.label.lines().map(|line| {
                    div()
                        .text_size(px(base_font_size * 0.85))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.text_primary)
                        .line_height(relative(1.4))
                        .child(line.trim().to_string())
                })),
        )
        .into_any_element()
}

struct CodeTooltipView {
    text: String,
}

impl Render for CodeTooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgba(0x18181bf2))
            .border_1()
            .border_color(rgba(0x3f3f4640))
            .text_size(px(11.))
            .text_color(rgb(0xf4f4f5))
            .font_family("ui-sans-serif, system-ui")
            .shadow_md()
            .child(self.text.clone())
    }
}

fn render_code_block(
    lang: Option<&str>,
    code: &str,
    theme: &ThemeColors,
    base_font_size: f32,
) -> AnyElement {
    let line_count = code.lines().count().max(1);
    let lang_display = lang.unwrap_or("text").to_uppercase();

    div()
        .w_full()
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .bg(theme.code_bg)
        .overflow_hidden()
        .child(
            div()
                .h(px(32.))
                .px_3()
                .bg(theme.surface)
                .border_b_1()
                .border_color(theme.border)
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(theme.bg)
                                .border_1()
                                .border_color(theme.border)
                                .text_size(px(10.))
                                .font_family("ui-monospace")
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.accent)
                                .child(lang_display),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_family("ui-monospace")
                                .text_color(theme.text_muted)
                                .child(format!("{} lines", line_count)),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("copy_code_btn_{}", code.len())))
                        .tooltip(|_window, cx| {
                            cx.new(|_| CodeTooltipView { text: "复制代码".to_string() }).into()
                        })
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .child(
                            svg()
                                .path("icons/copy.svg")
                                .size(px(13.))
                                .text_color(theme.text_muted),
                        ),
                ),
        )
        .child(
            div()
                .id("fenced_code_scroll")
                .p_3()
                .overflow_x_scroll()
                .flex()
                .gap_3()
                // Line numbers
                .child(
                    div()
                        .pr_2()
                        .border_r_1()
                        .border_color(theme.border)
                        .flex()
                        .flex_col()
                        .items_end()
                        .children((1..=line_count).map(|n| {
                            div()
                                .text_size(px(base_font_size * 0.85))
                                .font_family("ui-monospace")
                                .line_height(relative(1.5))
                                .text_color(theme.text_muted)
                                .child(n.to_string())
                        })),
                )
                // Code lines
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .children(code.lines().map(|line| {
                            div()
                                .text_size(px(base_font_size * 0.85))
                                .font_family("ui-monospace")
                                .line_height(relative(1.5))
                                .text_color(theme.text_primary)
                                .child(if line.is_empty() { " ".to_string() } else { line.to_string() })
                        })),
                ),
        )
        .into_any_element()
}
