use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;
use glidemark_core::ast::HeadingItem;
use glidemark_core::i18n::I18nStrings;

use crate::state::{root_label, DocKey, DrawerTab, FileTreeNode, OpenDoc, WorkspaceRoot};
use crate::theme::ThemeColors;

#[derive(Clone, Default)]
pub struct DrawerSearchParams {
    pub search_query: String,
    pub has_search_matches: bool,
    pub search_in_progress: bool,
    pub search_truncated: bool,
    pub on_clear_search: Option<Arc<dyn Fn(&mut Window, &mut App)>>,
    pub on_focus_search: Option<Arc<dyn Fn(&mut Window, &mut App)>>,
}

impl DrawerSearchParams {
    #[allow(dead_code)]
    pub fn from_state(state: &crate::state::AppState, on_clear: Option<Arc<dyn Fn(&mut Window, &mut App)>>) -> Self {
        Self {
            search_query: state.search_query.clone(),
            has_search_matches: state.has_search_matches(),
            search_in_progress: state.search_in_progress,
            search_truncated: state.search_truncated,
            on_clear_search: on_clear,
            on_focus_search: None,
        }
    }
}

#[allow(dead_code)]
pub fn render_drawer(
    is_fullscreen: bool,
    active_tab: DrawerTab,
    workspace_roots: &[WorkspaceRoot],
    loose_docs: &[DocKey],
    open_docs: &[OpenDoc],
    active_key: Option<&DocKey>,
    headings: &[HeadingItem],
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    scroll_handle: &ScrollHandle,
    on_select_tab: Arc<dyn Fn(DrawerTab, &mut Window, &mut App)>,
    on_toggle_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_remove_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_refresh_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_toggle_folder: Arc<dyn Fn(usize, &PathBuf, &mut Window, &mut App)>,
    on_select_file: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_doc: Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    on_close_doc: Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    on_new_draft: Arc<dyn Fn(&mut Window, &mut App)>,
    on_select_heading: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_open_folder: Arc<dyn Fn(&mut Window, &mut App)>,
    on_toggle_drawer: Arc<dyn Fn(&mut Window, &mut App)>,
    on_new_file_in_dir: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
) -> impl IntoElement {
    render_drawer_with_search(
        is_fullscreen,
        active_tab,
        workspace_roots,
        loose_docs,
        open_docs,
        active_key,
        headings,
        theme,
        strings,
        scroll_handle,
        &DrawerSearchParams::default(),
        on_select_tab,
        on_toggle_root,
        on_remove_root,
        on_refresh_root,
        on_toggle_folder,
        on_select_file,
        on_select_doc,
        on_close_doc,
        on_new_draft,
        on_select_heading,
        on_open_folder,
        on_toggle_drawer,
        on_new_file_in_dir,
    )
}

