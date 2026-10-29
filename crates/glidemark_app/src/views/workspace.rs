use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use glidemark_core::document::DocumentStatus;

use crate::actions::{
    ExportHtml, OpenFileDialog, SaveFile, ToggleDrawer, TogglePreviewMode, ToggleSettings,
    ToggleSourceMode, ToggleSplitMode,
};
use crate::state::{parse_debounce, AppState, ViewMode};
use crate::views::drawer::render_drawer;
use crate::views::editor::render_editor_pane;
use crate::views::preview::render_preview_pane;
use crate::views::settings::SettingsWindowView;

pub struct WorkspaceView {
    pub state: AppState,
    pub focus_handle: FocusHandle,
    pub editor_scroll_handle: UniformListScrollHandle,
    pub preview_list: ListState,
    pub drawer_scroll_handle: ScrollHandle,
    pub split_ratio: f32,
    pub is_resizing_split: bool,
    pub is_renaming: bool,
    pub rename_buffer: String,
    pub workspace_width: f32,
    pub auto_save_task: Option<Task<()>>,
    pub parse_task: Option<Task<()>>,
    pub parse_paused: bool,
    pub drawer_closing: bool,
    pub settings_window: Option<WindowHandle<SettingsWindowView>>,
}

impl WorkspaceView {
    pub fn new(state: AppState, cx: &mut Context<Self>) -> Self {
        let preview_list = ListState::new(
            state.doc.parsed.blocks.len(),
            ListAlignment::Top,
            px(600.),
        );
        Self {
            state,
            focus_handle: cx.focus_handle(),
            editor_scroll_handle: UniformListScrollHandle::new(),
            preview_list,
            drawer_scroll_handle: ScrollHandle::new(),
            split_ratio: 0.5,
            is_resizing_split: false,
            is_renaming: false,
            rename_buffer: String::new(),
            workspace_width: 1200.0,
            auto_save_task: None,
            parse_task: None,
            parse_paused: false,
            drawer_closing: false,
            settings_window: None,
        }
    }

    /// Swaps in a new document: the preview list drops cached heights and starts at the top.
    pub fn reset_preview_list(&self) {
        self.preview_list.reset(self.state.doc.parsed.blocks.len());
    }

    /// Re-measures the preview after an edit, keeping the current scroll position.
    pub fn invalidate_preview_list(&self) {
        let count = self.state.doc.parsed.blocks.len();
        let top = self.preview_list.logical_scroll_top();
        if count != self.preview_list.item_count() {
            self.preview_list.reset(count);
            self.preview_list.scroll_to(top);
        } else {
            self.preview_list.splice(0..count, count);
            self.preview_list.scroll_to(top);
        }
    }

