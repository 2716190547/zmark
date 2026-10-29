use gpui::prelude::*;
use gpui::*;
use glidemark_core::ast::{BlockElement, InlineGroup, InlineItem, ParsedDocument};
use std::sync::Arc;

use crate::theme::ThemeColors;

pub fn render_preview_pane(
    parsed: Arc<ParsedDocument>,
    theme: &ThemeColors,
    max_column_width: f32,
    font_size: f32,
    list_state: &ListState,
) -> impl IntoElement {
    let theme = theme.clone();
    let item_theme = theme.clone();
    let item_parsed = parsed.clone();

    div()
        .flex_1()
        .w_full()
        .min_w(px(0.))
        .min_h(px(0.))
        .h_full()
        .flex()
        .flex_col()
        .bg(theme.surface)
        .child(
            list(list_state.clone(), move |ix, _window, _cx| {
                let Some(block) = item_parsed.blocks.get(ix) else {
                    return div().into_any_element();
                };

                div()
                    .w_full()
                    .px_8()
                    .pb_4()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(max_column_width))
                            .mx_auto()
                            .child(render_block_element(block, &item_theme, font_size)),
                    )
                    .into_any_element()
            })
            .flex_1()
            .w_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .pt_6()
            .pb_6(),
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
                .w_full()
                .min_w(px(0.))
                .text_size(px(base_font_size * size_mult))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_primary)
                .pt_3()
                .pb_1();

            if border_bottom {
                el = el.border_b_1().border_color(theme.border);
            }

            el.child(render_styled_inlines(content, theme, base_font_size * size_mult))
                .into_any_element()
        }
        BlockElement::Paragraph(content) => div()
            .w_full()
            .min_w(px(0.))
            .text_size(px(base_font_size))
            .line_height(relative(1.75))
            .text_color(theme.text_primary)
            .child(render_styled_inlines(content, theme, base_font_size))
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
            .min_w(px(0.))
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
            .min_w(px(0.))
            .flex()
            .flex_col()
            .gap_1p5()
            .children(items.iter().enumerate().map(|(idx, item)| {
                div()
                    .w_full()
                    .min_w(px(0.))
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
                                svg()
                                    .path("icons/check.svg")
                                    .size(px(9.))
                                    .text_color(rgb(0xffffff))
                                    .into_any_element()
                            } else {
                                div().into_any_element()
                            })
                    } else if *ordered {
                        div()
                            .flex_shrink_0()
                            .w(px(20.))
                            .text_right()
                            .text_size(px(base_font_size))
                            .font_family("ui-monospace")
                            .text_color(theme.text_muted)
                            .child(format!("{}.", idx + 1))
                    } else {
                        div()
                            .flex_shrink_0()
                            .w(px(12.))
                            .text_center()
                            .text_size(px(base_font_size))
                            .text_color(theme.accent)
                            .child("–")
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_size(px(base_font_size))
                            .line_height(relative(1.6))
                            .text_color(theme.text_primary)
                            .child(render_styled_inlines(&item.content, theme, base_font_size)),
                    )
            }))
            .into_any_element(),
        BlockElement::Table { headers, rows } => div()
            .id("preview_table_scroll")
            .w_full()
            .min_w(px(0.))
            .overflow_x_scroll()
            .my_2()
            .child(
                div()
                    .w_full()
                    .min_w(px(520.))
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .bg(theme.bg)
                            .border_b_1()
                            .border_color(theme.border)
                            .flex()
                            .children(headers.iter().map(|h| {
                                div()
                                    .flex_1()
                                    .min_w(px(120.))
                                    .p_3()
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(base_font_size * 0.95))
                                    .text_color(theme.text_primary)
                                    .child(render_styled_inlines(h, theme, base_font_size * 0.95))
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
                                    .min_w(px(120.))
                                    .p_3()
                                    .text_size(px(base_font_size * 0.95))
                                    .text_color(theme.text_primary)
                                    .child(render_styled_inlines(cell, theme, base_font_size * 0.95))
                            }))
                    })),
            )
            .into_any_element(),
        BlockElement::Rule => div()
            .w_full()
            .my_4()
            .border_b_1()
            .border_color(theme.border)
            .into_any_element(),
    }
}

fn render_styled_inlines(
    group: &InlineGroup,
    theme: &ThemeColors,
    font_size: f32,
) -> AnyElement {
    let has_image = group.0.iter().any(|item| matches!(item, InlineItem::Image { .. }));
    if has_image {
        return div()
            .w_full()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .gap_2()
            .children(group.0.iter().map(|item| match item {
                InlineItem::Image { alt, url, .. } => div()
                    .w_full()
                    .min_w(px(0.))
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.bg)
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        svg()
                            .path("icons/image.svg")
                            .size(px(18.))
                            .text_color(theme.text_muted),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child(if alt.is_empty() { url.clone() } else { format!("{} ({})", alt, url) }),
                    )
                    .into_any_element(),
                other => {
                    let single = InlineGroup(vec![other.clone()]);
                    render_styled_text_only(&single, theme, font_size).into_any_element()
                }
            }))
            .into_any_element();
    }

    render_styled_text_only(group, theme, font_size).into_any_element()
}

