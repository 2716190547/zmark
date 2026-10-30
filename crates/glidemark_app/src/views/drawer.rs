use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;
use glidemark_core::ast::HeadingItem;
use glidemark_core::i18n::I18nStrings;

use crate::state::{DocKey, DrawerTab, OpenDoc, VisibleTreeRow};
use crate::theme::ThemeColors;

pub fn render_drawer(
    active_tab: DrawerTab,
    tree_rows: &[VisibleTreeRow],
    open_docs: &[OpenDoc],
    active_key: Option<&DocKey>,
    is_active_dirty: bool,
    workspace_folder: Option<&PathBuf>,
    headings: &[HeadingItem],
    theme: &ThemeColors,
    strings: &I18nStrings,
    scroll_handle: &ScrollHandle,
    on_select_tab: Arc<dyn Fn(DrawerTab, &mut Window, &mut App)>,
    on_toggle_folder: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_file: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_doc: Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    on_close_doc: Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    on_new_draft: Arc<dyn Fn(&mut Window, &mut App)>,
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
                        let mut content = div().flex().flex_col().gap_1();

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

                        // Draft Virtual Group (Fixed at the top of Files view)
                        let draft_doc = open_docs.iter().find(|d| d.key.is_scratch());
                        let has_open_draft = draft_doc.is_some();
                        let is_draft_active = draft_doc.map_or(false, |d| active_key == Some(&d.key));
                        let is_draft_dirty = if is_draft_active {
                            is_active_dirty
                        } else if let Some(d) = draft_doc {
                            d.doc.status == glidemark_core::document::DocumentStatus::Modified
                        } else {
                            false
                        };

                        let draft_title = draft_doc.map_or_else(|| "草稿 (未打开)".to_string(), |d| d.doc.display_name());
                        let draft_key_opt = draft_doc.map(|d| d.key.clone());

                        let t_switch_draft = on_select_doc.clone();
                        let t_close_draft = on_close_doc.clone();
                        let t_create_draft = on_new_draft.clone();

                        let mut draft_item = div()
                            .id("drawer_draft_item")
                            .group("drawer_draft_row")
                            .w_full()
                            .px_2()
                            .py_1()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_xs()
                            .tooltip(move |_window, cx| {
                                let t = if has_open_draft {
                                    "草稿 (⌘N 聚焦)".to_string()
                                } else {
                                    "点击新建草稿 (⌘N)".to_string()
                                };
                                cx.new(|_| DrawerTooltipView { text: t }).into()
                            })
                            .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                if let Some(key) = &draft_key_opt {
                                    t_switch_draft(key, window, cx);
                                } else {
                                    t_create_draft(window, cx);
                                }
                            });

                        if is_draft_active {
                            draft_item = draft_item
                                .bg(theme.warm_pill_bg())
                                .text_color(theme.warm_pill_text())
                                .font_weight(FontWeight::MEDIUM);
                        } else {
                            draft_item = draft_item
                                .text_color(if has_open_draft { theme.text_primary } else { theme.text_muted })
                                .hover(|s| s.bg(theme.surface));
                        }

                        let draft_right = {
                            let mut right = div().flex().items_center().gap_1();
                            if is_draft_dirty {
                                right = right.child(
                                    div()
                                        .w(px(6.))
                                        .h(px(6.))
                                        .rounded_full()
                                        .bg(theme.accent),
                                );
                            }
                            if let Some(d_key) = draft_doc.map(|d| d.key.clone()) {
                                let on_close = t_close_draft.clone();
                                right = right.child(
                                    div()
                                        .id("btn_close_draft")
                                        .w(px(16.))
                                        .h(px(16.))
                                        .rounded(px(3.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(12.))
                                        .text_color(theme.text_muted)
                                        .opacity(0.0)
                                        .group_hover("drawer_draft_row", |s| s.opacity(1.0))
                                        .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                                        .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                            cx.stop_propagation();
                                            on_close(&d_key, window, cx);
                                        })
                                        .child("×"),
                                );
                            }
                            right
                        };

                        let draft_section = div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .px_2()
                                    .pt_1()
                                    .pb_0p5()
                                    .text_size(px(10.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_muted)
                                    .child("草稿")
                            )
                            .child(
                                draft_item
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_1p5()
                                            .min_w(px(0.))
                                            .child(
                                                svg()
                                                    .path("icons/files.svg")
                                                    .size(px(13.))
                                                    .text_color(if is_draft_active { theme.warm_pill_text() } else { theme.text_muted })
                                            )
                                            .child(
                                                div()
                                                    .min_w(px(0.))
                                                    .truncate()
                                                    .child(draft_title)
                                            )
                                    )
                                    .child(draft_right)
                            );

                        content = content.child(draft_section);

                        // File Tree Section
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
                            let mut tree_container = div().flex().flex_col().gap_0p5().child(
                                div()
                                    .px_2()
                                    .pt_1()
                                    .pb_0p5()
                                    .text_size(px(10.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_muted)
                                    .child("工作区文件")
                            );

                            for (idx, row) in tree_rows.iter().enumerate() {
                                let depth_indent = (row.depth as f32) * 12.0;
                                if row.is_dir {
                                    let p = row.path.clone();
                                    let on_toggle = on_toggle_folder.clone();
                                    let is_expanded = row.is_expanded;
                                    let folder_name = row.name.clone();

                                    tree_container = tree_container.child(
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
                                    let p = row.path.clone();
                                    let target_key = DocKey::Path(p.clone());
                                    let is_active = active_key == Some(&target_key);
                                    let open_doc = open_docs.iter().find(|d| d.key == target_key);
                                    let is_open = open_doc.is_some();
                                    let is_dirty = if is_active {
                                        is_active_dirty
                                    } else if let Some(od) = open_doc {
                                        od.doc.status == glidemark_core::document::DocumentStatus::Modified
                                    } else {
                                        false
                                    };

                                    let file_name = row.name.clone();
                                    let tooltip_name = file_name.clone();
                                    let on_select = on_select_file.clone();
                                    let on_close = on_close_doc.clone();
                                    let close_key = target_key.clone();

                                    let mut item = div()
                                        .id(("drawer_tree_file", idx))
                                        .group("drawer_file_row")
                                        .w_full()
                                        .pl(px(depth_indent + 8.0))
                                        .pr_1p5()
                                        .py_1()
                                        .rounded(px(6.))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .justify_between()
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

                                    let left_part = div()
                                        .flex()
                                        .items_center()
                                        .gap_1p5()
                                        .min_w(px(0.))
                                        .child(
                                            if is_open && !is_active {
                                                div()
                                                    .w(px(5.))
                                                    .h(px(5.))
                                                    .rounded_full()
                                                    .bg(theme.text_muted)
                                            } else {
                                                div().w(px(5.)).h(px(5.))
                                            }
                                        )
                                        .child(
                                            div()
                                                .min_w(px(0.))
                                                .truncate()
                                                .child(file_name)
                                        );

                                    let mut right_part = div().flex().items_center().gap_1();
                                    if is_dirty {
                                        right_part = right_part.child(
                                            div()
                                                .w(px(6.))
                                                .h(px(6.))
                                                .rounded_full()
                                                .bg(theme.accent)
                                        );
                                    }

                                    if is_open {
                                        right_part = right_part.child(
                                            div()
                                                .id(("btn_close_file", idx))
                                                .w(px(16.))
                                                .h(px(16.))
                                                .rounded(px(3.))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .text_size(px(12.))
                                                .text_color(theme.text_muted)
                                                .opacity(0.0)
                                                .group_hover("drawer_file_row", |s| s.opacity(1.0))
                                                .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                                                .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                                    cx.stop_propagation();
                                                    on_close(&close_key, window, cx);
                                                })
                                                .child("×")
                                        );
                                    }

                                    tree_container = tree_container.child(item.child(left_part).child(right_part));
                                }
                            }
                            content = content.child(tree_container);
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