    /// Debounces a reparse based on document size; very large documents wait for an explicit refresh.
    pub fn schedule_parse(&mut self, cx: &mut Context<Self>) {
        let debounce = parse_debounce(self.state.doc.buffer.len());
        self.parse_task = None;
        self.parse_paused = debounce.is_none();
        if let Some(delay) = debounce {
            self.parse_task = Some(cx.spawn(async move |this, app| {
                app.background_executor().timer(delay).await;
                let _ = this.update(app, |this, cx| {
                    this.state.parse_now();
                    this.invalidate_preview_list();
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }

    /// Parses immediately (manual save, explicit refresh, or switching into the preview).
    pub fn parse_now_and_refresh(&mut self) {
        self.parse_task = None;
        self.parse_paused = false;
        self.state.sync_from_editor();
        if self.state.parse_now() {
            self.invalidate_preview_list();
        }
    }

    /// A new document starts from a preview scrolled to the top, parsing only if needed.
    pub fn on_document_swapped(&mut self) {
        self.parse_task = None;
        self.parse_paused = false;
        if self.state.view_mode != ViewMode::Source && self.state.doc.is_parse_stale() {
            self.parse_now_and_refresh();
        }
        self.reset_preview_list();
    }

    fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        if mode != ViewMode::Source && self.state.parse_is_stale() {
            self.parse_now_and_refresh();
        }
        self.state.set_view_mode(mode);
        cx.notify();
    }

    pub fn toggle_drawer(&mut self, cx: &mut Context<Self>) {
        if self.drawer_closing {
            self.drawer_closing = false;
            self.state.drawer_open = true;
            cx.notify();
            return;
        }

        if self.state.drawer_open {
            self.drawer_closing = true;
            cx.notify();
            cx.spawn(async move |this, app| {
                app.background_executor()
                    .timer(std::time::Duration::from_millis(180))
                    .await;
                let _ = this.update(app, |this, cx| {
                    if this.drawer_closing {
                        this.state.drawer_open = false;
                        this.drawer_closing = false;
                        cx.notify();
                    }
                });
            })
            .detach();
        } else {
            self.state.drawer_open = true;
            self.drawer_closing = false;
            cx.notify();
        }
    }

    pub fn open_settings_window(&mut self, cx: &mut Context<Self>) {
        if let Some(existing) = &self.settings_window {
            if existing
                .update(cx, |_, window, _| {
                    window.activate_window();
                })
                .is_ok()
            {
                return;
            }
        }
        let workspace = cx.entity().clone();
        let ws = cx.entity().downgrade();
        let preferences = self.state.preferences.clone();
        let theme = self.state.theme.clone();
        let strings = self.state.strings;
        let current_tab = self.state.settings_tab;

        cx.defer(move |cx| {
            let bounds = Bounds::centered(None, size(px(640.), px(460.)), cx);
            let handle = cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("GlideMark - 偏好设置".into()),
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(14.), px(14.))),
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |_window, cx| {
                    cx.new(|cx| SettingsWindowView::new(ws, current_tab, preferences, theme, strings, cx))
                },
            );
            if let Ok(h) = handle {
                let _ = workspace.update(cx, |this, _cx| {
                    this.settings_window = Some(h);
                });
            }
        });
    }

    pub fn trigger_auto_save(&mut self, cx: &mut Context<Self>) {
        if !self.state.preferences.auto_save {
            return;
        }
        let delay_ms = self.state.preferences.auto_save_delay_ms;
        self.auto_save_task = Some(cx.spawn(async move |this, app| {
            app.background_executor().timer(std::time::Duration::from_millis(delay_ms)).await;
            let _ = this.update(app, |this, cx| {
                if this.state.is_dirty() {
                    let _ = this.state.save();
                    cx.notify();
                }
            });
        }));
    }

    fn render_tab_strip(&self, theme: &crate::theme::ThemeColors, cx: &mut Context<Self>) -> impl IntoElement {
        let active_idx = self.state.active_doc_idx;

        div()
            .id("tab_strip_scroll")
            .h(px(34.))
            .w_full()
            .bg(theme.bg)
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .px_2()
            .gap_1()
            .overflow_x_scroll()
            .children(self.state.open_docs.iter().enumerate().map(|(idx, doc)| {
                let is_active = idx == active_idx;
                let is_dirty = doc.status == DocumentStatus::Modified;
                let title = doc.display_name();

                let tab_item = div()
                    .id(("tab_item", idx))
                    .h(px(26.))
                    .max_w(px(180.))
                    .min_w(px(90.))
                    .px_2p5()
                    .rounded_md()
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_1p5()
                    .text_xs();

                let switch_handler = cx.listener(move |this, _, _window, cx| {
                    this.state.switch_tab(idx);
                    this.on_document_swapped();
                    cx.notify();
                });

                let close_handler = cx.listener(move |this, _, _window, cx| {
                    cx.stop_propagation();
                    this.state.close_tab(idx);
                    this.on_document_swapped();
                    cx.notify();
                });

                let title_el = div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .child(title);

                let dirty_indicator = if is_dirty {
                    Some(
                        div()
                            .w(px(6.))
                            .h(px(6.))
                            .rounded_full()
                            .bg(theme.accent)
                            .flex_shrink_0(),
                    )
                } else {
                    None
                };

                let close_btn = div()
                    .id(("tab_close_btn", idx))
                    .w(px(18.))
                    .h(px(18.))
                    .rounded_sm()
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme.text_muted)
                    .hover(|s| s.bg(theme.border).text_color(theme.text_primary))
                    .tooltip(|_window, cx| {
                        cx.new(|_| TooltipView { text: "关闭标签页 (⌘W)".to_string() }).into()
                    })
                    .on_mouse_down(MouseButton::Left, close_handler)
                    .child(
                        svg()
                            .path("icons/x.svg")
                            .size(px(11.))
                    );

                let mut tab = tab_item.child(title_el);
                if let Some(dot) = dirty_indicator {
                    tab = tab.child(dot);
                }
                tab = tab.child(close_btn);

                if is_active {
                    tab
                        .bg(theme.surface)
                        .border_1()
                        .border_color(theme.border)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.accent)
                        .on_mouse_down(MouseButton::Left, switch_handler)
                } else {
                    tab
                        .text_color(theme.text_muted)
                        .hover(|s| s.bg(theme.surface).text_color(theme.text_primary))
                        .on_mouse_down(MouseButton::Left, switch_handler)
                }
            }))
            // "+" Button to open new scratch tab
            .child(
                div()
                    .id("tab_new_btn")
                    .w(px(24.))
                    .h(px(24.))
                    .rounded_md()
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme.text_muted)
                    .hover(|s| s.bg(theme.surface).text_color(theme.text_primary))
                    .tooltip(|_window, cx| {
                        cx.new(|_| TooltipView { text: "新建标签页 (⌘T)".to_string() }).into()
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                        this.state.new_tab();
                        this.on_document_swapped();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_size(px(16.))
                            .line_height(relative(1.0))
                            .child("+")
                    )
            )
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_sys_dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        self.state.update_appearance(is_sys_dark);
        self.workspace_width = f32::from(window.viewport_size().width);

        let theme = &self.state.theme;
        let strings = self.state.strings;
        let view_mode = self.state.view_mode;
        let is_dirty = self.state.is_dirty();
        let has_conflict = self.state.has_conflict();
        let parse_paused = self.parse_paused;

        let file_title = self
            .state
            .doc
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or(strings.untitled)
            .to_string();

        #[cfg(target_os = "macos")]
        let topbar_pl = px(80.);
        #[cfg(not(target_os = "macos"))]
        let topbar_pl = px(12.);

        #[cfg(target_os = "windows")]
        let topbar_pr = px(140.);
        #[cfg(not(target_os = "windows"))]
        let topbar_pr = px(12.);

        let view_click = cx.entity().clone();
        let on_click_line = Arc::new(move |line: usize, col: usize, _window: &mut Window, cx: &mut App| {
            view_click.update(cx, |this, cx| {
                this.state.editor.set_cursor(line, col);
                cx.notify();
            });
        });

        let view_drawer = cx.entity().clone();
        let on_select_file = Arc::new(move |path: &std::path::PathBuf, _window: &mut Window, cx: &mut App| {
            view_drawer.update(cx, |this, cx| {
                let _ = this.state.load_file(path.clone());
                this.on_document_swapped();
                cx.notify();
            });
        });

        let view_tab = cx.entity().clone();
        let on_select_tab = Arc::new(move |tab: crate::state::DrawerTab, _window: &mut Window, cx: &mut App| {
            view_tab.update(cx, |this, cx| {
                this.state.set_drawer_tab(tab);
                cx.notify();
            });
        });

        let view_heading = cx.entity().clone();
        let on_select_heading = Arc::new(move |block_idx: usize, _window: &mut Window, cx: &mut App| {
            view_heading.update(cx, |this, cx| {
                this.preview_list.scroll_to(ListOffset {
                    item_ix: block_idx,
                    offset_in_item: px(0.),
                });
                cx.notify();
            });
        });

        let view_folder = cx.entity().clone();
        let on_open_folder = Arc::new(move |_window: &mut Window, cx: &mut App| {
            let rx = cx.prompt_for_paths(PathPromptOptions {
                files: false,
                directories: true,
                multiple: false,
                prompt: None,
            });
            view_folder.update(cx, |_this, cx| {
                cx.spawn(async move |this, app| {
                    if let Ok(Ok(Some(mut paths))) = rx.await {
                        if let Some(path) = paths.pop() {
                            let _ = this.update(app, |this, cx| {
                                this.state.set_workspace_folder(path);
                                cx.notify();
                            });
                        }
                    }
                })
                .detach();
            });
        });

        let show_drawer = self.state.drawer_open || self.drawer_closing;
        let is_drawer_closing = self.drawer_closing;

        div()
            .key_context("Workspace")
            .track_focus(&self.focus_handle)
            .can_drop(|item, _window, _cx| {
                if let Some(paths) = item.downcast_ref::<ExternalPaths>() {
                    paths.paths().iter().any(|p| {
                        p.extension().map_or(false, |ext| ext == "md" || ext == "markdown" || ext == "txt")
                    })
                } else {
                    false
                }
            })
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                if let Some(path) = paths.paths().iter().find(|p| {
                    p.extension().map_or(false, |ext| ext == "md" || ext == "markdown" || ext == "txt")
                }) {
                    let _ = this.state.load_file(path.clone());
                    this.on_document_swapped();
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                if this.is_resizing_split {
                    let w = this.workspace_width;
                    let x = f32::from(event.position.x);
                    let left_offset = if this.state.drawer_open { 250.0 } else { 0.0 };
                    let avail_w = (w - left_offset).max(100.0);
                    let offset_x = (x - left_offset).max(0.0);
                    this.split_ratio = (offset_x / avail_w).clamp(0.20, 0.80);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _window, _cx| {
                if this.is_resizing_split {
                    this.is_resizing_split = false;
                }
            }))
            .on_action(cx.listener(|this, _: &ToggleSourceMode, _window, cx| {
                this.set_view_mode(ViewMode::Source, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSplitMode, _window, cx| {
                this.set_view_mode(ViewMode::Split, cx);
            }))
            .on_action(cx.listener(|this, _: &TogglePreviewMode, _window, cx| {
                this.set_view_mode(ViewMode::Preview, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleDrawer, _window, cx| {
                this.toggle_drawer(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSettings, _window, cx| {
                this.open_settings_window(cx);
            }))
            .on_action(cx.listener(|_this, _: &OpenFileDialog, _window, cx| {
                let rx = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: None,
                });
                cx.spawn(async move |this, app| {
                    if let Ok(Ok(Some(mut paths))) = rx.await {
                        if let Some(path) = paths.pop() {
                            let _ = this.update(app, |this, cx| {
                                let _ = this.state.load_file(path);
                                this.on_document_swapped();
                                cx.notify();
                            });
                        }
                    }
                })
                .detach();
            }))
            .on_action(cx.listener(|this, _: &SaveFile, _window, cx| {
                this.parse_now_and_refresh();
                let _ = this.state.save();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ExportHtml, _window, _cx| {
                let html = this.state.export_standalone_html();
                if let Some(path) = &this.state.doc.path {
                    let out_path = path.with_extension("html");
                    let _ = std::fs::write(&out_path, html);
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if this.is_renaming {
                    match event.keystroke.key.as_str() {
                        "enter" => {
                            let new_name = this.rename_buffer.trim();
                            if !new_name.is_empty() {
                                let _ = this.state.rename_file(new_name);
                                this.state.refresh_sibling_files();
                            }
                            this.is_renaming = false;
                            cx.notify();
                        }
                        "escape" => {
                            this.is_renaming = false;
                            cx.notify();
                        }
                        "backspace" => {
                            this.rename_buffer.pop();
                            cx.notify();
                        }
                        "space" => {
                            this.rename_buffer.push(' ');
                            cx.notify();
                        }
                        "period" => {
                            this.rename_buffer.push('.');
                            cx.notify();
                        }
                        key => {
                            if let Some(ch) = &event.keystroke.key_char {
                                this.rename_buffer.push_str(ch);
                                cx.notify();
                            } else if key.chars().count() == 1
                                && !event.keystroke.modifiers.control
                                && !event.keystroke.modifiers.platform
                            {
                                this.rename_buffer.push_str(key);
                                cx.notify();
                            }
                        }
                    }
                    return;
                }

                let key = event.keystroke.key.as_str();
                let mods = event.keystroke.modifiers;

                if mods.platform {
                    match key {
                        "z" => {
                            if mods.shift {
                                this.state.editor.redo();
                            } else {
                                this.state.editor.undo();
                            }
                            if this.state.sync_from_editor() {
                                this.schedule_parse(cx);
                            }
                            this.trigger_auto_save(cx);
                            cx.notify();
                        }
                        "s" => {
                            this.parse_now_and_refresh();
                            let _ = this.state.save();
                            cx.notify();
                        }
                        "w" => {
                            this.state.close_tab(this.state.active_doc_idx);
                            this.on_document_swapped();
                            cx.notify();
                        }
                        "t" => {
                            this.state.new_tab();
                            this.on_document_swapped();
                            cx.notify();
                        }
                        "o" => {
                            let rx = cx.prompt_for_paths(PathPromptOptions {
                                files: true,
                                directories: false,
                                multiple: false,
                                prompt: None,
                            });
                            cx.spawn(async move |this, app| {
                                if let Ok(Ok(Some(mut paths))) = rx.await {
                                    if let Some(path) = paths.pop() {
                                        let _ = this.update(app, |this, cx| {
                                            let _ = this.state.load_file(path);
                                            this.on_document_swapped();
                                            cx.notify();
                                        });
                                    }
                                }
                            })
                            .detach();
                        }
                        "," => {
                            this.open_settings_window(cx);
                        }
                        "b" => {
                            this.toggle_drawer(cx);
                        }
                        "1" => {
                            this.set_view_mode(ViewMode::Source, cx);
                        }
                        "2" => {
                            this.set_view_mode(ViewMode::Split, cx);
                        }
                        "3" => {
                            this.set_view_mode(ViewMode::Preview, cx);
                        }
                        _ => {}
                    }
                    return;
                }

                let mut handled = true;
                match key {
                    "backspace" => this.state.editor.delete_backward(),
                    "delete" => this.state.editor.delete_forward(),
                    "enter" => this.state.editor.insert_newline(),
                    "tab" => {
                        let spaces = " ".repeat(this.state.preferences.tab_size);
                        this.state.editor.insert_text(&spaces);
                    }
                    "left" => this.state.editor.move_left(),
                    "right" => this.state.editor.move_right(),
                    "up" => this.state.editor.move_up(),
                    "down" => this.state.editor.move_down(),
                    "home" => this.state.editor.move_to_line_start(),
                    "end" => this.state.editor.move_to_line_end(),
                    _ => {
                        if let Some(key_char) = &event.keystroke.key_char {
                            if !mods.control {
                                this.state.editor.insert_text(key_char);
                            } else {
                                handled = false;
                            }
                        } else if key.chars().count() == 1 && !mods.control {
                            this.state.editor.insert_text(key);
                        } else {
                            handled = false;
                        }
                    }
                }

                if handled {
                    if this.state.sync_from_editor() {
                        this.schedule_parse(cx);
                    }
                    this.trigger_auto_save(cx);
                    cx.notify();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.bg)
            .text_color(theme.text_primary)
            // Top Toolbar (Pure icon-driven, safe native insets)
            .child(
                div()
                    .h(px(42.))
                    .pl(topbar_pl)
                    .pr(topbar_pr)
                    .bg(theme.bg)
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    // Left area: Drawer icon + Open icon + Title (Double-click to rename)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            // Sidebar Drawer icon button
                            .child(render_icon_button(
                                "btn_drawer",
                                format!("{} (⌘B)", strings.directory_drawer),
                                self.state.drawer_open,
                                theme,
                                render_sidebar_icon(if self.state.drawer_open { theme.accent } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.toggle_drawer(cx);
                                }),
                            ))
                            // Open file icon button
                            .child(render_icon_button(
                                "btn_open",
                                format!("{} (⌘O)", strings.open_file),
                                false,
                                theme,
                                render_folder_icon(theme.text_muted),
                                cx.listener(|_this, _, _window, cx| {
                                    let rx = cx.prompt_for_paths(PathPromptOptions {
                                        files: true,
                                        directories: false,
                                        multiple: false,
                                        prompt: None,
                                    });
                                    cx.spawn(async move |this, app| {
                                        if let Ok(Ok(Some(mut paths))) = rx.await {
                                            if let Some(path) = paths.pop() {
                                                let _ = this.update(app, |this, cx| {
                                                    let _ = this.state.load_file(path);
                                                    this.on_document_swapped();
                                                    cx.notify();
                                                });
                                            }
                                        }
                                    })
                                    .detach();
                                }),
                            ))
                            // Document Title & Double-click Rename
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .ml_2()
                                    .child(if self.is_renaming {
                                        div()
                                            .id("title_rename_box")
                                            .px_2()
                                            .py_0p5()
                                            .bg(theme.bg)
                                            .border_1()
                                            .border_color(theme.accent)
                                            .rounded_sm()
                                            .shadow_sm()
                                            .flex()
                                            .items_center()
                                            .cursor_text()
                                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, _cx| {
                                                window.focus(&this.focus_handle);
                                            }))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(theme.text_primary)
                                                    .child(if self.rename_buffer.is_empty() {
                                                        "Untitled.md".to_string()
                                                    } else {
                                                        self.rename_buffer.clone()
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .w(px(1.5))
                                                    .h(px(12.))
                                                    .ml_0p5()
                                                    .bg(theme.accent)
                                                    .with_animation(
                                                        "rename_cursor_blink",
                                                        Animation::new(std::time::Duration::from_millis(600)).repeat(),
                                                        |el, delta| el.opacity(if delta < 0.5 { 1.0 } else { 0.0 }),
                                                    ),
                                            )
                                            .into_any_element()
                                    } else {
                                        div()
                                            .cursor_pointer()
                                            .hover(|s| s.text_color(theme.accent))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                                    if event.click_count == 2 {
                                                        let name = this
                                                            .state
                                                            .doc
                                                            .path
                                                            .as_ref()
                                                            .and_then(|p| p.file_name())
                                                            .and_then(|n| n.to_str())
                                                            .unwrap_or("Untitled.md");
                                                        this.rename_buffer = name.to_string();
                                                        this.is_renaming = true;
                                                        window.focus(&this.focus_handle);
                                                        cx.notify();
                                                    }
                                                }),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(file_title),
                                            )
                                            .into_any_element()
                                    })
                                    .when(is_dirty, |el| {
                                        el.child(
                                            div()
                                                .w(px(7.))
                                                .h(px(7.))
                                                .rounded_full()
                                                .bg(theme.accent),
                                        )
                                    }),
                            ),
                    )
                    // Center area: View Mode Switcher Icons (Mac-style Segmented Control)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .bg(theme.bg)
                            .p(px(2.))
                            .rounded(px(7.))
                            .gap(px(2.))
                            .border_1()
                            .border_color(theme.border)
                            .child(render_mode_icon_tab(
                                "btn_mode_source",
                                format!("{} (⌘1)", strings.source_mode),
                                view_mode == ViewMode::Source,
                                theme,
                                render_source_mode_icon(if view_mode == ViewMode::Source { theme.accent } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Source, cx);
                                }),
                            ))
                            .child(render_mode_icon_tab(
                                "btn_mode_split",
                                format!("{} (⌘2)", strings.split_mode),
                                view_mode == ViewMode::Split,
                                theme,
                                render_split_mode_icon(if view_mode == ViewMode::Split { theme.accent } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Split, cx);
                                }),
                            ))
                            .child(render_mode_icon_tab(
                                "btn_mode_preview",
                                format!("{} (⌘3)", strings.preview_mode),
                                view_mode == ViewMode::Preview,
                                theme,
                                render_preview_mode_icon(if view_mode == ViewMode::Preview { theme.accent } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Preview, cx);
                                }),
                            )),
                    )
                    // Right area: Action Icon Buttons
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            // Export HTML icon button
                            .child(render_icon_button(
                                "btn_export",
                                format!("{} (⇧⌘E)", strings.export_html),
                                false,
                                theme,
                                render_export_icon(theme.text_muted),
                                cx.listener(|this, _, _window, _cx| {
                                    let html = this.state.export_standalone_html();
                                    if let Some(path) = &this.state.doc.path {
                                        let out_path = path.with_extension("html");
                                        let _ = std::fs::write(&out_path, html);
                                    }
                                }),
                            ))
                            // Save file icon button
                            .child(render_icon_button(
                                "btn_save",
                                format!("{} (⌘S)", strings.save_file),
                                false,
                                theme,
                                render_save_icon(if is_dirty { theme.accent } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.parse_now_and_refresh();
                                    let _ = this.state.save();
                                    cx.notify();
                                }),
                            ))
                            // Settings icon button
                            .child(render_icon_button(
                                "btn_settings",
                                format!("{} (⌘,)", strings.settings),
                                self.settings_window.is_some(),
                                theme,
                                render_settings_icon(theme.text_muted),
                                cx.listener(|this, _, _window, cx| {
                                    this.open_settings_window(cx);
                                }),
                            )),
                    ),
            )
            // Conflict Warning Banner
            .when(has_conflict, |el| {
                el.child(
                    div()
                        .h(px(36.))
                        .px_4()
                        .bg(theme.conflict_bg)
                        .border_b_1()
                        .border_color(theme.conflict_border)
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(0xfef08a))
                                .child(strings.conflict_warning),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(theme.surface)
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _window, cx| {
                                                this.state.doc.resolve_conflict_keep_local();
                                                cx.notify();
                                            }),
                                        )
                                        .child(strings.keep_local),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(theme.surface)
                                        .text_xs()
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _window, cx| {
                                                if let Some(path) = &this.state.doc.path.clone() {
                                                    let _ = this.state.load_file(path.clone());
                                                    this.on_document_swapped();
                                                }
                                                cx.notify();
                                            }),
                                        )
                                        .child(strings.reload_disk),
                                ),
                        ),
                )
            })
            // Large Document Parse Pause Banner
            .when(parse_paused, |el| {
                el.child(
                    div()
                        .h(px(36.))
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
                                .text_color(theme.text_muted)
                                .child(strings.parse_paused),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_0p5()
                                .rounded_sm()
                                .bg(theme.bg)
                                .border_1()
                                .border_color(theme.border)
                                .text_xs()
                                .cursor_pointer()
                                .hover(|s| s.text_color(theme.accent))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _window, cx| {
                                        this.parse_now_and_refresh();
                                        cx.notify();
                                    }),
                                )
                                .child(strings.refresh_preview),
                        ),
                )
            })
            // Main workspace content layout: edge-to-edge, continuous surface with clean hairlines
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .flex()
                    .overflow_hidden()
                    // 1. Left Drawer (Files & TOC) with Bidirectional 180ms Animation
                    .when(show_drawer, |body| {
                        body.child(
                            div()
                                .id(if is_drawer_closing { "animated_drawer_closing" } else { "animated_drawer_opening" })
                                .h_full()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .w(px(250.))
                                        .h_full()
                                        .border_r_1()
                                        .border_color(theme.border)
                                        .bg(theme.bg)
                                        .overflow_hidden()
                                        .child(render_drawer(
                                            self.state.drawer_tab,
                                            &self.state.sibling_files,
                                            self.state.doc.path.as_ref(),
                                            self.state.workspace_folder.as_ref(),
                                            &self.state.doc.parsed.headings(),
                                            theme,
                                            strings,
                                            &self.drawer_scroll_handle,
                                            on_select_tab,
                                            on_select_file,
                                            on_select_heading,
                                            on_open_folder,
                                        )),
                                )
                                .with_animation(
                                    if is_drawer_closing { "drawer_slide_out" } else { "drawer_slide_in" },
                                    Animation::new(std::time::Duration::from_millis(180)).with_easing(ease_in_out),
                                    move |el, delta| {
                                        let factor = if is_drawer_closing { 1.0 - delta } else { delta };
                                        el.w(px(250.0 * factor)).opacity(factor.clamp(0.0, 1.0))
                                    },
                                ),
                        )
                    })
                    // 2. Center Content Area (Editor / Preview / Split)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .min_h(px(0.))
                            .h_full()
                            .bg(theme.surface)
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .when(self.state.open_docs.len() > 1, |center| {
                                center.child(self.render_tab_strip(theme, cx))
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .min_h(px(0.))
                                    .size_full()
                                    .overflow_hidden()
                                    .child(match view_mode {
                                        ViewMode::Source => div()
                                            .size_full()
                                            .child(render_editor_pane(
                                                self.state.editor.lines.clone(),
                                                self.state.editor.cursor(),
                                                theme,
                                                self.state.preferences.font_size,
                                                is_dirty,
                                                &self.editor_scroll_handle,
                                                on_click_line,
                                            ))
                                            .into_any_element(),
                                        ViewMode::Preview => div()
                                            .size_full()
                                            .min_w(px(0.))
                                            .child(render_preview_pane(
                                                self.state.doc.parsed.clone(),
                                                theme,
                                                self.state.preferences.max_column_width,
                                                self.state.preferences.font_size,
                                                &self.preview_list,
                                            ))
                                            .into_any_element(),
                                        ViewMode::Split => div()
                                            .size_full()
                                            .flex()
                                            .overflow_hidden()
                                            // Left Editor Pane
                                            .child(
                                                div()
                                                    .w(relative(self.split_ratio))
                                                    .min_w(px(0.))
                                                    .min_h(px(0.))
                                                    .h_full()
                                                    .flex()
                                                    .flex_col()
                                                    .overflow_hidden()
                                                    .child(render_editor_pane(
                                                        self.state.editor.lines.clone(),
                                                        self.state.editor.cursor(),
                                                        theme,
                                                        self.state.preferences.font_size,
                                                        is_dirty,
                                                        &self.editor_scroll_handle,
                                                        on_click_line,
                                                    )),
                                            )
                                            // Draggable Resizable Divider: 5px hit zone with 1px hairline
                                            .child(
                                                div()
                                                    .w(px(5.))
                                                    .h_full()
                                                    .flex()
                                                    .justify_center()
                                                    .items_center()
                                                    .cursor_col_resize()
                                                    .bg(theme.surface)
                                                    .hover(|s| s.bg(theme.accent))
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                                                            if event.click_count == 2 {
                                                                this.split_ratio = 0.5;
                                                                cx.notify();
                                                            } else {
                                                                this.is_resizing_split = true;
                                                            }
                                                        }),
                                                    )
                                                    .child(
                                                        div()
                                                            .w(px(1.))
                                                            .h_full()
                                                            .bg(theme.border),
                                                    ),
                                            )
                                            // Right Preview Pane
                                            .child(
                                                div()
                                                    .w(relative(1.0 - self.split_ratio))
                                                    .min_w(px(0.))
                                                    .min_h(px(0.))
                                                    .h_full()
                                                    .flex()
                                                    .flex_col()
                                                    .overflow_hidden()
                                                    .child(render_preview_pane(
                                                        self.state.doc.parsed.clone(),
                                                        theme,
                                                        self.state.preferences.max_column_width,
                                                        self.state.preferences.font_size,
                                                        &self.preview_list,
                                                    )),
                                            )
                                            .into_any_element(),
                                    }),
                            ),
                    ),
            )
    }
}

