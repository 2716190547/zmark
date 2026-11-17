use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;
use zmark_core::ast::HeadingItem;
use zmark_core::i18n::I18nStrings;

use crate::state::{root_label, DocKey, DrawerTab, FileTreeNode, OpenDoc, WorkspaceRoot};
use crate::theme::ThemeColors;
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};

pub use gpui_component::input::InputState as SearchInput;

#[derive(Clone, Default)]
pub struct DrawerSearchParams {
    pub search_query: String,
    pub has_search_matches: bool,
    pub search_in_progress: bool,
    pub search_truncated: bool,
    pub on_clear_search: Option<Arc<dyn Fn(&mut Window, &mut App)>>,
    pub on_focus_search: Option<Arc<dyn Fn(&mut Window, &mut App)>>,
    pub search_input: Option<Entity<SearchInput>>,
    pub focused: bool,
    pub selected_directory: Option<PathBuf>,
    pub selection: crate::views::sidebar_selection::SharedSelection,
    pub hidden_paths: Vec<PathBuf>,
    pub reduce_motion: bool,
    pub on_row: Option<Arc<dyn Fn(&DocKey, &MouseDownEvent, bool, &mut Window, &mut App)>>,
    pub on_trash_directory: Option<Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>>,
    pub on_remove_path: Option<Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>>,
    pub undo_available: bool,
    pub on_undo: Option<Arc<dyn Fn(&mut Window, &mut App)>>,

    pub folder_values: std::collections::HashMap<PathBuf, f32>,
    pub rename_target: Option<DocKey>,
    pub rename_input: Option<Entity<SearchInput>>,
    pub on_select_directory: Option<Arc<dyn Fn(&PathBuf, bool, &mut Window, &mut App)>>,
    pub on_cancel_rename: Option<Arc<dyn Fn(&mut Window, &mut App)>>,
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
    _on_remove_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_refresh_root: Arc<dyn Fn(usize, &mut Window, &mut App)>,
    on_toggle_folder: Arc<dyn Fn(usize, &PathBuf, &mut Window, &mut App)>,
    on_select_file: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_rename_file: Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_doc: Arc<dyn Fn(&DocKey, bool, &mut Window, &mut App)>,
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

    let is_searching = !search.search_query.is_empty();