pub fn render_drawer_with_search(
    is_fullscreen: bool,
    active_tab: DrawerTab,
    workspace_roots: &[WorkspaceRoot],
    loose_docs: &[DocKey],
    open_docs: &[OpenDoc],
    active_key: Option<&DocKey>,
    headings: &[HeadingItem],
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    scroll_handle: &ScrollHandle,
    search: &DrawerSearchParams,
    on_select_tab: Arc<dyn Fn(DrawerTab, &mut Window, &mut App)>,
    on_toggle_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_remove_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_refresh_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_toggle_folder: Arc<dyn Fn(usize, &PathBuf, &mut Window, &mut App)>,
    on_select_file: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_doc: Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    on_close_doc: Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    _on_new_draft: Arc<dyn Fn(&mut Window, &mut App)>,
    on_select_heading: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_open_folder: Arc<dyn Fn(&mut Window, &mut App)>,
    on_toggle_drawer: Arc<dyn Fn(&mut Window, &mut App)>,
    on_new_file_in_dir: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
) -> impl IntoElement {
    let t1 = on_select_tab.clone();
    let t2 = on_select_tab.clone();
    let t_folder = on_open_folder.clone();
    let t_toggle = on_toggle_drawer.clone();

    let is_macos = cfg!(target_os = "macos");
    let drawer_header_pl = px(crate::views::workspace::adaptive_header_padding(is_macos, is_fullscreen, false));

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
                    &format!("{} (⌘B)", strings.collapse_sidebar),
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
                .child(render_icon_btn(
                    "drawer_add_folder_btn",
                    strings.add_folder,
                    "icons/folder-plus.svg",
                    false,
                    theme,
                    move |_, window, cx| t_folder(window, cx),
                ))
                .child(
                    div()
                        .id("drawer_header_drag_area")
                        .flex_1()
                        .h_full()
                        .window_control_area(WindowControlArea::Drag)
                        .on_mouse_down(MouseButton::Left, |_e, window, _| {
                            window.start_window_move();
                        }),
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

                        // ==========================================
                        // 0. Search Row
                        // ==========================================
                        let is_searching = !search.search_query.is_empty();

                        let mut search_row = div()
                            .id("drawer_search_container")
                            .w_full()
                            .h(px(28.))
                            .px_2()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.surface)
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_1p5();

                        if let Some(ref on_focus) = search.on_focus_search {
                            let on_focus_cb = on_focus.clone();
                            search_row = search_row
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                    on_focus_cb(window, cx);
                                });
                        }

                        let search_left = div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(theme.text_muted)
                                    .child("🔍"),
                            )
                            .child(
                                div()
                                    .id("drawer_search_query_display")
                                    .min_w(px(0.))
                                    .truncate()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(if is_searching {
                                        theme.text_primary
                                    } else {
                                        theme.text_muted
                                    })
                                    .child(if is_searching {
                                        search.search_query.clone()
                                    } else {
                                        strings.search_placeholder.to_string()
                                    }),
                            );

                        search_row = search_row.child(search_left);

                        if is_searching {
                            let on_clear = search.on_clear_search.clone();
                            let clear_btn = div()
                                .id("btn_clear_search")
                                .w(px(16.))
                                .h(px(16.))
                                .rounded(px(3.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_size(px(12.))
                                .text_color(theme.text_muted)
                                .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                                .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                    cx.stop_propagation();
                                    if let Some(ref cb) = on_clear {
                                        cb(window, cx);
                                    }
                                })
                                .child("×");
                            search_row = search_row.child(clear_btn);
                        }

                        content = content.child(search_row);

                        // Status indicators below search input
                        if is_searching && !search.has_search_matches {
                            content = content.child(
                                div()
                                    .id("drawer_search_no_results")
                                    .px_2()
                                    .py_1()
                                    .text_size(px(11.))
                                    .text_color(theme.text_muted)
                                    .child(strings.search_no_results),
                            );
                        }

                        if search.search_in_progress {
                            content = content.child(
                                div()
                                    .id("drawer_search_scanning")
                                    .px_2()
                                    .py_0p5()
                                    .text_size(px(11.))
                                    .text_color(theme.text_muted)
                                    .child(strings.search_scanning),
                            );
                        }

                        if search.search_truncated {
                            content = content.child(
                                div()
                                    .id("drawer_search_incomplete")
                                    .px_2()
                                    .py_0p5()
                                    .text_size(px(11.))
                                    .text_color(theme.conflict_border)
                                    .child(strings.search_incomplete),
                            );
                        }

                        // ==========================================
                        // 1. Draft Item (decentralized: rendered only when an active/open scratch doc exists)
                        // ==========================================
                        if let Some(draft_doc) = open_docs.iter().find(|d| d.key.is_scratch()) {
                            let draft_title = draft_doc.doc.display_name_with_fallback(strings.untitled_file);
                            let heading_match = draft_doc
                                .doc
                                .parsed
                                .headings()
                                .first()
                                .map(|h| h.text.to_lowercase().contains(&search.search_query.to_lowercase()))
                                .unwrap_or(false);
                            let show_draft = search.search_query.is_empty()
                                || draft_title.to_lowercase().contains(&search.search_query.to_lowercase())
                                || heading_match;
                            if show_draft {
                                let is_draft_active = active_key == Some(&draft_doc.key);
                                let d_key = draft_doc.key.clone();
                                let t_switch_draft = on_select_doc.clone();
                                let t_close_draft = on_close_doc.clone();

                                let close_key = d_key.clone();
                                let on_close = t_close_draft.clone();
                            let draft_right = div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
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
                                            on_close(&close_key, window, cx);
                                        })
                                        .child("×"),
                                );

                            let draft_tooltip_text = format!("{} (⌘N)", strings.draft_focus_hint);
                            let sel_key = d_key.clone();
                            let mut draft_item = div()
                                .id("drawer_draft_item")
                                .group("drawer_draft_row")
                                .w_full()
                                .h(px(28.))
                                .px_2()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_between()
                                .text_size(px(13.))
                                .font_weight(FontWeight::NORMAL)
                                .tooltip(move |_window, cx| {
                                    let t = draft_tooltip_text.clone();
                                    cx.new(|_| DrawerTooltipView { text: t }).into()
                                })
                                .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                    t_switch_draft(&sel_key, window, cx);
                                });

                            if is_draft_active {
                                draft_item = draft_item
                                    .bg(theme.selection_bg())
                                    .text_color(theme.selection_text())
                                    .font_weight(FontWeight::MEDIUM);
                            } else {
                                draft_item = draft_item
                                    .text_color(theme.text_primary)
                                    .hover(|s| s.bg(theme.hover_bg()));
                            }

                            let draft_item = draft_item
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
                                                .text_color(if is_draft_active {
                                                    theme.selection_text()
                                                } else {
                                                    theme.text_muted
                                                }),
                                        )
                                        .child(
                                            div()
                                                .min_w(px(0.))
                                                .truncate()
                                                .child(draft_title),
                                        ),
                                )
                                .child(draft_right);

                            content = content.child(draft_item);
                            }
                        }

                        // ==========================================
                        // 2. Section 2: Workspace Roots Group (工作区多根组)
                        // ==========================================
                        let mut roots_section = div()
                            .flex()
                            .flex_col()
                            .gap_0p5();

                        if workspace_roots.is_empty() {
                            roots_section = roots_section.child(
                                div()
                                    .px_2()
                                    .py_2()
                                    .flex()
                                    .items_center()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(theme.text_muted)
                                    .child(strings.workspace_empty),
                            );
                        } else {
                            for (root_idx, root) in workspace_roots.iter().enumerate() {
                                let root_path_str = root.path.to_string_lossy().to_string();
                                let root_name = root_label(workspace_roots, root_idx);
                                let is_missing = root.is_missing;
                                let is_deferred = root.deferred;
                                let is_expanded = root.is_expanded;
                                let is_truncated = root.truncated;
                                let on_toggle_r = on_toggle_root.clone();
                                let on_refresh_r = on_refresh_root.clone();
                                let on_remove_r = on_remove_root.clone();

                                let mut root_header = div()
                                    .id(SharedString::from(format!("drawer_root_header_{}", root_idx)))
                                    .group("drawer_root_row")
                                    .w_full()
                                    .h(px(28.))
                                    .px_2()
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .hover(|s| s.bg(theme.hover_bg()))
                                    .tooltip(move |_window, cx| {
                                        let t = root_path_str.clone();
                                        cx.new(|_| DrawerTooltipView { text: t }).into()
                                    });

                                let on_refresh_header = on_refresh_r.clone();
                                let on_remove_header = on_remove_r.clone();
                                root_header = root_header
                                    .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                        if is_missing {
                                            return;
                                        }
                                        if is_deferred {
                                            on_refresh_header(root_idx, window, cx);
                                        } else {
                                            on_toggle_r(root_idx, window, cx);
                                        }
                                    })
                                    .on_mouse_down(MouseButton::Right, move |_e, window, cx| {
                                        cx.stop_propagation();
                                        on_remove_header(root_idx, window, cx);
                                    });

                                let folder_icon = if is_expanded {
                                    "icons/folder-open.svg"
                                } else {
                                    "icons/folder.svg"
                                };

                                let left_part = div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .min_w(px(0.))
                                    .child(
                                        svg()
                                            .path(folder_icon)
                                            .size(px(14.))
                                            .text_color(theme.text_muted),
                                    )
                                    .child(
                                        div()
                                            .min_w(px(0.))
                                            .truncate()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(if is_missing { theme.text_muted } else { theme.text_primary })
                                            .child(root_name),
                                    )
                                    .when(is_missing, |d| {
                                        d.child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(theme.text_muted)
                                                .child(format!("({})", strings.missing_root)),
                                        )
                                    })
                                    .when(is_deferred, |d| {
                                        d.child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(theme.accent)
                                                .child(format!("({})", strings.deferred_hint)),
                                        )
                                    });

                                let on_new_file_root = on_new_file_in_dir.clone();
                                let root_dir_path = root.path.clone();
                                let right_part = div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("btn_new_file_root_{}", root_idx)))
                                            .w(px(16.))
                                            .h(px(16.))
                                            .rounded(px(3.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(12.))
                                            .text_color(theme.text_muted)
                                            .opacity(0.0)
                                            .group_hover("drawer_root_row", |s| s.opacity(1.0))
                                            .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                                            .tooltip(move |_window, cx| {
                                                let t = strings.new_file.to_string();
                                                cx.new(|_| DrawerTooltipView { text: t }).into()
                                            })
                                            .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                                cx.stop_propagation();
                                                on_new_file_root(&root_dir_path, window, cx);
                                            })
                                            .child("+"),
                                    );

                                root_header = root_header.child(left_part).child(right_part);
                                roots_section = roots_section.child(root_header);

                                if is_expanded && !is_missing {
                                    let mut root_children_container = div().flex().flex_col().gap_0p5();

                                    if root.tree.is_empty() && !is_deferred {
                                        root_children_container = root_children_container.child(
                                            div()
                                                .pl(px(24.))
                                                .py_1()
                                                .text_size(px(11.))
                                                .text_color(theme.text_muted)
                                                .child(strings.no_markdown_files),
                                        );
                                    } else {
                                        let mut tree_elements = Vec::new();
                                        render_file_tree_nodes(
                                            &root.tree,
                                            0,
                                            root_idx,
                                            open_docs,
                                            active_key,
                                            theme,
                                            strings,
                                            &search.search_query,
                                            &on_toggle_folder,
                                            &on_new_file_in_dir,
                                            &on_select_file,
                                            &on_close_doc,
                                            &mut tree_elements,
                                        );
                                        root_children_container = root_children_container.children(tree_elements);
                                    }

                                    if is_truncated {
                                        let on_refresh = on_refresh_r.clone();
                                        root_children_container = root_children_container.child(
                                            div()
                                                .id(SharedString::from(format!("btn_retry_truncated_{}", root_idx)))
                                                .pl(px(24.))
                                                .pr_2()
                                                .py_1()
                                                .rounded(px(4.))
                                                .cursor_pointer()
                                                .flex()
                                                .items_center()
                                                .gap_1p5()
                                                .hover(|s| s.bg(theme.hover_bg()))
                                                .tooltip(move |_window, cx| {
                                                    let t = strings.retry_scan.to_string();
                                                    cx.new(|_| DrawerTooltipView { text: t }).into()
                                                })
                                                .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                                    cx.stop_propagation();
                                                    on_refresh(root_idx, window, cx);
                                                })
                                                .child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .text_color(theme.text_muted)
                                                        .child(format!("({})", strings.truncated_hint)),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .text_color(theme.accent)
                                                        .child(format!("· {}", strings.retry_scan)),
                                                ),
                                        );
                                    }

                                    roots_section = roots_section.child(root_children_container);
                                }
                            }
                        }

                        content = content.child(roots_section);

                        // ==========================================
                        // 3. Section 3: Loose Files Group (外部文件组)
                        // ==========================================
                        let loose_file_paths: Vec<&PathBuf> = loose_docs
                            .iter()
                            .filter_map(|k| match k {
                                DocKey::Path(p) => {
                                    if !search.search_query.is_empty() {
                                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                                        if !name.to_lowercase().contains(&search.search_query.to_lowercase()) {
                                            return None;
                                        }
                                    }
                                    Some(p)
                                }
                                DocKey::Scratch(_) => None,
                            })
                            .collect();

                        if !loose_file_paths.is_empty() {
                            let mut loose_section = div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .px_2()
                                        .pt_3()
                                        .pb_0p5()
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.text_muted)
                                        .child(strings.loose_files),
                                );

                            for (idx, p) in loose_file_paths.iter().enumerate() {
                                let target_key = DocKey::Path((*p).clone());
                                let is_active = active_key == Some(&target_key);
                                let file_name = p
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(strings.untitled_file)
                                    .to_string();
                                let tooltip_path = p.to_string_lossy().to_string();
                                let on_select = on_select_doc.clone();
                                let on_close = on_close_doc.clone();
                                let sel_key = target_key.clone();
                                let close_key = target_key.clone();

                                let mut item = div()
                                    .id(SharedString::from(format!("drawer_loose_file_{}", idx)))
                                    .group("drawer_loose_file_row")
                                    .w_full()
                                    .h(px(28.))
                                    .px_2()
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::NORMAL)
                                    .tooltip(move |_window, cx| {
                                        let t = tooltip_path.clone();
                                        cx.new(|_| DrawerTooltipView { text: t }).into()
                                    })
                                    .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                                        on_select(&sel_key, window, cx);
                                    });

                                if is_active {
                                    item = item
                                        .bg(theme.selection_bg())
                                        .text_color(theme.selection_text())
                                        .font_weight(FontWeight::MEDIUM);
                                } else {
                                    item = item
                                        .text_color(theme.text_primary)
                                        .hover(|s| s.bg(theme.hover_bg()));
                                }

                                let left_part = div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .min_w(px(0.))
                                    .child(
                                        svg()
                                            .path("icons/files.svg")
                                            .size(px(13.))
                                            .text_color(if is_active {
                                                theme.selection_text()
                                            } else {
                                                theme.text_muted
                                            }),
                                    )
                                    .child(
                                        div()
                                            .min_w(px(0.))
                                            .truncate()
                                            .child(file_name),
                                    );

                                let right_part = div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("btn_close_loose_file_{}", idx)))
                                            .w(px(16.))
                                            .h(px(16.))
                                            .rounded(px(3.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_size(px(12.))
                                            .text_color(theme.text_muted)
                                            .opacity(0.0)
                                            .group_hover("drawer_loose_file_row", |s| s.opacity(1.0))
                                            .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                                            .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                                cx.stop_propagation();
                                                on_close(&close_key, window, cx);
                                            })
                                            .child("×"),
                                    );

                                loose_section = loose_section.child(item.child(left_part).child(right_part));
                            }

                            content = content.child(loose_section);
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
                                        .hover(|s| s.bg(theme.hover_bg()).text_color(theme.accent))
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

