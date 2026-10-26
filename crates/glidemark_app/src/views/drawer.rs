use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;
use glidemark_core::ast::HeadingItem;
use glidemark_core::i18n::I18nStrings;

use crate::state::DrawerTab;
use crate::theme::ThemeColors;

pub fn render_drawer(
    active_tab: DrawerTab,
    sibling_files: &[PathBuf],
    current_path: Option<&PathBuf>,
    headings: &[HeadingItem],
    theme: &ThemeColors,
    strings: &I18nStrings,
    scroll_handle: &ScrollHandle,
    on_select_tab: Arc<dyn Fn(DrawerTab, &mut Window, &mut App)>,
    on_select_file: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_heading: Arc<dyn Fn(usize, &mut Window, &mut App)>,
) -> impl IntoElement {
    let t1 = on_select_tab.clone();
    let t2 = on_select_tab.clone();

    div()
        .w(px(250.))
        .h_full()
        .bg(theme.surface)
        .border_r_1()
        .border_color(theme.border.opacity(0.35))
        .flex()
        .flex_col()
        // Top Header: Dual Segment Capsule Tabs (Files / Outline)
        .child(
            div()
                .p_2()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w_full()
                        .bg(theme.bg)
                        .p_0p5()
                        .rounded_md()
                        .flex()
                        .gap_0p5()
                        .child(render_tab_pill(
                            "tab_pill_files",
                            strings.tab_files,
                            "icons/files.svg",
                            active_tab == DrawerTab::Files,
                            theme,
                            move |_, window, cx| t1(DrawerTab::Files, window, cx),
                        ))
                        .child(render_tab_pill(
                            "tab_pill_outline",
                            strings.tab_outline,
                            "icons/list.svg",
                            active_tab == DrawerTab::Outline,
                            theme,
                            move |_, window, cx| t2(DrawerTab::Outline, window, cx),
                        )),
                ),
        )
        // Scrollable Tab Body
        .child(
            div()
                .id("drawer_content_scroll")
                .track_scroll(scroll_handle)
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scroll()
                .p_2()
                .flex()
                .flex_col()
                .gap_1()
                .child(match active_tab {
                    DrawerTab::Files => div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .children(sibling_files.iter().map(|path| {
                            let is_active = current_path.map_or(false, |cp| cp == path);
                            let file_name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("untitled.md")
                                .to_string();

                            let item = div()
                                .px_2p5()
                                .py_1p5()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer();

                            let p = path.clone();
                            let on_select = on_select_file.clone();
                            let interactive_item = item.on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                on_select(&p, window, cx);
                            });

                            if is_active {
                                interactive_item
                                    .bg(theme.accent)
                                    .text_color(rgb(0xffffff))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(file_name)
                            } else {
                                interactive_item
                                    .text_color(theme.text_primary)
                                    .hover(|s| s.bg(theme.bg))
                                    .child(file_name)
                            }
                        }))
                        .into_any_element(),
                    DrawerTab::Outline => {
                        if headings.is_empty() {
                            div()
                                .p_4()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_xs()
                                .text_color(theme.text_muted)
                                .child(strings.outline_empty)
                                .into_any_element()
                        } else {
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .children(headings.iter().map(|h| {
                                    let block_idx = h.block_index;
                                    let on_jump = on_select_heading.clone();
                                    let indent = ((h.level.saturating_sub(1) as f32) * 12.0).min(36.0);

                                    let font_weight = match h.level {
                                        1 => FontWeight::BOLD,
                                        2 => FontWeight::SEMIBOLD,
                                        _ => FontWeight::NORMAL,
                                    };

                                    div()
                                        .pl(px(indent + 8.0))
                                        .pr_2()
                                        .py_1()
                                        .rounded_md()
                                        .text_xs()
                                        .font_weight(font_weight)
                                        .text_color(theme.text_primary)
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.bg).text_color(theme.accent))
                                        .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                            on_jump(block_idx, window, cx);
                                        })
                                        .child(h.text.clone())
                                }))
                                .into_any_element()
                        }
                    }
                }),
        )
}

struct DrawerTooltipView {
    text: String,
}

impl Render for DrawerTooltipView {
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

fn render_tab_pill<F>(
    id: impl Into<ElementId>,
    title: &str,
    icon_path: &'static str,
    active: bool,
    theme: &ThemeColors,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let text = title.to_string();
    let base = div()
        .id(id)
        .tooltip(move |_window, cx| {
            let t = text.clone();
            cx.new(|_| DrawerTooltipView { text: t }).into()
        })
        .flex_1()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .gap_1p5()
        .text_xs()
        .on_mouse_down(MouseButton::Left, handler);

    let color = if active { theme.accent } else { theme.text_muted };
    let icon_el = svg().path(icon_path).size(px(13.)).text_color(color);

    if active {
        base.bg(theme.surface)
            .shadow_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.accent)
            .child(icon_el)
            .child(title.to_string())
    } else {
        base.text_color(theme.text_muted)
            .hover(|s| s.text_color(theme.text_primary))
            .child(icon_el)
            .child(title.to_string())
    }
}