    let mut search_row = div()
        .id("drawer_search_container")
        .debug_selector(|| "drawer_search_container".to_string())
        .w_full()
        .h(px(28.))
        .px(px(8.))
        .rounded_full()
        .bg(theme.text_primary.opacity(0.04))
        .hover(|s| s.bg(theme.text_primary.opacity(0.06)))
        .when(search.focused, |s| {
            s.border(px(1.))
                .border_color(theme.text_primary.opacity(0.08))
        })
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
        .gap_2()
        .min_w(px(0.))
        .text_color(theme.text_primary)
        .flex_1()
        .child(
            svg()
                .path("icons/search.svg")
                .size(px(14.))
                .text_color(theme.text_muted),
        )
        .child(if let Some(ref input) = search.search_input {
            div()
                .id("drawer_search_input_host")
                .min_w(px(0.))
                .flex_1()
                .child(render_input(input, false))
                .into_any_element()
        } else {
            div()
                .id("drawer_search_query_display")
                .min_w(px(0.))
                .flex_1()
                .truncate()
                .text_size(px(13.))
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
                })
                .into_any_element()
        });

    search_row = search_row.child(search_left);

    if is_searching {
        let on_clear = search.on_clear_search.clone();
        let clear_btn = div()
            .id("btn_clear_search")
            .w(px(20.))
            .h(px(20.))
            .rounded_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(theme.text_muted)
            .hover(|s| s.bg(theme.text_primary.opacity(0.08)).text_color(theme.text_primary))
            .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                cx.stop_propagation();
                if let Some(ref cb) = on_clear {
                    cb(window, cx);
                }
            })
            .child("×");
        search_row = search_row.child(clear_btn);
    }

    let mut fixed_search_area = div()
        .id("drawer_search_fixed_container")
        .px_2()
        .pt_2()
        .flex()
        .flex_col()
        .gap_1()
        .child(search_row);

    // Status indicators below search input
    if !search.search_in_progress && is_searching && !search.has_search_matches {
        fixed_search_area = fixed_search_area.child(
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
        fixed_search_area = fixed_search_area.child(
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
        fixed_search_area = fixed_search_area.child(
            div()
                .id("drawer_search_incomplete")
                .px_2()
                .py_0p5()
                .text_size(px(11.))
                .text_color(theme.conflict_border)
                .child(strings.search_incomplete),
        );
    }

    let mut root_container = div()
        .w(px(crate::views::editor::DRAWER_WIDTH))
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
                    &format!("{} ({})", strings.collapse_sidebar, crate::platform::shortcut_hint("Shift+B")),
                    "icons/panel-left.svg",
                    false,
                    theme,
                    move |_, window, cx| t_toggle(window, cx),
                ))
                .child(render_icon_btn(
                    "drawer_tab_files",
                    strings.tab_files,
                    "icons/folder.svg",
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
                    strings.open_file_or_folder,
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
                        .occlude()
                        .window_control_area(WindowControlArea::Drag)
                        .when(!cfg!(target_os = "windows"), |el| {
                            el.on_mouse_down(MouseButton::Left, |_e, window, _| {
                                window.start_window_move();
                            })
                        }),
                ),
        );

    if active_tab == DrawerTab::Files {
        root_container = root_container.child(fixed_search_area);
    }

    root_container.child(
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
                    search.selection.lock().unwrap().rows.clear();
                    let mut content = div().relative().flex().flex_col().gap_1()
                        .child(crate::views::sidebar_selection::selection_layer(search.selection.clone(), theme.control_selection_bg(), search.reduce_motion));

                    // ==========================================
                    // 1. Draft Item (decentralized: rendered only when an active/open scratch doc exists)
                    // ==========================================
                    if let Some(draft_doc) = open_docs.iter().find(|d| d.key.is_scratch()) {
                        let draft_title = draft_doc.doc.display_name_with_fallback(strings.untitled_file);
                        let show_draft = search.search_query.is_empty()
                            || draft_title.to_lowercase().contains(&search.search_query.to_lowercase());
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

                            let draft_tooltip_text = format!("{} ({})", strings.draft_focus_hint, crate::platform::shortcut_hint("N"));
                            let sel_key = d_key.clone();
                            let row_handler = search.on_row.clone();
                            let mut draft_item = div().relative()
                                .child(crate::views::sidebar_selection::row_anchor(search.selection.clone(), d_key.clone()))
                                .id("drawer_draft_item")
                                .debug_selector(|| "drawer_draft_item".to_string())
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
                                .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
                                    if event.click_count == 2 { cx.stop_propagation(); }
                                    if let Some(handler) = &row_handler { handler(&sel_key, event, true, window, cx); }
                                    else { t_switch_draft(&sel_key, event.click_count == 2, window, cx); }
                                });

                            if is_draft_active {
                                draft_item = draft_item

                                    .text_color(theme.selection_text())
                                    .font_weight(FontWeight::NORMAL);
                            } else {
                                draft_item = draft_item
                                    .text_color(theme.text_primary)
                                    ;
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
                                                .path("icons/file.svg")
                                                .size(px(14.)).flex_shrink_0()
                                                .text_color(if is_draft_active {
                                                    theme.selection_text()
                                                } else {
                                                    theme.text_muted
                                                }),
                                        )
                                        .child(
                                            inline_rename(&d_key, search).unwrap_or_else(|| div()
                                                .min_w(px(0.)).truncate().child(draft_title).into_any_element()),
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
                            .id("drawer_roots_section").debug_selector(|| "drawer_roots_section".into())
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
                                if search.hidden_paths.iter().any(|p| root.path.starts_with(p)) { continue; }
                                let root_path_str = root.path.to_string_lossy().to_string();
                                let root_name = root_label(workspace_roots, root_idx);
                                let is_missing = root.is_missing;
                                let is_deferred = root.deferred;
                                let is_expanded = root.is_expanded;
                                let value = search.folder_values.get(&root.path).copied().unwrap_or(if is_expanded { 1.0 } else { 0.0 });
                                let root_key = DocKey::Path(root.path.clone());
                                let select_directory = search.on_select_directory.clone();
                                let name_handler = search.on_row.clone();
                                let select_path = root.path.clone();
                                let is_truncated = root.truncated;
                                let on_toggle_r = on_toggle_root.clone();
                                let icon_handler = search.on_row.clone();
                                let icon_key = root_key.clone();
                                let on_refresh_r = on_refresh_root.clone();

                                let row_handler = search.on_row.clone();
                                let row_key = root_key.clone();
                                let mut root_header = div().relative()
                                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                        if let Some(handler) = &row_handler { handler(&row_key, event, false, window, cx); }
                                    })
                                    .id(SharedString::from(format!("drawer_root_header_{}", root_idx)))
                                    .debug_selector(move || format!("drawer_root_header_{root_idx}"))

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
                                    .font_weight(FontWeight::NORMAL)

                                    .tooltip(move |_window, cx| {
                                        let t = root_path_str.clone();
                                        cx.new(|_| DrawerTooltipView { text: t }).into()
                                    });

                                let folder_icon = if is_expanded {
                                    "icons/folder-open.svg"
                                } else {
                                    "icons/folder.svg"
                                };

                                let left_part = div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .min_w(px(0.)).flex_1()
                                    .child(
                                        div().id(SharedString::from(format!("drawer_root_toggle_{root_idx}")))
                                            .debug_selector(move || format!("drawer_root_toggle_{root_idx}"))
                                            .size(px(16.)).flex_shrink_0().flex().items_center().justify_center()
                                            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                                cx.stop_propagation();
                                                if let Some(handler) = &icon_handler { handler(&icon_key, event, false, window, cx); }
                                                else if !is_missing { on_toggle_r(root_idx, window, cx); }
                                            })
                                            .child(svg().path(folder_icon).size(px(14.)).flex_shrink_0().text_color(theme.text_muted)),
                                    )
                                    .child(
                                        inline_rename(&root_key, search).unwrap_or_else(|| div()
                                            .id(SharedString::from(format!("drawer_root_name_{root_idx}")))
                                            .debug_selector(move || format!("drawer_root_name_{root_idx}"))
                                            .min_w(px(0.)).truncate().text_size(px(13.)).font_weight(FontWeight::NORMAL)
                                            .text_color(if is_missing { theme.text_muted } else { theme.text_primary })
                                            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                                cx.stop_propagation();
                                                if let Some(handler) = &name_handler { handler(&DocKey::Path(select_path.clone()), event, true, window, cx); }
                                                else if let Some(select) = &select_directory { select(&select_path, event.click_count == 2, window, cx); }
                                            }).child(root_name).into_any_element()),
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
                                    .child(path_actions(root.path.clone(), "drawer_root_row", search, strings))
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
                                roots_section = roots_section.child(path_context_menu(root_header, root.path.clone(), search, theme, strings));

                                let mut closing_search = search.clone();
                                if !is_expanded && !is_missing {
                                    if let Some(file) = retained_file(&root.tree, &root.path, search) {
                                        let mut retained = Vec::new();
                                        render_file_tree_nodes(std::slice::from_ref(&file), 0, root_idx, open_docs, active_key,
                                            theme, strings, &search.search_query, search, &on_toggle_folder, &on_new_file_in_dir,
                                            &on_select_file, &on_rename_file, &on_close_doc, &mut retained);
                                        roots_section = roots_section.children(retained);
                                        closing_search.hidden_paths.push(file.path);
                                    }
                                }
                                if (is_expanded || value > 0.0) && !is_missing {
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
                                            &closing_search,
                                            &on_toggle_folder,
                                            &on_new_file_in_dir,
                                            &on_select_file,
                                            &on_rename_file,
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
                                                        .font_weight(FontWeight::NORMAL)
                                                        .text_color(theme.accent)
                                                        .child(format!("· {}", strings.retry_scan)),
                                                ),
                                        );
                                    }

                                    roots_section = roots_section.child(fading_children(root_children_container, is_expanded, value, root.path.clone()));
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
                                    if search.hidden_paths.iter().any(|hidden| p.starts_with(hidden)) { return None; }
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
                                        .font_weight(FontWeight::NORMAL)
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

                                let row_handler = search.on_row.clone();
                                let mut item = div().relative()
                                    .child(crate::views::sidebar_selection::row_anchor(search.selection.clone(), target_key.clone()))
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
                                    .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
                                        if event.click_count == 2 { cx.stop_propagation(); }
                                        if let Some(handler) = &row_handler { handler(&sel_key, event, true, window, cx); }
                                        else { on_select(&sel_key, event.click_count == 2, window, cx); }
                                    });

                                if is_active {
                                    item = item

                                        .text_color(theme.selection_text())
                                        .font_weight(FontWeight::NORMAL);
                                } else {
                                    item = item
                                        .text_color(theme.text_primary)
                                        ;
                                }

                                let left_part = div()
                                    .flex()
                                    .items_center()
                                    .gap_1p5()
                                    .min_w(px(0.))
                                    .child(
                                        svg()
                                            .path("icons/file.svg")
                                            .size(px(14.)).flex_shrink_0()
                                            .text_color(if is_active {
                                                theme.selection_text()
                                            } else {
                                                theme.text_muted
                                            }),
                                    )
                                    .child(
                                        inline_rename(&target_key, search).unwrap_or_else(|| div()
                                            .min_w(px(0.)).truncate().child(file_name).into_any_element()),
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
                        .tooltip(move |_, cx| cx.new(|_| DrawerTooltipView { text: strings.trash_file.to_string() }).into())
                                            .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                                            .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                                                cx.stop_propagation();
                                                on_close(&close_key, window, cx);
                                            })
                                            .child("×"),
                                    );

                                let right_part = if search.on_trash_directory.is_some() { path_actions((*p).clone(), "drawer_loose_file_row", search, strings) } else { right_part };
                                loose_section = loose_section.child(path_context_menu(item.child(left_part).child(right_part), (*p).clone(), search, theme, strings));
                            }

                            content = content.child(loose_section);
                        }

                        if search.undo_available {
                            if let Some(undo) = &search.on_undo {
                                let undo = undo.clone();
                                content = content.child(div().id("sidebar_undo").h(px(28.)).px_2().flex().items_center()
                                    .text_size(px(13.)).text_color(theme.text_primary).cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, move |_,window,cx| undo(window,cx))
                                    .gap_1p5()
                                    .child(svg().path("icons/undo.svg").size(px(14.)).text_color(theme.text_muted))
                                    .child(strings.undo_sidebar_remove)
                                    .child(div().text_size(px(11.)).text_color(theme.text_muted).child(if cfg!(target_os = "macos") { "⌘⌥Z / Ctrl+Alt+Z" } else { "Ctrl+Alt+Z" })));
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
                                        2 => (FontWeight::NORMAL, theme.text_primary),
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

pub fn render_input(input: &Entity<SearchInput>, title: bool) -> gpui_component::input::Input {
    // Input::h only configures multiline editors; single-line frames need Styled::h.
    Styled::h(gpui_component::input::Input::new(input), px(24.))
        .appearance(false).bordered(false).focus_bordered(false)
        .w_full().min_w(px(0.)).px_0().py_0()
        .line_height(px(if title { 24. } else { 20. }))
        .text_size(px(if title { 18. } else { 13. }))
        .font_family(crate::platform::UI_FONT)
        .font_weight(if title { FontWeight::BOLD } else { FontWeight::NORMAL })
}

fn inline_rename(key: &DocKey, params: &DrawerSearchParams) -> Option<AnyElement> {
    if params.rename_target.as_ref() != Some(key) { return None; }
    let input = params.rename_input.as_ref()?;
    Some(div().id("sidebar_rename_box").debug_selector(|| "sidebar_rename_box".into())
        .key_context("InlineRename")
        .min_w(px(0.)).w_full().h(px(20.))
        .text_size(px(13.)).font_weight(FontWeight::NORMAL).line_height(px(20.))
        .child(Styled::h(render_input(input, false), px(20.)))
        .when_some(params.on_cancel_rename.clone(), |el, cancel| {
            el.on_mouse_down_out(move |_, window, cx| cancel(window, cx))
        }).into_any_element())
}

const TREE_DIR_ROW_H: f32 = 28.0;
const TREE_FILE_ROW_H: f32 = 48.0;

/// Native measured reveal owns height; settled closed children are not built.
fn fading_children(children: Div, expanded: bool, value: f32, path: PathBuf) -> AnyElement {
    // Settled content must participate in normal layout, without nested reveal masks.
    if expanded && value >= 1.0 { return children.w_full().into_any_element(); }
    div().relative().w_full().flex_shrink_0().overflow_hidden()
        .child(gpui_base::Collapsible::new().w_full().open(expanded)
            .reveal(SharedString::from(format!("folder-reveal:{}", path.display())), value)
            .content(children.w_full().flex_shrink_0().opacity(value)))
        .when(!expanded, |el| {
            el.child(div().absolute().inset_0().occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation()))
        }).into_any_element()
}