// Floating tooltip micro-capsule with dark frosted styling
struct TooltipView {
    text: String,
}

impl Render for TooltipView {
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

// Minimalist vector icon button helper with tooltips and hover capsule
fn render_icon_button<F>(
    id: impl Into<ElementId>,
    tooltip_text: impl Into<String>,
    active: bool,
    theme: &crate::theme::ThemeColors,
    icon: impl IntoElement + 'static,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let text = tooltip_text.into();
    let base = div()
        .id(id)
        .tooltip(move |_window, cx| {
            let t = text.clone();
            cx.new(|_| TooltipView { text: t }).into()
        })
        .w(px(32.))
        .h(px(32.))
        .rounded_md()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler);

    if active {
        base.bg(theme.surface)
            .child(icon)
    } else {
        base.hover(|s| s.bg(theme.surface))
            .child(icon)
    }
}

fn render_mode_icon_tab<F>(
    id: impl Into<ElementId>,
    tooltip_text: impl Into<String>,
    active: bool,
    theme: &crate::theme::ThemeColors,
    icon: impl IntoElement + 'static,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let text = tooltip_text.into();
    let base = div()
        .id(id)
        .tooltip(move |_window, cx| {
            let t = text.clone();
            cx.new(|_| TooltipView { text: t }).into()
        })
        .w(px(36.))
        .h(px(26.))
        .rounded(px(5.))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler);

    if active {
        base.bg(theme.surface)
            .border_1()
            .border_color(theme.border)
            .text_color(theme.accent)
            .child(icon)
    } else {
        base.text_color(theme.text_muted)
            .hover(|s| s.bg(theme.surface).text_color(theme.text_primary))
            .child(icon)
    }
}

// Standard Lucide SVG icons rendered natively by GPUI
fn render_sidebar_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/panel-left.svg").size(px(20.)).text_color(color)
}

fn render_folder_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/folder.svg").size(px(20.)).text_color(color)
}

fn render_source_mode_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/code.svg").size(px(18.)).text_color(color)
}

fn render_split_mode_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/columns-2.svg").size(px(18.)).text_color(color)
}

fn render_preview_mode_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/book-open.svg").size(px(18.)).text_color(color)
}

fn render_export_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/download.svg").size(px(20.)).text_color(color)
}

fn render_save_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/save.svg").size(px(20.)).text_color(color)
}

fn render_settings_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/settings.svg").size(px(20.)).text_color(color)
}