fn render_styled_text_only(
    group: &InlineGroup,
    theme: &ThemeColors,
    _font_size: f32,
) -> StyledText {
    let mut text = String::new();
    let mut highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();

    for item in &group.0 {
        let start = text.len();
        match item {
            InlineItem::Text(s) => {
                text.push_str(s);
            }
            InlineItem::Bold(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Italic(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Strikethrough(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        color: Some(theme.text_muted),
                        strikethrough: Some(StrikethroughStyle {
                            thickness: px(1.),
                            color: Some(theme.text_muted),
                        }),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Code(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        color: Some(theme.accent),
                        background_color: Some(theme.code_bg),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Link { text: link_text, .. } => {
                text.push_str(link_text);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        color: Some(theme.accent),
                        underline: Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(theme.accent),
                            wavy: false,
                        }),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Image { alt, .. } => {
                text.push_str(alt);
            }
        }
    }

    if text.is_empty() {
        text.push(' ');
    }

    StyledText::new(text).with_highlights(highlights)
}

pub const MAX_DIAGRAM_CACHE: usize = 64;

pub struct DiagramCache {
    entries: std::collections::HashMap<(u64, bool), std::path::PathBuf>,
    order: std::collections::VecDeque<(u64, bool)>,
    max_size: usize,
}

impl DiagramCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
            max_size,
        }
    }

    pub fn get(&mut self, key: &(u64, bool)) -> Option<std::path::PathBuf> {
        let path = self.entries.get(key)?.clone();
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
            self.order.push_back(*key);
        }
        Some(path)
    }

    pub fn insert(&mut self, key: (u64, bool), path: std::path::PathBuf) {
        if self.entries.insert(key, path).is_none() {
            self.order.push_back(key);
            if self.order.len() > self.max_size {
                if let Some(evicted_key) = self.order.pop_front() {
                    if let Some(evicted_path) = self.entries.remove(&evicted_key) {
                        let _ = std::fs::remove_file(evicted_path);
                    }
                }
            }
        } else if let Some(pos) = self.order.iter().position(|k| k == &key) {
            self.order.remove(pos);
            self.order.push_back(key);
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

static DIAGRAM_CACHE: std::sync::LazyLock<std::sync::Mutex<DiagramCache>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(DiagramCache::new(MAX_DIAGRAM_CACHE)));

fn render_mermaid_diagram(code: &str, theme: &ThemeColors, _base_font_size: f32) -> AnyElement {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    let code_hash = hasher.finish();
    let is_dark = theme.is_dark;

    // Check in-memory memoized cache first (zero-cost on scroll)
    let cached_path = {
        let mut cache = DIAGRAM_CACHE.lock().unwrap();
        cache.get(&(code_hash, is_dark))
    };

    if let Some(path) = cached_path {
        if path.exists() {
            return div()
                .id("mermaid_diagram_scroll")
                .w_full()
                .min_w(px(0.))
                .my_3()
                .rounded_xl()
                .border_1()
                .border_color(theme.border)
                .bg(theme.surface)
                .p_4()
                .flex()
                .justify_center()
                .items_center()
                .overflow_x_scroll()
                .child(
                    img(path)
                        .max_w_full()
                )
                .into_any_element();
        }
    }

    let result = glidemark_core::mermaid::render_to_svg_themed(code, is_dark);
    match result {
        Ok(svg_content) => {
            let cache_dir = std::env::temp_dir()
                .join("glidemark")
                .join("mermaid");
            let _ = std::fs::create_dir_all(&cache_dir);
            let filename = format!("{:016x}_{}.svg", code_hash, if is_dark { "dark" } else { "light" });
            let path = cache_dir.join(filename);
            let _ = std::fs::write(&path, svg_content);

            {
                let mut cache = DIAGRAM_CACHE.lock().unwrap();
                cache.insert((code_hash, is_dark), path.clone());
            }

            div()
                .id("mermaid_diagram_scroll")
                .w_full()
                .min_w(px(0.))
                .my_3()
                .rounded_xl()
                .border_1()
                .border_color(theme.border)
                .bg(theme.surface)
                .p_4()
                .flex()
                .justify_center()
                .items_center()
                .overflow_x_scroll()
                .child(
                    img(path)
                        .max_w_full()
                )
                .into_any_element()
        }
        Err(err) => {
            div()
                .w_full()
                .min_w(px(0.))
                .my_3()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg)
                .p_3()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.accent)
                        .child("Mermaid Syntax Error"),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family("ui-monospace")
                        .text_color(theme.text_muted)
                        .child(err),
                )
                .into_any_element()
        }
    }
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
    let line_count = if code.is_empty() { 1 } else { code.lines().count() };
    let lang_display = lang.unwrap_or("text").to_uppercase();
    let line_numbers = (1..=line_count)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let code_text = if code.is_empty() { " ".to_string() } else { code.to_string() };

    div()
        .w_full()
        .min_w(px(0.))
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .bg(theme.code_bg)
        .overflow_hidden()
        .child(
            div()
                .px_2p5()
                .py_2p5()
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
                .w_full()
                .min_w(px(0.))
                .flex()
                .items_start()
                .child(
                    div()
                        .w(px(32.))
                        .flex_shrink_0()
                        .pr_2()
                        .mr_2()
                        .border_r_1()
                        .border_color(theme.border)
                        .text_right()
                        .text_size(px(base_font_size * 0.85))
                        .font_family("ui-monospace")
                        .line_height(relative(1.5))
                        .text_color(theme.text_muted)
                        .child(line_numbers),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(px(base_font_size * 0.85))
                        .font_family("ui-monospace")
                        .line_height(relative(1.5))
                        .text_color(theme.text_primary)
                        .child(StyledText::new(code_text)),
                ),
        )
        .into_any_element()
}