/// A collapsed folder keeps only the singly selected file, with its real relative path.
/// The projection is display-only: the tree, file identity and rename target stay intact.
fn retained_file(nodes: &[FileTreeNode], parent: &std::path::Path, params: &DrawerSearchParams) -> Option<FileTreeNode> {
    let selected = {
        let selection = params.selection.lock().unwrap();
        if selection.selected.len() != 1 { return None; }
        selection.selected.iter().next()?.as_path()?.clone()
    };
    if !selected.starts_with(parent) || params.hidden_paths.iter().any(|path| selected.starts_with(path)) { return None; }
    fn find(nodes: &[FileTreeNode], path: &std::path::Path) -> Option<FileTreeNode> {
        for node in nodes {
            if !node.is_dir && node.path == path { return Some(node.clone()); }
            if node.is_dir && path.starts_with(&node.path) {
                if let Some(file) = find(&node.children, path) { return Some(file); }
            }
        }
        None
    }
    let mut node = find(nodes, &selected)?;
    node.name = selected.strip_prefix(parent).ok()?.to_string_lossy().into_owned();
    Some(node)
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
    params: &DrawerSearchParams,
    on_toggle_folder: &Arc<dyn Fn(usize, &PathBuf, &mut Window, &mut App)>,
    on_new_file_in_dir: &Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_select_file: &Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_rename_file: &Arc<dyn Fn(&PathBuf, &mut Window, &mut App)>,
    on_close_doc: &Arc<dyn Fn(&DocKey, &mut Window, &mut App)>,
    out: &mut Vec<AnyElement>,
) {
    let q_lower = search_query.to_lowercase();
    for (node_idx, node) in nodes.iter().enumerate() {
        if params.hidden_paths.iter().any(|p| node.path.starts_with(p)) { continue; }
        let depth_indent = (depth as f32 + 1.0) * 12.0;
        if node.is_dir {
            if !search_query.is_empty() && !dir_matches_search(node, &q_lower) {
                continue;
            }
            let p = node.path.clone();
            let on_toggle = on_toggle_folder.clone();
            let icon_handler = params.on_row.clone();
            let icon_key = DocKey::Path(node.path.clone());
            let is_expanded = node.is_expanded;
            let value = params.folder_values.get(&node.path).copied().unwrap_or(if is_expanded { 1.0 } else { 0.0 });
            let folder_name = node.name.clone();
            let key = DocKey::Path(node.path.clone());
            let select_directory = params.on_select_directory.clone();
            let name_handler = params.on_row.clone();
            let select_path = node.path.clone();
            let toggle_selector = format!("drawer_dir_toggle:{}", node.path.display());
            let name_selector = format!("drawer_dir_name:{}", node.path.display());

            let folder_icon = if is_expanded {
                "icons/folder-open.svg"
            } else {
                "icons/folder.svg"
            };

            let left_part = div()
                .flex()
                .items_center()
                .gap_1()
                .min_w(px(0.)).flex_1()
                .child(
                    div().id(SharedString::from(toggle_selector.clone())).debug_selector(move || toggle_selector.clone())
                        .size(px(16.)).flex_shrink_0().flex().items_center().justify_center()
                        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                            cx.stop_propagation();
                            if let Some(handler) = &icon_handler { handler(&icon_key, event, false, window, cx); }
                            else { on_toggle(root_idx, &p, window, cx); }
                        })
                        .child(svg().path(folder_icon).size(px(14.)).flex_shrink_0().text_color(theme.text_muted)),
                )
                .child(
                    inline_rename(&key, params).unwrap_or_else(|| div()
                        .id(SharedString::from(name_selector.clone())).debug_selector(move || name_selector.clone())
                        .min_w(px(0.)).truncate()
                        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                            cx.stop_propagation();
                            if let Some(handler) = &name_handler { handler(&DocKey::Path(select_path.clone()), event, true, window, cx); }
                                                else if let Some(select) = &select_directory { select(&select_path, event.click_count == 2, window, cx); }
                        }).child(folder_name).into_any_element()),
                );

            let on_new_file_dir = on_new_file_in_dir.clone();
            let dir_path = node.path.clone();
            let right_part = div()
                .child(path_actions(node.path.clone(), "drawer_dir_row", params, strings))
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

            let row_handler = params.on_row.clone();
            let row_key = key.clone();
            out.push(
                path_context_menu(div().relative()
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        if let Some(handler) = &row_handler { handler(&row_key, event, false, window, cx); }
                    })
                    .id(SharedString::from(format!("drawer_tree_dir_{}_{}_{}", root_idx, depth, node_idx)))
                    .group("drawer_dir_row")
                    .ml(px(depth_indent))
                    .h(px(TREE_DIR_ROW_H))
                    .pl_2()
                    .pr_2()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(13.))
                    .font_weight(FontWeight::NORMAL)
                    .text_color(theme.text_primary)


                    .child(left_part)
                    .child(right_part), node.path.clone(), params, theme, strings)
                    .into_any_element(),
            );

            let mut closing_params = params.clone();
            if !is_expanded {
                if let Some(file) = retained_file(&node.children, &node.path, params) {
                    render_file_tree_nodes(std::slice::from_ref(&file), depth + 1, root_idx, open_docs, active_key,
                        theme, strings, search_query, params, on_toggle_folder, on_new_file_in_dir,
                        on_select_file, on_rename_file, on_close_doc, out);
                    closing_params.hidden_paths.push(file.path);
                }
            }
            if is_expanded || value > 0.0 {
                let mut children = Vec::new();
                render_file_tree_nodes(
                    &node.children,
                    depth + 1,
                    root_idx,
                    open_docs,
                    active_key,
                    theme,
                    strings,
                    search_query,
                    &closing_params,
                    on_toggle_folder,
                    on_new_file_in_dir,
                    on_select_file,
                    on_rename_file,
                    on_close_doc,
                    &mut children,
                );
                out.push(fading_children(div().flex().flex_col().gap_0p5().children(children), is_expanded, value, node.path.clone()));
            }
        } else {
            if !search_query.is_empty() && !node.name.to_lowercase().contains(&q_lower) {
                continue;
            }
            let p = node.path.clone();
            let target_key = DocKey::Path(p.clone());
            let is_active = active_key == Some(&target_key);

            let file_name = node.name.clone();
            let tooltip_name = file_name.clone();
            let on_select = on_select_file.clone();
            let on_rename = on_rename_file.clone();
            let on_close = on_close_doc.clone();
            let rename_path = p.clone();
            let selector = format!("drawer_file:{}", p.display());
            let close_key = target_key.clone();

            let row_handler = params.on_row.clone();
            let row_key = target_key.clone();
            let mut item = div().relative()
                .child(crate::views::sidebar_selection::row_anchor(params.selection.clone(), target_key.clone()))
                .id(SharedString::from(format!("drawer_tree_file:{}", p.display())))
                .debug_selector(move || selector.clone())
                .group("drawer_file_row")
                .ml(px(depth_indent))
                .h(px(TREE_FILE_ROW_H))
                .pl_2()
                .pr_1p5()
                .py(px(6.))
                .rounded(px(5.))
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
                .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    if let Some(handler) = &row_handler {
                        handler(&row_key, event, true, window, cx);
                    } else if event.click_count == 2 {
                        cx.stop_propagation();
                        on_rename(&rename_path, window, cx);
                    } else {
                        on_select(&p, window, cx);
                    }
                });

            if is_active {
                item = item

                    .text_color(theme.selection_text())
                    .font_weight(FontWeight::NORMAL);
            } else {
                item = item
                    .text_color(theme.text_primary)
                    ;
            }

            let mut left_part = div()
                .flex()
                .flex_col()
                .justify_center()
                .min_w(px(0.)).w(px(0.)).flex_1()
                .child(inline_rename(&target_key, params).unwrap_or_else(|| div()
                    .w_full().min_w(px(0.)).h(px(20.)).line_height(px(20.)).truncate().text_size(px(13.)).child(file_name).into_any_element()));

            if let Some(ref mtime_str) = node.mtime {
                left_part = left_part.child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .text_size(px(11.)).line_height(px(16.))
                        .text_color(theme.text_muted)
                        .child(mtime_str.clone()),
                );
            }

            let mut right_part = div().flex().items_center().gap_1();

            {
                right_part = right_part.child(
                    div()
                        .id(SharedString::from(format!("btn_close_file:{}", node.path.display())))
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
                        .tooltip(move |_, cx| cx.new(|_| DrawerTooltipView { text: strings.trash_file.to_string() }).into())
                        .hover(|s| s.bg(rgba(0xffffff20)).text_color(theme.text_primary))
                        .on_mouse_down(MouseButton::Left, move |_e, window, cx| {
                            cx.stop_propagation();
                            on_close(&close_key, window, cx);
                        })
                        .child("×"),
                );
            }

            if params.on_trash_directory.is_some() {
                right_part = path_actions(node.path.clone(), "drawer_file_row", params, strings);
            }
            let file_content = div().flex().flex_1().items_start().gap_1().min_w(px(0.)).overflow_hidden()
                .child(div().size(px(16.)).flex_shrink_0().mt(px(2.)).flex().items_center().justify_center()
                    .child(svg().path("icons/file.svg").size(px(14.)).flex_shrink_0().text_color(theme.text_muted)))
                .child(left_part);
            out.push(path_context_menu(item.child(file_content).child(right_part), node.path.clone(), params, theme, strings).into_any_element());
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
            .font_family(crate::platform::UI_FONT)
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
        base.bg(theme.control_selection_bg())
            .rounded(px(6.))
            .child(icon_el)
    } else {
        base.rounded(px(6.))

            .child(icon_el)
    }
}

