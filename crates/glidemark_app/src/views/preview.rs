use gpui::prelude::*;
use gpui::*;
use glidemark_core::ast::{BlockElement, InlineGroup, InlineItem, ParsedDocument};
use glidemark_core::i18n::I18nStrings;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::theme::ThemeColors;

pub type OnCopyCallback = Arc<dyn Fn(String, u64, &mut Window, &mut App) + Send + Sync>;
pub type OnOpenLightboxCallback = Arc<dyn Fn(std::path::PathBuf, u64, &mut Window, &mut App) + Send + Sync>;

pub fn render_preview_pane(
    parsed: Arc<ParsedDocument>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    max_column_width: f32,
    font_size: f32,
    list_state: &ListState,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<OnCopyCallback>,
    on_open_lightbox: Option<OnOpenLightboxCallback>,
) -> impl IntoElement {
    let theme = theme.clone();
    let item_theme = theme.clone();
    let item_parsed = parsed.clone();
    let item_on_copy = on_copy.clone();
    let item_on_lightbox = on_open_lightbox.clone();

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
                            .child(render_block_element(
                                block,
                                &item_theme,
                                strings,
                                font_size,
                                copied_code_hash,
                                item_on_copy.as_ref(),
                                item_on_lightbox.as_ref(),
                            )),
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
    strings: &'static I18nStrings,
    base_font_size: f32,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<&OnCopyCallback>,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
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
                render_mermaid_diagram(code, theme, strings, base_font_size, on_open_lightbox)
            } else {
                render_code_block(
                    lang.as_deref(),
                    code,
                    theme,
                    strings,
                    base_font_size,
                    copied_code_hash,
                    on_copy,
                )
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
                    .map(|b| render_block_element(b, theme, strings, base_font_size, copied_code_hash, on_copy, on_open_lightbox)),
            )
            .into_any_element(),
        BlockElement::List { ordered, items } => render_list_element(
            *ordered,
            items,
            theme,
            strings,
            base_font_size,
            0,
            copied_code_hash,
            on_copy,
            on_open_lightbox,
        ),
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
                        color: Some(theme.syntax_inline_code),
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

fn render_mermaid_container(
    path: std::path::PathBuf,
    code_hash: u64,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
) -> AnyElement {
    let mut container = div()
        .id(("mermaid_diagram_container", code_hash))
        .w_full()
        .min_w(px(0.))
        .my_3()
        .rounded_xl()
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .p_4()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .relative()
        .overflow_x_scroll();

    if let Some(cb) = on_open_lightbox {
        let cb = (*cb).clone();
        let p_click = path.clone();
        let cb_btn = cb.clone();
        let p_btn = path.clone();
        let btn = div()
            .id(("mermaid_zoom_btn", code_hash))
            .absolute()
            .top(px(8.))
            .right(px(8.))
            .w(px(28.))
            .h(px(28.))
            .rounded_md()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.selection_bg())
            .hover(|s| s.bg(theme.hover_bg()))
            .tooltip({
                let t = strings.mermaid_zoom_hint.to_string();
                move |_window, cx| cx.new(|_| CodeTooltipView { text: t.clone() }).into()
            })
            .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                cx.stop_propagation(); // 防冒泡到外层容器
                cb_btn(p_btn.clone(), code_hash, window, cx);
            })
            .child(
                svg()
                    .path("icons/maximize.svg")
                    .size(px(14.))
                    .text_color(theme.selection_text()),
            );

        container = container
            .cursor(CursorStyle::PointingHand)
            .hover(|s| s.border_color(theme.accent))
            .on_mouse_down(MouseButton::Left, move |_event, window, app| {
                cb(p_click.clone(), code_hash, window, app);
            })
            .child(btn);
    }

    container
        .child(
            img(path)
                .max_w_full()
        )
        .into_any_element()
}

