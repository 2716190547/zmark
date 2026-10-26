use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;

use crate::actions::{
    ExportHtml, OpenFileDialog, SaveFile, ToggleDrawer, TogglePreviewMode, ToggleSettings,
    ToggleSourceMode, ToggleSplitMode,
};
use crate::state::{AppState, ViewMode};
use crate::views::drawer::render_drawer;
use crate::views::editor::render_editor_pane;
use crate::views::preview::render_preview_pane;
use crate::views::settings::render_settings_modal;

pub struct WorkspaceView {
    pub state: AppState,
    pub focus_handle: FocusHandle,
    pub editor_scroll_handle: ScrollHandle,
    pub preview_scroll_handle: ScrollHandle,
    pub drawer_scroll_handle: ScrollHandle,
    pub split_ratio: f32,
    pub is_resizing_split: bool,
    pub is_renaming: bool,
    pub rename_buffer: String,
    pub workspace_width: f32,
    pub auto_save_task: Option<Task<()>>,
}

impl WorkspaceView {
    pub fn new(state: AppState, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus_handle: cx.focus_handle(),
            editor_scroll_handle: ScrollHandle::new(),
            preview_scroll_handle: ScrollHandle::new(),
            drawer_scroll_handle: ScrollHandle::new(),
            split_ratio: 0.5,
            is_resizing_split: false,
            is_renaming: false,
            rename_buffer: String::new(),
            workspace_width: 1200.0,
            auto_save_task: None,
        }
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
                let y_approx = (block_idx as f32) * 55.0;
                this.preview_scroll_handle.set_offset(point(px(0.), px(-y_approx)));
                cx.notify();
            });
        });

        let view_close = cx.entity().clone();
        let on_close_settings = Arc::new(move |_: &MouseDownEvent, _window: &mut Window, cx: &mut App| {
            view_close.update(cx, |this, cx| {
                this.state.settings_open = false;
                cx.notify();
            });
        });

        let view_mode_setter = cx.entity().clone();
        let on_set_mode = Arc::new(move |mode, _window: &mut Window, cx: &mut App| {
            view_mode_setter.update(cx, |this, cx| {
                this.state.set_theme_mode(mode);
                cx.notify();
            });
        });

        let view_pal_setter = cx.entity().clone();
        let on_set_palette = Arc::new(move |palette: &'static str, _window: &mut Window, cx: &mut App| {
            view_pal_setter.update(cx, |this, cx| {
                this.state.set_palette(palette);
                cx.notify();
            });
        });

        let view_loc_setter = cx.entity().clone();
        let on_set_locale = Arc::new(move |locale, _window: &mut Window, cx: &mut App| {
            view_loc_setter.update(cx, |this, cx| {
                this.state.set_locale(locale);
                cx.notify();
            });
        });

        let view_tab_setter = cx.entity().clone();
        let on_set_settings_tab = Arc::new(move |tab: crate::state::SettingsTab, _window: &mut Window, cx: &mut App| {
            view_tab_setter.update(cx, |this, cx| {
                this.state.set_settings_tab(tab);
                cx.notify();
            });
        });

        let view_as_setter = cx.entity().clone();
        let on_toggle_auto_save = Arc::new(move |enabled: bool, _window: &mut Window, cx: &mut App| {
            view_as_setter.update(cx, |this, cx| {
                this.state.set_auto_save(enabled);
                cx.notify();
            });
        });

        let view_asb_setter = cx.entity().clone();
        let on_toggle_auto_save_on_blur = Arc::new(move |enabled: bool, _window: &mut Window, cx: &mut App| {
            view_asb_setter.update(cx, |this, cx| {
                this.state.set_auto_save_on_blur(enabled);
                cx.notify();
            });
        });

        let view_ts_setter = cx.entity().clone();
        let on_set_tab_size = Arc::new(move |tab_size: usize, _window: &mut Window, cx: &mut App| {
            view_ts_setter.update(cx, |this, cx| {
                this.state.set_tab_size(tab_size);
                cx.notify();
            });
        });

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
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                if this.is_resizing_split {
                    let w = this.workspace_width;
                    let x = f32::from(event.position.x);
                    let offset_x = if this.state.drawer_open { (x - 240.0).max(0.0) } else { x };
                    let avail_w = if this.state.drawer_open { (w - 240.0).max(100.0) } else { w };
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
                this.state.set_view_mode(ViewMode::Source);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleSplitMode, _window, cx| {
                this.state.set_view_mode(ViewMode::Split);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &TogglePreviewMode, _window, cx| {
                this.state.set_view_mode(ViewMode::Preview);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleDrawer, _window, cx| {
                this.state.toggle_drawer();
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleSettings, _window, cx| {
                this.state.toggle_settings();
                cx.notify();
            }))
            .on_action(cx.listener(|_this, _: &OpenFileDialog, _window, cx| {
                cx.spawn(async move |this, app| {
                    let file = rfd::AsyncFileDialog::new()
                        .add_filter("Markdown", &["md", "markdown", "txt"])
                        .pick_file()
                        .await;
                    if let Some(file) = file {
                        let path = file.path().to_path_buf();
                        let _ = this.update(app, |this, cx| {
                            let _ = this.state.load_file(path);
                            cx.notify();
                        });
                    }
                })
                .detach();
            }))
            .on_action(cx.listener(|this, _: &SaveFile, _window, cx| {
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
                if this.state.settings_open {
                    if event.keystroke.key == "escape" {
                        this.state.settings_open = false;
                        cx.notify();
                    }
                    return;
                }

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
                            this.state.sync_from_editor();
                            this.trigger_auto_save(cx);
                            cx.notify();
                        }
                        "s" => {
                            let _ = this.state.save();
                            cx.notify();
                        }
                        "o" => {
                            cx.spawn(async move |this, app| {
                                let file = rfd::AsyncFileDialog::new()
                                    .add_filter("Markdown", &["md", "markdown", "txt"])
                                    .pick_file()
                                    .await;
                                if let Some(file) = file {
                                    let path = file.path().to_path_buf();
                                    let _ = this.update(app, |this, cx| {
                                        let _ = this.state.load_file(path);
                                        cx.notify();
                                    });
                                }
                            })
                            .detach();
                        }
                        "," => {
                            this.state.toggle_settings();
                            cx.notify();
                        }
                        "b" => {
                            this.state.toggle_drawer();
                            cx.notify();
                        }
                        "1" => {
                            this.state.set_view_mode(ViewMode::Source);
                            cx.notify();
                        }
                        "2" => {
                            this.state.set_view_mode(ViewMode::Split);
                            cx.notify();
                        }
                        "3" => {
                            this.state.set_view_mode(ViewMode::Preview);
                            cx.notify();
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
                    this.state.sync_from_editor();
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
                    .bg(theme.surface)
                    .border_b_1()
                    .border_color(theme.border.opacity(0.35))
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
                                    this.state.toggle_drawer();
                                    cx.notify();
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
                                    cx.spawn(async move |this, app| {
                                        let file = rfd::AsyncFileDialog::new()
                                            .add_filter("Markdown", &["md", "markdown", "txt"])
                                            .pick_file()
                                            .await;
                                        if let Some(file) = file {
                                            let path = file.path().to_path_buf();
                                            let _ = this.update(app, |this, cx| {
                                                let _ = this.state.load_file(path);
                                                cx.notify();
                                            });
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
                                    this.state.set_view_mode(ViewMode::Source);
                                    cx.notify();
                                }),
                            ))
                            .child(render_mode_icon_tab(
                                "btn_mode_split",
                                format!("{} (⌘2)", strings.split_mode),
                                view_mode == ViewMode::Split,
                                theme,
                                render_split_mode_icon(if view_mode == ViewMode::Split { theme.accent } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.state.set_view_mode(ViewMode::Split);
                                    cx.notify();
                                }),
                            ))
                            .child(render_mode_icon_tab(
                                "btn_mode_preview",
                                format!("{} (⌘3)", strings.preview_mode),
                                view_mode == ViewMode::Preview,
                                theme,
                                render_preview_mode_icon(if view_mode == ViewMode::Preview { theme.accent } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.state.set_view_mode(ViewMode::Preview);
                                    cx.notify();
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
                                    let _ = this.state.save();
                                    cx.notify();
                                }),
                            ))
                            // Settings icon button
                            .child(render_icon_button(
                                "btn_settings",
                                format!("{} (⌘,)", strings.settings),
                                self.state.settings_open,
                                theme,
                                render_settings_icon(if self.state.settings_open { theme.accent } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.state.toggle_settings();
                                    cx.notify();
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
                                                }
                                                cx.notify();
                                            }),
                                        )
                                        .child(strings.reload_disk),
                                ),
                        ),
                )
            })
            // Main workspace content layout
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .flex()
                    .overflow_hidden()
                    .when(self.state.drawer_open, |body| {
                        body.child(
                            div()
                                .id("animated_drawer_wrapper")
                                .h_full()
                                .overflow_hidden()
                                .child(render_drawer(
                                    self.state.drawer_tab,
                                    &self.state.sibling_files,
                                    self.state.doc.path.as_ref(),
                                    &self.state.doc.parsed.headings(),
                                    theme,
                                    strings,
                                    &self.drawer_scroll_handle,
                                    on_select_tab,
                                    on_select_file,
                                    on_select_heading,
                                ))
                                .with_animation(
                                    "drawer_slide_in",
                                    Animation::new(std::time::Duration::from_millis(180)).with_easing(ease_in_out),
                                    |el, delta| {
                                        el.w(px(250.0 * delta)).opacity(delta.clamp(0.0, 1.0))
                                    },
                                ),
                        )
                    })
                    .child(match view_mode {
                        ViewMode::Source => div()
                            .flex_1()
                            .min_h(px(0.))
                            .min_w(px(0.))
                            .h_full()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(render_editor_pane(
                                &self.state.editor.lines,
                                self.state.editor.cursor(),
                                theme,
                                self.state.preferences.font_size,
                                is_dirty,
                                strings.source_mode,
                                &self.editor_scroll_handle,
                                on_click_line,
                            ))
                            .into_any_element(),
                        ViewMode::Preview => div()
                            .flex_1()
                            .min_h(px(0.))
                            .min_w(px(0.))
                            .h_full()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(render_preview_pane(
                                &self.state.doc.parsed,
                                theme,
                                self.state.preferences.max_column_width,
                                self.state.preferences.font_size,
                                &self.preview_scroll_handle,
                                strings.preview_mode,
                            ))
                            .into_any_element(),
                        ViewMode::Split => div()
                            .flex_1()
                            .min_h(px(0.))
                            .min_w(px(0.))
                            .h_full()
                            .flex()
                            .overflow_hidden()
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
                                        &self.state.editor.lines,
                                        self.state.editor.cursor(),
                                        theme,
                                        self.state.preferences.font_size,
                                        is_dirty,
                                        strings.source_mode,
                                        &self.editor_scroll_handle,
                                        on_click_line,
                                    )),
                            )
                            // Draggable resizable divider: 5px hit area with delicate hairline center
                            .child(
                                div()
                                    .w(px(5.))
                                    .h_full()
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .cursor_col_resize()
                                    .hover(|s| s.bg(theme.surface))
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
                                    .child(div().w(px(1.)).h_full().bg(theme.border.opacity(0.35))),
                            )
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
                                        &self.state.doc.parsed,
                                        theme,
                                        self.state.preferences.max_column_width,
                                        self.state.preferences.font_size,
                                        &self.preview_scroll_handle,
                                        strings.preview_mode,
                                    )),
                            )
                            .into_any_element(),
                    }),
            )
            // Settings Modal
            .when(self.state.settings_open, |body| {
                body.child(
                    div()
                        .id("animated_settings_modal")
                        .child(render_settings_modal(
                            self.state.settings_tab,
                            self.state.preferences.theme,
                            &self.state.preferences.palette,
                            self.state.preferences.locale,
                            self.state.preferences.auto_save,
                            self.state.preferences.auto_save_on_blur,
                            self.state.preferences.tab_size,
                            theme,
                            strings,
                            on_close_settings,
                            on_set_settings_tab,
                            on_set_mode,
                            on_set_palette,
                            on_set_locale,
                            on_toggle_auto_save,
                            on_toggle_auto_save_on_blur,
                            on_set_tab_size,
                        ))
                        .with_animation(
                            "settings_fade_in",
                            Animation::new(std::time::Duration::from_millis(150)).with_easing(ease_in_out),
                            |el, delta| el.opacity(delta.clamp(0.0, 1.0)),
                        ),
                )
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
        .w(px(28.))
        .h(px(28.))
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
        .w(px(32.))
        .h(px(22.))
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
    svg().path("icons/panel-left.svg").size(px(16.)).text_color(color)
}

fn render_folder_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/folder.svg").size(px(16.)).text_color(color)
}

fn render_source_mode_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/code.svg").size(px(15.)).text_color(color)
}

fn render_split_mode_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/columns-2.svg").size(px(15.)).text_color(color)
}

fn render_preview_mode_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/book-open.svg").size(px(15.)).text_color(color)
}

fn render_export_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/download.svg").size(px(16.)).text_color(color)
}

fn render_save_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/save.svg").size(px(16.)).text_color(color)
}

fn render_settings_icon(color: Hsla) -> impl IntoElement {
    svg().path("icons/settings.svg").size(px(16.)).text_color(color)
}