fn path_actions(path: PathBuf, group: &'static str, params: &DrawerSearchParams, strings: &'static I18nStrings) -> Div {
    let mut actions = div().flex().items_center().gap_1().flex_shrink_0();
    for (suffix, label, title, callback) in [
        ("remove", "−", strings.remove_from_sidebar, params.on_remove_path.clone()),
    ] {
        if let Some(callback) = callback {
            let target = path.clone();
            actions = actions.child(div().id(SharedString::from(format!("sidebar_{suffix}:{}", path.display())))
                .size(px(16.)).flex().items_center().justify_center().rounded(px(3.)).cursor_pointer().text_size(px(12.))
                .opacity(0.0).group_hover(group, |s| s.opacity(1.0))
                .tooltip(move |_,cx| cx.new(|_| DrawerTooltipView { text: title.to_string() }).into())
                .on_mouse_down(MouseButton::Left,move |_,window,cx| {cx.stop_propagation();callback(&target,window,cx);}).child(label));
        }
    }
    actions
}

fn path_context_menu<E: InteractiveElement + ParentElement + Styled + IntoElement + 'static>(
    element: E, path: PathBuf, params: &DrawerSearchParams, theme: &ThemeColors, strings: &'static I18nStrings,
) -> impl IntoElement {
    let remove = params.on_remove_path.clone();
    let trash = params.on_trash_directory.clone();
    let danger = theme.destructive_text();
    element.context_menu(move |mut menu, _, _| {
        if let Some(callback) = &remove {
            let callback = callback.clone(); let path = path.clone();
            menu = menu.item(PopupMenuItem::new(strings.remove_from_sidebar)
                .on_click(move |_,window,cx| callback(&path,window,cx)));
        }
        if let Some(callback) = &trash {
            let callback = callback.clone(); let path = path.clone();
            menu = menu.item(PopupMenuItem::element(move |_,_| div().text_color(danger).child(strings.trash_file))
                .on_click(move |_,window,cx| callback(&path,window,cx)));
        }
        menu
    })
}