fn render_mermaid_diagram(
    code: &str,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    _base_font_size: f32,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
) -> AnyElement {
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
            return render_mermaid_container(path, code_hash, theme, strings, on_open_lightbox);
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

            render_mermaid_container(path, code_hash, theme, strings, on_open_lightbox)
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

fn render_list_element(
    ordered: bool,
    items: &[glidemark_core::ast::ListItem],
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    base_font_size: f32,
    depth: usize,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<&OnCopyCallback>,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
) -> AnyElement {
    div()
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
                .flex_col()
                .child(
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
                                .border_color(theme.syntax_bullet)
                                .bg(if checked { theme.syntax_bullet } else { theme.bg })
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
                        } else if ordered {
                            div()
                                .flex_shrink_0()
                                .w(px(20.))
                                .text_right()
                                .text_size(px(base_font_size))
                                .font_family("ui-monospace")
                                .text_color(theme.syntax_bullet)
                                .child(format!("{}.", idx + 1))
                        } else {
                            let bullet = match depth % 3 {
                                0 => "–",
                                1 => "◦",
                                _ => "▪",
                            };
                            div()
                                .flex_shrink_0()
                                .w(px(12.))
                                .text_center()
                                .text_size(px(base_font_size))
                                .text_color(theme.syntax_bullet)
                                .child(bullet)
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(px(base_font_size))
                                .line_height(relative(1.6))
                                .text_color(theme.text_primary)
                                .child(render_styled_inlines(&item.content, theme, base_font_size)),
                        ),
                )
                .children(item.sub_list.as_ref().map(|sub| {
                    div()
                        .pl_5()
                        .pt_1()
                        .child(if let BlockElement::List { ordered: sub_ordered, items: sub_items } = sub.as_ref() {
                            render_list_element(
                                *sub_ordered,
                                sub_items,
                                theme,
                                strings,
                                base_font_size,
                                depth + 1,
                                copied_code_hash,
                                on_copy,
                                on_open_lightbox,
                            )
                        } else {
                            render_block_element(sub, theme, strings, base_font_size, copied_code_hash, on_copy, on_open_lightbox)
                        })
                }))
        }))
        .into_any_element()
}

/// Determines whether a code block should soft-wrap (`whitespace_normal`)
/// or retain horizontal scroll (`overflow_x_scroll().whitespace_nowrap()`).
///
/// Under ADR 0036 review specification:
/// - `None`, `""`, `"text"` (case and whitespace-insensitive) return `true` (soft-wrap).
/// - Programming languages such as `"python"`, `"rust"`, etc. return `false` (horizontal scroll).
/// - Note: Mermaid diagrams do not enter this function as they are rendered
///   as interactive diagram widgets with pan/zoom lightbox capabilities.
pub fn should_wrap_code_block(lang: Option<&str>) -> bool {
    match lang {
        None => true,
        Some(l) => {
            let s = l.trim().to_lowercase();
            s.is_empty() || s == "text"
        }
    }
}

fn render_code_block(
    lang: Option<&str>,
    code: &str,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    base_font_size: f32,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<&OnCopyCallback>,
) -> AnyElement {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    let hash = hasher.finish();
    let is_copied = copied_code_hash
        .map(|(h, t)| h == hash && t.elapsed() < std::time::Duration::from_millis(1200))
        .unwrap_or(false);

    let on_copy_cb = on_copy.cloned();
    let code_to_copy = code.to_string();
    let code_text = if code.is_empty() { " ".to_string() } else { code.to_string() };

    let should_wrap = should_wrap_code_block(lang);

    let group_name: SharedString = format!("code_block_{}", hash).into();

    let mut content = div()
        .id(SharedString::from(format!("code_content_scroll_{}", hash)))
        .w_full()
        .min_w(px(0.))
        .p_3()
        .text_size(px(base_font_size * 0.85))
        .font_family("ui-monospace")
        .line_height(relative(1.5))
        .text_color(theme.syntax_code)
        .child(StyledText::new(code_text));

    if should_wrap {
        content = content.whitespace_normal();
    } else {
        content = content.overflow_x_scroll().whitespace_nowrap();
    }

    let copy_btn = div()
        .id(SharedString::from(format!("copy_code_btn_{}", hash)))
        .absolute()
        .top(px(8.))
        .right(px(8.))
        .w(px(26.))
        .h(px(26.))
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .opacity(if is_copied { 1.0 } else { 0.0 })
        .group_hover(group_name.clone(), |s| s.opacity(1.0))
        .bg(theme.selection_bg())
        .hover(|s| s.bg(theme.hover_bg()))
        .tooltip({
            let is_copied = is_copied;
            move |_window, cx| {
                cx.new(|_| CodeTooltipView {
                    text: if is_copied {
                        strings.copied.to_string()
                    } else {
                        strings.copy_code.to_string()
                    },
                })
                .into()
            }
        })
        .on_click(move |_, window, cx| {
            if let Some(ref cb) = on_copy_cb {
                cb(code_to_copy.clone(), hash, window, cx);
            }
        })
        .child(
            svg()
                .path(if is_copied {
                    "icons/check.svg"
                } else {
                    "icons/copy.svg"
                })
                .size(px(13.))
                .text_color(if is_copied {
                    theme.accent
                } else {
                    theme.selection_text()
                }),
        );

    div()
        .id(SharedString::from(format!("code_block_{}", hash)))
        .group(group_name)
        .relative()
        .w_full()
        .min_w(px(0.))
        .rounded_md()
        .bg(theme.code_bg)
        .child(content)
        .child(copy_btn)
        .into_any_element()
}
