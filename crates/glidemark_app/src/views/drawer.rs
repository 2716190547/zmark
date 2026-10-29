use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;
use glidemark_core::ast::HeadingItem;
use glidemark_core::i18n::I18nStrings;

use crate::state::{DrawerTab, VisibleTreeRow};
use crate::theme::ThemeColors;

pub fn render_drawer(
    active_tab: DrawerTab,
    tree_rows: &[VisibleTreeRow],
    current_path: Option<&PathBuf>,
    workspace_folder: Option<&PathBuf>,
    headings: &[HeadingItem],
    theme: &ThemeColors,
    strings: &I18nStrings,
    scroll_handle: &ScrollHandle,
    on_select_tab: Arc<dyn Fn(DrawerTab, &mut Window, &mut App)>,
    on_toggle_folder: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_file: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_heading: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_open_folder: Arc<dyn Fn(&mut Window, &mut App)>,
    on_toggle_drawer: Arc<dyn Fn(&mut Window, &mut App)>,
) -> impl IntoElement {
    let t1 = on_select_tab.clone();
    let t2 = on_select_tab.clone();
    let t_folder = on_open_folder.clone();
    let t_toggle = on_toggle_drawer.clone();

    #[cfg(target_os = "macos")]
    let drawer_header_pl = px(76.);
    #[cfg(not(target_os = "macos"))]
    let drawer_header_pl = px(8.);

    div()
        .w(px(250.))
        .h_full()
        .bg(theme.bg)
        .border_r_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        // Top Header: 38px matching right canvas header, seamless with no horizontal bottom border
        .child(
            div()
                .h(px(38.))
                .pl(drawer_header_pl)
                .pr_2()
                .flex()
                .items_center()
                .justify_between()
                .gap_1()
                .child(render_icon_btn(
                    "drawer_collapse_btn",
                    "收起侧栏 (⌘B)",
                    "icons/panel-left.svg",
                    false,
                    theme,
                    move |_, window, cx| t_toggle(window, cx),
                ))
                .child(render_icon_btn(
                    "drawer_tab_files",
                    strings.tab_files,
                    "icons/files.svg",
                    active_tab == DrawerTab::Files,
                    theme,
                    move |_, window, cx| t1(DrawerTab::Files, window, cx),
                ))
                .child(render_icon_btn(
                    "drawer_tab_outline",
                    strings.tab_outline,
                    "icons/list.svg",
                    active_tab == DrawerTab::Outline,
                    theme,
                    move |_, window, cx| t2(DrawerTab::Outline, window, cx),
                ))
                .child(
                    div()
                        .id("drawer_header_drag_area")
                        .flex_1()
                        .h_full()
                        .window_control_area(WindowControlArea::Drag)
                        .on_mouse_down(MouseButton::Left, |_e, window, _| {
                            window.start_window_move();
                        })
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
                    DrawerTab::Files => {
                        let mut content = div().flex().flex_col().gap_0p5();

                        let folder_name = workspace_folder
                            .and_then(|f| f.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("工作区")
                            .to_string();
                        let folder_path_str = workspace_folder
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let t_switch_folder = t_folder.clone();

                        content = content.child(
                            div()
                                .id("drawer_workspace_folder_header")
                                .w_full()
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .id("drawer_folder_info")
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .min_w(px(0.))
                                        .text_color(theme.text_muted)
                                        .tooltip(move |_window, cx| {
                                            let t = folder_path_str.clone();
                                            cx.new(|_| DrawerTooltipView { text: t }).into()
                                        })
                                        .child(
                                            svg()
                                                .path("icons/folder.svg")
                                                .size(px(14.))
                                                .text_color(theme.text_muted),
                                        )
                                        .child(
                                            div()
                                                .min_w(px(0.))
                                                .truncate()
                                                .text_size(px(12.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(theme.text_primary)
                                                .child(folder_name),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn_switch_workspace")
                                        .px_1p5()
                                        .py_0p5()
                                        .rounded(px(4.))
                                        .cursor_pointer()
                                        .text_size(px(10.))
                                        .text_color(theme.text_muted)
                                        .hover(|s| s.bg(theme.surface).text_color(theme.text_primary))
                                        .tooltip(|_window, cx| {
                                            cx.new(|_| DrawerTooltipView { text: "切换工作区文件夹...".to_string() }).into()
                                        })
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            t_switch_folder(window, cx);
                                        })
                                        .child("切换"),
                                ),
                        );

                        if tree_rows.is_empty() {
                            content = content.child(
                                div()
                                    .p_4()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child("无 Markdown 文件"),
                            );
                        } else {
                            for (idx, row) in tree_rows.iter().enumerate() {
                                let depth_indent = (row.depth as f32) * 12.0;
                                if row.is_dir {
                                    let p = row.path.clone();
                                    let on_toggle = on_toggle_folder.clone();
                                    let is_expanded = row.is_expanded;
                                    let folder_name = row.name.clone();

                                    content = content.child(
                                        div()
                                            .id(("drawer_tree_dir", idx))
                                            .w_full()
                                            .pl(px(depth_indent + 4.0))
                                            .pr_2()
                                            .py_1()
                                            .rounded(px(5.))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .gap_1p5()
                                            .text_xs()
                                            .text_color(theme.text_muted)
                                            .hover(|s| s.bg(theme.surface).text_color(theme.text_primary))
                                            .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                                on_toggle(&p, window, cx);
                                            })
                                            .child(
                                                div()
                                                    .w(px(12.))
                                                    .h(px(12.))
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .text_size(px(10.))
                                                    .child(if is_expanded { "▾" } else { "▸" }),
                                            )
                                            .child(
                                                svg()
                                                    .path("icons/folder.svg")
                                                    .size(px(13.))
                                                    .text_color(theme.text_muted),
                                            )
                                            .child(
                                                div()
                                                    .min_w(px(0.))
                                                    .truncate()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(folder_name),
                                            ),
                                    );
                                } else {
                                    let is_active = current_path.map_or(false, |cp| cp == &row.path);
                                    let file_name = row.name.clone();
                                    let tooltip_name = file_name.clone();
                                    let p = row.path.clone();
                                    let on_select = on_select_file.clone();

                                    let mut item = div()
                                        .id(("drawer_tree_file", idx))
                                        .w_full()
                                        .pl(px(depth_indent + 20.0))
                                        .pr_2()
                                        .py_1()
                                        .rounded(px(6.))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .text_xs()
                                        .tooltip(move |_window, cx| {
                                            let t = tooltip_name.clone();
                                            cx.new(|_| DrawerTooltipView { text: t }).into()
                                        })
                                        .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                            on_select(&p, window, cx);
                                        });

                                    if is_active {
                                        item = item
                                            .bg(theme.warm_pill_bg())
                                            .text_color(theme.warm_pill_text())
                                            .font_weight(FontWeight::MEDIUM);
                                    } else {
                                        item = item
                                            .text_color(theme.text_primary)
                                            .hover(|s| s.bg(theme.surface));
                                    }

                                    content = content.child(
                                        item.child(
                                            div()
                                                .min_w(px(0.))
                                                .truncate()
                                                .child(file_name),
                                        ),
                                    );
                                }
                            }
                        }
                        content.into_any_element()
                    }
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
                                .children(headings.iter().enumerate().map(|(idx, h)| {
                                    let block_idx = h.block_index;
                                    let on_jump = on_select_heading.clone();
                                    let indent = ((h.level.saturating_sub(1) as f32) * 12.0).min(36.0);

                                    let (font_weight, text_color) = match h.level {
                                        1 => (FontWeight::BOLD, theme.text_primary),
                                        2 => (FontWeight::SEMIBOLD, theme.text_primary),
                                        _ => (FontWeight::NORMAL, theme.text_muted),
                                    };

                                    let heading_text = h.text.clone();
                                    let tooltip_heading = heading_text.clone();

                                    div()
                                        .id(("drawer_heading_item", idx))
                                        .w_full()
                                        .min_w(px(0.))
                                        .pl(px(indent + 8.0))
                                        .pr_2()
                                        .py_1()
                                        .rounded_md()
                                        .text_xs()
                                        .font_weight(font_weight)
                                        .text_color(text_color)
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.surface).text_color(theme.accent))
                                        .tooltip(move |_window, cx| {
                                            let t = tooltip_heading.clone();
                                            cx.new(|_| DrawerTooltipView { text: t }).into()
                                        })
                                        .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                            on_jump(block_idx, window, cx);
                                        })
                                        .child(
                                            div()
                                                .w_full()
                                                .min_w(px(0.))
                                                .truncate()
                                                .child(heading_text),
                                        )
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

fn render_icon_btn<F>(
    id: impl Into<ElementId>,
    tooltip_text: &str,
    icon_path: &'static str,
    active: bool,
    theme: &ThemeColors,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let text = tooltip_text.to_string();
    let base = div()
        .id(id)
        .tooltip(move |_window, cx| {
            let t = text.clone();
            cx.new(|_| DrawerTooltipView { text: t }).into()
        })
        .w(px(28.))
        .h(px(28.))
        .rounded_md()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler);

    let color = if active { theme.warm_pill_text() } else { theme.text_muted };
    let icon_el = svg().path(icon_path).size(px(16.)).text_color(color);

    if active {
        base.bg(theme.warm_pill_bg())
            .rounded(px(6.))
            .child(icon_el)
    } else {
        base.rounded(px(6.))
            .hover(|s| s.bg(theme.surface).text_color(theme.text_primary))
            .child(icon_el)
    }
}