#[cfg(test)]
mod collapsed_selection_tests {
    use super::{retained_file, DocKey, DrawerSearchParams, FileTreeNode, PathBuf};

    #[test]
    fn collapsed_projection_preserves_identity_and_respects_selection_and_hidden_paths() {
        let file = FileTreeNode {
            path: PathBuf::from("root/nested/note.md"), name: "note.md".into(),
            is_dir: false, is_expanded: false, children: vec![], mtime: Some("today".into()),
        };
        let tree = vec![FileTreeNode {
            path: PathBuf::from("root/nested"), name: "nested".into(),
            is_dir: true, is_expanded: false, children: vec![file.clone()], mtime: None,
        }];
        let mut params = DrawerSearchParams::default();
        params.selection.lock().unwrap().click(DocKey::Path(file.path.clone()), false, false);
        let projected = retained_file(&tree, std::path::Path::new("root"), &params).unwrap();
        assert_eq!(projected.name, PathBuf::from("nested").join("note.md").to_string_lossy());
        assert_eq!(projected.path, file.path);
        assert_eq!(projected.mtime, file.mtime);
        assert_eq!(tree[0].children[0], file);
        params.hidden_paths.push(PathBuf::from("root/nested"));
        assert!(retained_file(&tree, std::path::Path::new("root"), &params).is_none());
        params.hidden_paths.clear();
        params.selection.lock().unwrap().click(DocKey::Path("other.md".into()), false, true);
        assert!(retained_file(&tree, std::path::Path::new("root"), &params).is_none());
    }
}
