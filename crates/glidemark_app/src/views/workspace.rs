use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;

use crate::actions::{
    ExportHtml, OpenFileDialog, SaveFile, ToggleDrawer, TogglePreviewMode, ToggleSettings,
    ToggleSourceMode, ToggleSplitMode,
};
use crate::state::{parse_debounce, AppState, CloseDecision, ViewMode};
use crate::views::drawer::render_drawer;
use crate::views::editor::render_editor_pane;
use crate::views::lightbox::{render_lightbox, LightboxState};
use crate::views::preview::render_preview_pane;
use crate::views::settings::SettingsWindowView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollOrigin {
    Editor,
    Preview,
}

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
    pub copied_code_hash: Option<(u64, std::time::Instant)>,
    pub last_scroll_origin: Option<(ScrollOrigin, std::time::Instant)>,
    pub lightbox: LightboxState,
    pub window_handle: Option<AnyWindowHandle>,
    pub error_banner: Option<String>,
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
            copied_code_hash: None,
            last_scroll_origin: None,
            lightbox: LightboxState::new(),
            window_handle: None,
            error_banner: None,
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
                    if this.state.view_mode == ViewMode::Split {
                        this.align_preview_to_cursor();
                    }
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
            if self.state.view_mode == ViewMode::Split {
                self.align_preview_to_cursor();
            }
        }
    }

    /// Aligns preview virtual list position to current editor cursor block.
    pub fn align_preview_to_cursor(&mut self) {
        if self.state.doc.is_parse_stale() || self.state.doc.parsed.anchors.is_empty() {
            return;
        }
        let cursor_line = self.state.editor.cursor().0 + 1;
        let map = glidemark_core::anchor::SyncScrollMap::new(self.state.doc.parsed.anchors.clone());
        let (block_index, _) = map.source_line_to_preview_progress(cursor_line);
        self.preview_list.scroll_to(ListOffset {
            item_ix: block_index,
            offset_in_item: px(0.),
        });
    }

    /// Synchronizes editor wheel scrolling to the preview pane with 100ms echo suppression.
    pub fn handle_editor_scroll_wheel(&mut self, _event: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.state.view_mode != ViewMode::Split || self.state.doc.is_parse_stale() || self.state.doc.parsed.anchors.is_empty() {
            return;
        }
        let now = std::time::Instant::now();
        if let Some((origin, time)) = self.last_scroll_origin {
            if origin == ScrollOrigin::Preview && now.duration_since(time) < std::time::Duration::from_millis(100) {
                return;
            }
        }
        self.last_scroll_origin = Some((ScrollOrigin::Editor, now));

        let scroll_y = (-f32::from(self.editor_scroll_handle.0.borrow().base_handle.offset().y)).max(0.0);
        let line_height = (self.state.preferences.font_size * 1.5).max(1.0);
        let top_line_index = (scroll_y / line_height).floor() as usize;
        let line = top_line_index + 1;

        let map = glidemark_core::anchor::SyncScrollMap::new(self.state.doc.parsed.anchors.clone());
        let (block_index, _) = map.source_line_to_preview_progress(line);
        self.preview_list.scroll_to(ListOffset {
            item_ix: block_index,
            offset_in_item: px(0.),
        });
        cx.notify();
    }

    /// Synchronizes preview wheel scrolling to the editor pane with 100ms echo suppression.
    pub fn handle_preview_scroll_wheel(&mut self, _event: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.state.view_mode != ViewMode::Split || self.state.doc.is_parse_stale() || self.state.doc.parsed.anchors.is_empty() {
            return;
        }
        let now = std::time::Instant::now();
        if let Some((origin, time)) = self.last_scroll_origin {
            if origin == ScrollOrigin::Editor && now.duration_since(time) < std::time::Duration::from_millis(100) {
                return;
            }
        }
        self.last_scroll_origin = Some((ScrollOrigin::Preview, now));

        let top_block = self.preview_list.logical_scroll_top().item_ix;
        let map = glidemark_core::anchor::SyncScrollMap::new(self.state.doc.parsed.anchors.clone());
        let target_line = map.preview_to_source_line(top_block, 0.0);
        self.editor_scroll_handle.scroll_to_item_strict(
            target_line.saturating_sub(1),
            ScrollStrategy::Top,
        );
        cx.notify();
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
                        title: Some(format!("GlideMark - {}", strings.settings).into()),
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

    pub fn trigger_save(&mut self, cx: &mut Context<Self>) {
        if self.state.active.is_none() {
            return;
        }
        self.parse_now_and_refresh();
        if self.state.doc.path.is_some() {
            let _ = self.state.save();
            cx.notify();
        } else {
            let suggested_dir = self
                .state
                .workspace_roots
                .first()
                .map(|r| r.path.clone())
                .unwrap_or_else(|| {
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                });
            let rx = cx.prompt_for_new_path(&suggested_dir, Some("Untitled.md"));
            cx.spawn(async move |this, app| {
                if let Ok(Ok(Some(path))) = rx.await {
                    let _ = this.update(app, |this, cx| {
                        if let Some(key) = this.state.active.clone() {
                            let _ = this.state.save_doc_as(&key, path);
                            this.on_document_swapped();
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
    }

    pub fn trigger_close_doc(&mut self, key: &crate::state::DocKey, cx: &mut Context<Self>) {
        let decision = self.state.request_close(key);
        match decision {
            CloseDecision::CloseImmediately => {
                self.on_document_swapped();
                cx.notify();
            }
            CloseDecision::PromptDirty { .. } => {
                self.on_document_swapped();
                cx.notify();
            }
        }
    }

    pub fn trigger_close_active(&mut self, cx: &mut Context<Self>) {
        if let Some(key) = self.state.active.clone() {
            self.trigger_close_doc(&key, cx);
        }
    }

    pub fn handle_modal_cancel(&mut self, cx: &mut Context<Self>) {
        self.state.cancel_close();
        cx.notify();
    }

    pub fn handle_modal_discard(&mut self, key: &crate::state::DocKey, window: &mut Window, cx: &mut Context<Self>) {
        self.state.confirm_close_discard(key);
        self.on_document_swapped();
        self.after_dirty_doc_resolved(Some(window), cx);
    }

    pub fn handle_modal_save(&mut self, key: &crate::state::DocKey, window: &mut Window, cx: &mut Context<Self>) {
        match self.state.confirm_close_save(key) {
            Ok(true) => {
                self.on_document_swapped();
                self.after_dirty_doc_resolved(Some(window), cx);
            }
            Ok(false) => {
                self.prompt_save_as_and_close(key.clone(), cx);
            }
            Err(e) => {
                eprintln!("Save error during close: {}", e);
                cx.notify();
            }
        }
    }

    pub fn prompt_save_as_and_close(&mut self, close_key: crate::state::DocKey, cx: &mut Context<Self>) {
        let was_quitting = self.state.is_quitting;
        self.state.pending_close = None;
        cx.notify();

        let suggested_dir = self
            .state
            .workspace_roots
            .first()
            .map(|r| r.path.clone())
            .unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            });
        let rx = cx.prompt_for_new_path(&suggested_dir, Some("Untitled.md"));

        cx.spawn(async move |this, app| {
            if let Ok(Ok(Some(path))) = rx.await {
                let _ = this.update(app, |this, cx| {
                    match this.state.save_doc_as(&close_key, path) {
                        Ok(()) => {
                            this.state.close_document_forced(&close_key);
                            this.on_document_swapped();
                            this.state.is_quitting = was_quitting;
                            this.after_dirty_doc_resolved(None, cx);
                        }
                        Err(e) => {
                            eprintln!("Save As failed: {}", e);
                            this.state.is_quitting = was_quitting;
                            let title = this.state.doc.display_name_with_fallback(this.state.strings.untitled_file);
                            this.state.pending_close = Some(crate::state::PendingClose {
                                key: close_key,
                                title,
                                has_path: false,
                            });
                            cx.notify();
                        }
                    }
                });
            } else {
                let _ = this.update(app, |this, cx| {
                    this.state.is_quitting = false;
                    this.state.pending_close = None;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn after_dirty_doc_resolved(&mut self, mut window_opt: Option<&mut Window>, cx: &mut Context<Self>) {
        if self.state.is_quitting {
            if let Some(next_dirty) = self.state.next_dirty_doc_key() {
                self.state.request_close(&next_dirty);
                self.on_document_swapped();
                cx.notify();
            } else {
                // All dirty documents resolved! Close window per ADR 0031.
                self.state.is_quitting = false;
                if let Some(window) = window_opt.as_deref_mut() {
                    window.remove_window();
                } else if let Some(h) = self.window_handle {
                    let _ = h.update(cx, |_view, window, _cx| {
                        window.remove_window();
                    });
                }
            }
        } else {
            cx.notify();
        }
    }

    pub fn on_window_should_close(&mut self, cx: &mut Context<Self>) -> bool {
        let allow = self.state.handle_window_should_close();
        self.on_document_swapped();
        cx.notify();
        allow
    }

    pub fn trigger_draft(&mut self, cx: &mut Context<Self>) {
        self.state.focus_or_create_draft();
        self.on_document_swapped();
        cx.notify();
    }

    fn render_canvas_header(&self, theme: &crate::theme::ThemeColors, cx: &mut Context<Self>) -> impl IntoElement {
        let is_macos = cfg!(target_os = "macos");
        let drawer_open = self.state.drawer_open;
        let view_mode = self.state.view_mode;
        let strings = self.state.strings;
        let is_renaming = self.is_renaming;
        let rename_buffer = self.rename_buffer.clone();

        // Left padding: on macOS when sidebar is collapsed, inject 76px safe area to avoid traffic lights.
        let header_pl = if is_macos && !drawer_open { px(76.) } else { px(8.) };
        // Right padding: on Windows/Linux, reserve 138px for system window controls (min/max/close).
        let header_pr = if !is_macos { px(138.) } else { px(12.) };

        let active_key = self.state.active.clone();
        let (title_text, tooltip_text, is_dirty) = if active_key.is_some() {
            let title = self.state.doc.display_name_with_fallback(strings.untitled_file);
            let tooltip = if let Some(path) = &self.state.doc.path {
                path.to_string_lossy().to_string()
            } else {
                strings.draft_unsaved_hint.to_string()
            };
            let dirty = self.state.is_dirty();
            (title, tooltip, dirty)
        } else {
            (strings.no_document_open.to_string(), strings.no_document_open.to_string(), false)
        };

        div()
            .id("canvas_integrated_header")
            .h(px(38.))
            .w_full()
            .pl(header_pl)
            .pr(header_pr)
            .bg(theme.surface)
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            // Left Group: Expand sidebar button (when collapsed) + Document Title
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .min_w(px(0.))
                    .when(!drawer_open, |el| {
                        el.child(render_icon_button(
                            "btn_expand_drawer",
                            format!("{} (⌘B)", strings.directory_drawer),
                            false,
                            theme,
                            render_sidebar_icon(theme.text_muted),
                            cx.listener(|this, _, _window, cx| {
                                this.toggle_drawer(cx);
                            }),
                        ))
                    })
                    .child(if is_renaming && active_key.is_some() {
                        div()
                            .id("tab_rename_box")
                            .px_2()
                            .py_0p5()
                            .bg(theme.bg)
                            .border_1()
                            .border_color(theme.accent)
                            .rounded_sm()
                            .flex()
                            .items_center()
                            .cursor_text()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.text_primary)
                                    .child(if rename_buffer.is_empty() {
                                        "Untitled.md".to_string()
                                    } else {
                                        rename_buffer.clone()
                                    }),
                            )
                            .child(
                                div()
                                    .w(px(1.5))
                                    .h(px(14.))
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
                        let tooltip = tooltip_text.clone();
                        div()
                            .id("canvas_document_title")
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .tooltip(move |_window, cx| {
                                let t = tooltip.clone();
                                cx.new(|_| TooltipView { text: t }).into()
                            })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                if event.click_count == 2 && this.state.active.is_some() {
                                    if this.state.doc.path.is_some() {
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
                                    } else {
                                        this.trigger_save(cx);
                                    }
                                }
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if active_key.is_some() { theme.text_primary } else { theme.text_muted })
                                    .max_w(px(320.))
                                    .truncate()
                                    .child(title_text)
                            )
                            .when(is_dirty, |el| {
                                el.child(
                                    div()
                                        .w(px(6.))
                                        .h(px(6.))
                                        .rounded_full()
                                        .bg(theme.accent)
                                        .flex_shrink_0()
                                )
                            })
                            .into_any_element()
                    })
            )
            // Draggable Window Control Area in center between title and action tools
            .child(
                div()
                    .id("canvas_header_drag_area")
                    .flex_1()
                    .h_full()
                    .min_w(px(20.))
                    .window_control_area(WindowControlArea::Drag)
                    .on_mouse_down(MouseButton::Left, |_e, window, _| {
                        window.start_window_move();
                    })
            )
            // Right Group: Segmented View Mode Switcher + Action Buttons (Save, Export, Settings)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
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
                                render_source_mode_icon(if view_mode == ViewMode::Source { theme.warm_pill_text() } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Source, cx);
                                }),
                            ))
                            .child(render_mode_icon_tab(
                                "btn_mode_split",
                                format!("{} (⌘2)", strings.split_mode),
                                view_mode == ViewMode::Split,
                                theme,
                                render_split_mode_icon(if view_mode == ViewMode::Split { theme.warm_pill_text() } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Split, cx);
                                }),
                            ))
                            .child(render_mode_icon_tab(
                                "btn_mode_preview",
                                format!("{} (⌘3)", strings.preview_mode),
                                view_mode == ViewMode::Preview,
                                theme,
                                render_preview_mode_icon(if view_mode == ViewMode::Preview { theme.warm_pill_text() } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.set_view_mode(ViewMode::Preview, cx);
                                }),
                            ))
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(render_icon_button(
                                "btn_save",
                                format!("{} (⌘S)", strings.save_file),
                                false,
                                theme,
                                render_save_icon(theme.text_muted),
                                cx.listener(|this, _, _window, cx| {
                                    this.trigger_save(cx);
                                }),
                            ))
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
                            .child(render_icon_button(
                                "btn_settings",
                                format!("{} (⌘,)", strings.settings),
                                self.settings_window.is_some(),
                                theme,
                                render_settings_icon(theme.text_muted),
                                cx.listener(|this, _, _window, cx| {
                                    this.open_settings_window(cx);
                                }),
                            ))
                    )
            )
    }

    fn render_close_modal(&self, theme: &crate::theme::ThemeColors, cx: &mut Context<Self>) -> impl IntoElement {
        let strings = self.state.strings;
        let pending = self.state.pending_close.clone().unwrap_or(crate::state::PendingClose {
            key: crate::state::DocKey::Scratch(0),
            title: String::new(),
            has_path: false,
        });
        let title = pending.title.clone();
        let key_discard = pending.key.clone();
        let key_save = pending.key.clone();
        let prompt_text = strings.modal_save_prompt.replace("{}", &title);

        div()
            .id("dirty_close_modal_backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(0x00000066))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("dirty_close_modal_card")
                    .w(px(380.))
                    .p_5()
                    .rounded_xl()
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child(prompt_text),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.text_muted)
                                    .child(strings.modal_save_detail),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .items_center()
                            .gap_2()
                            // Cancel button
                            .child(
                                div()
                                    .id("btn_modal_cancel")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_xs()
                                    .text_color(theme.text_primary)
                                    .hover(|s| s.bg(theme.bg))
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _window, cx| {
                                        this.handle_modal_cancel(cx);
                                    }))
                                    .child(format!("{} (Esc)", strings.modal_cancel)),
                            )
                            // Discard button
                            .child(
                                div()
                                    .id("btn_modal_discard")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(rgba(0xef44441a))
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xef4444))
                                    .hover(|s| s.bg(rgba(0xef444433)))
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                        this.handle_modal_discard(&key_discard, window, cx);
                                    }))
                                    .child(format!("{} (D)", strings.modal_discard)),
                            )
                            // Save button
                            .child(
                                div()
                                    .id("btn_modal_save")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(theme.accent)
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(0xffffff))
                                    .hover(|s| s.opacity(0.9))
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _, window, cx| {
                                        this.handle_modal_save(&key_save, window, cx);
                                    }))
                                    .child(format!("{} (S)", strings.modal_save)),
                            ),
                    ),
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
        self.window_handle = Some(window.window_handle());

        let theme = &self.state.theme;
        let strings = self.state.strings;
        let view_mode = self.state.view_mode;
        let is_dirty = self.state.is_dirty();
        let has_conflict = self.state.has_conflict();
        let parse_paused = self.parse_paused;

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

        let view_doc_switch = cx.entity().clone();
        let on_select_doc = Arc::new(move |key: &crate::state::DocKey, _window: &mut Window, cx: &mut App| {
            view_doc_switch.update(cx, |this, cx| {
                this.state.switch_document(key);
                this.on_document_swapped();
                cx.notify();
            });
        });

        let view_doc_close = cx.entity().clone();
        let on_close_doc = Arc::new(move |key: &crate::state::DocKey, _window: &mut Window, cx: &mut App| {
            view_doc_close.update(cx, |this, cx| {
                this.trigger_close_doc(key, cx);
            });
        });

        let view_draft = cx.entity().clone();
        let on_new_draft = Arc::new(move |_window: &mut Window, cx: &mut App| {
            view_draft.update(cx, |this, cx| {
                this.trigger_draft(cx);
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
                                match this.state.add_workspace_root(path) {
                                    Ok(_) => {
                                        this.error_banner = None;
                                        this.state.clear_error();
                                    }
                                    Err(e) => {
                                        let strings = this.state.strings;
                                        let msg = match e {
                                            crate::state::RootAddError::NotFound => {
                                                strings.error_root_not_found.to_string()
                                            }
                                            crate::state::RootAddError::NotADirectory => {
                                                strings.error_root_not_dir.to_string()
                                            }
                                            crate::state::RootAddError::AlreadyExists => {
                                                strings.error_root_already_exists.to_string()
                                            }
                                            crate::state::RootAddError::NestedInExisting(p) => {
                                                format!("{}: {}", strings.error_root_nested, p.display())
                                            }
                                            crate::state::RootAddError::ContainsExisting(p) => {
                                                format!("{}: {}", strings.error_root_parent, p.display())
                                            }
                                            crate::state::RootAddError::SaveFailed(err) => {
                                                format!("{}: {}", strings.error_save_failed, err)
                                            }
                                            crate::state::RootAddError::CanonicalizeFailed(p, err) => {
                                                format!("{}: {}: {}", strings.error_canonicalize_failed, p.display(), err)
                                            }
                                        };
                                        this.error_banner = Some(msg.clone());
                                        this.state.set_error(msg);
                                    }
                                }
                                cx.notify();
                            });
                        }
                    }
                })
                .detach();
            });
        });

        let view_toggle = cx.entity().clone();
        let on_toggle_drawer = Arc::new(move |_window: &mut Window, cx: &mut App| {
            view_toggle.update(cx, |this, cx| {
                this.toggle_drawer(cx);
            });
        });

        let view_toggle_root = cx.entity().clone();
        let on_toggle_root = Arc::new(move |root_idx: usize, _window: &mut Window, cx: &mut App| {
            view_toggle_root.update(cx, |this, cx| {
                if root_idx < this.state.workspace_roots.len() {
                    let path = this.state.workspace_roots[root_idx].path.clone();
                    this.state.toggle_root_folder(root_idx, &path);
                    cx.notify();
                }
            });
        });

        let view_remove_root = cx.entity().clone();
        let on_remove_root = Arc::new(move |root_idx: usize, _window: &mut Window, cx: &mut App| {
            view_remove_root.update(cx, |this, cx| {
                match this.state.remove_workspace_root(root_idx) {
                    Ok(_) => {
                        this.error_banner = None;
                        this.state.clear_error();
                    }
                    Err(e) => {
                        let strings = this.state.strings;
                        let msg = format!("{}: {}", strings.error_remove_failed, e);
                        this.error_banner = Some(msg.clone());
                        this.state.set_error(msg);
                    }
                }
                cx.notify();
            });
        });

        let view_refresh_root = cx.entity().clone();
        let on_refresh_root = Arc::new(move |root_idx: usize, _window: &mut Window, cx: &mut App| {
            view_refresh_root.update(cx, |this, cx| {
                this.state.refresh_root(root_idx);
                cx.notify();
            });
        });

        let view_tree = cx.entity().clone();
        let on_toggle_folder = Arc::new(
            move |root_idx: usize, path: &std::path::PathBuf, _window: &mut Window, cx: &mut App| {
                view_tree.update(cx, |this, cx| {
                    this.state.toggle_root_folder(root_idx, path);
                    cx.notify();
                });
            },
        );

        let loose_docs = self.state.loose_docs();

        let view_lb = cx.entity().clone();
        let on_open_lightbox = Arc::new(move |svg_path: std::path::PathBuf, hash: u64, _window: &mut Window, cx: &mut App| {
            view_lb.update(cx, |this, cx| {
                this.lightbox.open(svg_path, hash);
                cx.notify();
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
                this.trigger_save(cx);
            }))
            .on_action(cx.listener(|this, _: &ExportHtml, _window, _cx| {
                let html = this.state.export_standalone_html();
                if let Some(path) = &this.state.doc.path {
                    let out_path = path.with_extension("html");
                    let _ = std::fs::write(&out_path, html);
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.lightbox.is_open {
                    if event.keystroke.key == "escape" {
                        this.lightbox.close();
                        cx.notify();
                        return;
                    }
                }

                if let Some(pending) = this.state.pending_close.clone() {
                    match event.keystroke.key.as_str() {
                        "escape" => {
                            this.handle_modal_cancel(cx);
                            return;
                        }
                        "d" | "D" => {
                            this.handle_modal_discard(&pending.key, window, cx);
                            return;
                        }
                        "enter" | "s" | "S" => {
                            this.handle_modal_save(&pending.key, window, cx);
                            return;
                        }
                        _ => {}
                    }
                }

                if this.is_renaming {
                    match event.keystroke.key.as_str() {
                        "enter" => {
                            let new_name = this.rename_buffer.trim();
                            if !new_name.is_empty() {
                                let _ = this.state.rename_file(new_name);
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
                            this.trigger_save(cx);
                        }
                        "w" => {
                            this.trigger_close_active(cx);
                        }
                        "n" => {
                            this.trigger_draft(cx);
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
            .flex_row()
            .bg(theme.bg)
            .text_color(theme.text_primary)
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
                                    &self.state.workspace_roots,
                                    &loose_docs,
                                    &self.state.open_docs,
                                    self.state.active.as_ref(),
                                    is_dirty,
                                    &self.state.doc.parsed.headings(),
                                    theme,
                                    strings,
                                    &self.drawer_scroll_handle,
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
            // 2. Center Content Canvas (Seamless edge-to-edge surface with integrated 38px header)
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
                    .child(self.render_canvas_header(theme, cx))
                    // Error Feedback Banner
                    .when_some(self.error_banner.clone(), |el, err_msg| {
                        el.child(
                            div()
                                .id("workspace_error_banner")
                                .h(px(36.))
                                .px_4()
                                .bg(rgba(0xef44441a))
                                .border_b_1()
                                .border_color(rgba(0xef444433))
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            svg()
                                                .path("icons/info.svg")
                                                .size(px(14.))
                                                .text_color(rgb(0xef4444)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(rgb(0xef4444))
                                                .child(err_msg),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("btn_close_error_banner")
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(rgb(0xef4444))
                                        .hover(|s| s.bg(rgba(0xef444422)))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _window, cx| {
                                                this.error_banner = None;
                                                this.state.clear_error();
                                                cx.notify();
                                            }),
                                        )
                                        .child("×"),
                                ),
                        )
                    })
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
                            .child({
                                let copied_code_hash = self.copied_code_hash;
                                let on_copy = {
                                    let view_entity = cx.entity().clone();
                                    Arc::new(move |code_text: String, hash: u64, _window: &mut Window, app: &mut App| {
                                        app.write_to_clipboard(ClipboardItem::new_string(code_text));
                                        let view_entity = view_entity.clone();
                                        view_entity.update(app, |view, cx| {
                                            view.copied_code_hash = Some((hash, std::time::Instant::now()));
                                            cx.notify();
                                            cx.spawn(async move |this, app| {
                                                app.background_executor().timer(std::time::Duration::from_millis(1200)).await;
                                                let _ = this.update(app, |view, cx| {
                                                    if let Some((curr_h, _)) = view.copied_code_hash {
                                                        if curr_h == hash {
                                                            view.copied_code_hash = None;
                                                            cx.notify();
                                                        }
                                                    }
                                                });
                                            }).detach();
                                        });
                                    })
                                };

                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .min_h(px(0.))
                                    .size_full()
                                    .overflow_hidden()
                                    .child(if self.state.active.is_none() {
                                        div()
                                            .size_full()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .justify_center()
                                            .gap_3()
                                            .text_color(theme.text_muted)
                                            .child(
                                                svg()
                                                    .path("icons/files.svg")
                                                    .size(px(40.))
                                                    .text_color(theme.border),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(strings.no_document_open),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.))
                                                    .child(strings.no_document_open_desc),
                                            )
                                            .into_any_element()
                                    } else {
                                        match view_mode {
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
                                                    strings,
                                                    self.state.preferences.max_column_width,
                                                    self.state.preferences.font_size,
                                                    &self.preview_list,
                                                    copied_code_hash,
                                                    Some(on_copy.clone()),
                                                    Some(on_open_lightbox.clone()),
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
                                                        .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                                                            this.handle_editor_scroll_wheel(event, window, cx);
                                                        }))
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
                                                        .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                                                            this.handle_preview_scroll_wheel(event, window, cx);
                                                        }))
                                                        .child(render_preview_pane(
                                                            self.state.doc.parsed.clone(),
                                                            theme,
                                                            strings,
                                                            self.state.preferences.max_column_width,
                                                            self.state.preferences.font_size,
                                                            &self.preview_list,
                                                            copied_code_hash,
                                                            Some(on_copy),
                                                            Some(on_open_lightbox.clone()),
                                                        )),
                                                )
                                                .into_any_element(),
                                        }
                                    })
                            })
                    )
                    .when(self.lightbox.is_open, |root| {
                        root.child(render_lightbox(&self.lightbox, theme, strings, cx))
                    })
                    .when(self.state.pending_close.is_some(), |root| {
                        root.child(self.render_close_modal(theme, cx))
                    })
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
        base.bg(theme.warm_pill_bg())
            .text_color(theme.warm_pill_text())
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