fn dir_matches_search(node: &FileTreeNode, q_lower: &str) -> bool {
    if node.name.to_lowercase().contains(q_lower) {
        return true;
    }
    crate::state::node_tree_has_match(&node.children, q_lower)
}

fn render_file_tree_nodes(
    nodes: &[FileTreeNode],
    depth: usize,
    root_idx: usize,
    open_docs: &[OpenDoc],
    active_key: Option<&DocKey>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    search_query: &str,
    on_toggle_folder: &Arc<dyn Fn(usize, &PathBuf, &mut Window, &mut App)>,
    on_new_file_in_dir: &Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_file: &Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_close_doc: &Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    out: &mut Vec<AnyElement>,
) {
    let q_lower = search_query.to_lowercase();
    for (node_idx, node) in nodes.iter().enumerate() {
        let depth_indent = (depth as f32 + 1.0) * 12.0;
        if node.is_dir {
            if !search_query.is_empty() && !dir_matches_search(node, &q_lower) {
                continue;
            }
            let p = node.path.clone();
            let on_toggle = on_toggle_folder.clone();
            let is_expanded = node.is_expanded;
            let folder_name = node.name.clone();

            let folder_icon = if is_expanded {
                "icons/folder-open.svg"
            } else {
                "icons/folder.svg"
            };

            let left_part = div()
                .flex()
                .items_center()
                .gap_1p5()
                .min_w(px(0.))
                .child(
                    svg()
                        .path(folder_icon)
                        .size(px(13.))
                        .text_color(theme.text_muted),
                )
                .child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .child(folder_name),
                );

            let on_new_file_dir = on_new_file_in_dir.clone();
            let dir_path = p.clone();
            let right_part = div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .id(SharedString::from(format!("btn_new_file_dir_{}_{}_{}", root_idx, depth, node_idx)))
                        .w(px(16.))
                        .h(px(16.))
                        .rounded(px(3.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.))
                        .text_color(theme.text_muted)
                        .opacity(0.0)
                        .group_hover("drawer_dir_row", |s| s.opacity(1.0))
                        .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                        .tooltip(move |_window, cx| {
                            let t = strings.new_file.to_string();
                            cx.new(|_| DrawerTooltipView { text: t }).into()
                        })
                        .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                            cx.stop_propagation();
                            on_new_file_dir(&dir_path, window, cx);
                        })
                        .child("+"),
                );

            out.push(
                div()
                    .id(SharedString::from(format!("drawer_tree_dir_{}_{}_{}", root_idx, depth, node_idx)))
                    .group("drawer_dir_row")
                    .w_full()
                    .h(px(28.))
                    .pl(px(depth_indent + 4.0))
                    .pr_2()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.text_muted)
                    .hover(|s| s.bg(theme.hover_bg()).text_color(theme.text_primary))
                    .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                        on_toggle(root_idx, &p, window, cx);
                    })
                    .child(left_part)
                    .child(right_part)
                    .into_any_element(),
            );

            if is_expanded {
                render_file_tree_nodes(
                    &node.children,
                    depth + 1,
                    root_idx,
                    open_docs,
                    active_key,
                    theme,
                    strings,
                    search_query,
                    on_toggle_folder,
                    on_new_file_in_dir,
                    on_select_file,
                    on_close_doc,
                    out,
                );
            }
        } else {
            if !search_query.is_empty() && !node.name.to_lowercase().contains(&q_lower) {
                continue;
            }
            let p = node.path.clone();
            let target_key = DocKey::Path(p.clone());
            let is_active = active_key == Some(&target_key);
            let open_doc = open_docs.iter().find(|d| d.key == target_key);
            let is_open = open_doc.is_some();

            let file_name = node.name.clone();
            let tooltip_name = file_name.clone();
            let on_select = on_select_file.clone();
            let on_close = on_close_doc.clone();
            let close_key = target_key.clone();

            let mut item = div()
                .id(SharedString::from(format!("drawer_tree_file_{}_{}_{}", root_idx, depth, node_idx)))
                .group("drawer_file_row")
                .w_full()
                .h(px(44.))
                .pl(px(depth_indent + 8.0))
                .pr_1p5()
                .py_1()
                .rounded(px(6.))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(13.))
                .font_weight(FontWeight::NORMAL)
                .tooltip(move |_window, cx| {
                    let t = tooltip_name.clone();
                    cx.new(|_| DrawerTooltipView { text: t }).into()
                })
                .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                    on_select(&p, window, cx);
                });

            if is_active {
                item = item
                    .bg(theme.selection_bg())
                    .text_color(theme.selection_text())
                    .font_weight(FontWeight::MEDIUM);
            } else {
                item = item
                    .text_color(theme.text_primary)
                    .hover(|s| s.bg(theme.hover_bg()));
            }

            let mut left_part = div()
                .flex()
                .flex_col()
                .justify_center()
                .min_w(px(0.))
                .child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .text_size(px(13.))
                        .child(file_name),
                );

            if let Some(ref mtime_str) = node.mtime {
                left_part = left_part.child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .text_size(px(11.))
                        .text_color(theme.text_muted)
                        .child(mtime_str.clone()),
                );
            }

            let mut right_part = div().flex().items_center().gap_1();

            if is_open {
                right_part = right_part.child(
                    div()
                        .id(SharedString::from(format!("btn_close_file_{}_{}_{}", root_idx, depth, node_idx)))
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
                        .child("×"),
                );
            }

            out.push(item.child(left_part).child(right_part).into_any_element());
        }
    }
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

    let color = if active { theme.selection_text() } else { theme.text_muted };
    let icon_el = svg().path(icon_path).size(px(16.)).text_color(color);

    if active {
        base.bg(theme.selection_bg())
            .rounded(px(6.))
            .child(icon_el)
    } else {
        base.rounded(px(6.))
            .hover(|s| s.bg(theme.hover_bg()).text_color(theme.text_primary))
            .child(icon_el)
    }
}
