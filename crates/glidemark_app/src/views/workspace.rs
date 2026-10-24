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
        }
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
                            if !this.rename_buffer.trim().is_empty() {
                                let _ = this.state.rename_file(&this.rename_buffer.clone());
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
                        key => {
                            if let Some(ch) = &event.keystroke.key_char {
                                this.rename_buffer.push_str(ch);
                                cx.notify();
                            } else if key.chars().count() == 1 && !event.keystroke.modifiers.control && !event.keystroke.modifiers.platform {
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
                    "tab" => this.state.editor.insert_text("    "),
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
                                            .px_2()
                                            .py_0p5()
                                            .bg(theme.bg)
                                            .border_1()
                                            .border_color(theme.accent)
                                            .rounded_sm()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child(format!("{}_", self.rename_buffer)),
                                            )
                                            .into_any_element()
                                    } else {
                                        div()
                                            .cursor_pointer()
                                            .hover(|s| s.text_color(theme.accent))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
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
                    // Center area: View Mode Switcher Icons
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .bg(theme.bg)
                            .p_0p5()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .gap_0p5()
                            .child(render_mode_icon_tab(
                                view_mode == ViewMode::Source,
                                theme,
                                div().text_xs().font_family("ui-monospace").font_weight(FontWeight::BOLD).child("< / >"),
                                cx.listener(|this, _, _window, cx| {
                                    this.state.set_view_mode(ViewMode::Source);
                                    cx.notify();
                                }),
                            ))
                            .child(render_mode_icon_tab(
                                view_mode == ViewMode::Split,
                                theme,
                                render_split_mode_icon(if view_mode == ViewMode::Split { theme.accent } else { theme.text_muted }),
                                cx.listener(|this, _, _window, cx| {
                                    this.state.set_view_mode(ViewMode::Split);
                                    cx.notify();
                                }),
                            ))
                            .child(render_mode_icon_tab(
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
                        body.child(render_drawer(
                            &self.state.sibling_files,
                            self.state.doc.path.as_ref(),
                            theme,
                            strings.directory_drawer,
                            &self.drawer_scroll_handle,
                            on_select_file,
                        ))
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
                                    .child(div().w(px(1.)).h_full().bg(theme.border)),
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
                body.child(render_settings_modal(
                    self.state.preferences.theme,
                    &self.state.preferences.palette,
                    self.state.preferences.locale,
                    theme,
                    strings,
                    on_close_settings,
                    on_set_mode,
                    on_set_palette,
                    on_set_locale,
                ))
            })
    }
}

// Minimalist vector icon button helper (Borderless in idle, soft capsule on hover)
fn render_icon_button<F>(
    active: bool,
    theme: &crate::theme::ThemeColors,
    icon: impl IntoElement,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let base = div()
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
    active: bool,
    theme: &crate::theme::ThemeColors,
    icon: impl IntoElement,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let base = div()
        .w(px(28.))
        .h(px(24.))
        .rounded_sm()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(MouseButton::Left, handler);

    if active {
        base.bg(theme.surface)
            .text_color(theme.accent)
            .child(icon)
    } else {
        base.text_color(theme.text_muted)
            .hover(|s| s.bg(theme.surface).text_color(theme.text_primary))
            .child(icon)
    }
}

// Native GPUI vector icons
fn render_sidebar_icon(color: Hsla) -> impl IntoElement {
    div()
        .w(px(14.))
        .h(px(12.))
        .border_1()
        .border_color(color)
        .rounded_xs()
        .flex()
        .child(div().w(px(4.)).h_full().border_r_1().border_color(color))
}

fn render_folder_icon(color: Hsla) -> impl IntoElement {
    div()
        .w(px(14.))
        .h(px(11.))
        .flex()
        .flex_col()
        .child(div().w(px(6.)).h(px(2.)).bg(color).rounded_t_xs())
        .child(div().w(px(14.)).h(px(9.)).border_1().border_color(color).rounded_xs())
}

fn render_split_mode_icon(color: Hsla) -> impl IntoElement {
    div()
        .w(px(14.))
        .h(px(12.))
        .border_1()
        .border_color(color)
        .rounded_xs()
        .flex()
        .child(div().w_1_2().h_full().border_r_1().border_color(color))
}

fn render_preview_mode_icon(color: Hsla) -> impl IntoElement {
    div()
        .w(px(13.))
        .h(px(13.))
        .border_1()
        .border_color(color)
        .rounded_xs()
        .flex()
        .flex_col()
        .justify_around()
        .p(px(2.))
        .child(div().w_full().h(px(1.5)).bg(color))
        .child(div().w_full().h(px(1.5)).bg(color))
        .child(div().w(px(6.)).h(px(1.5)).bg(color))
}

fn render_export_icon(color: Hsla) -> impl IntoElement {
    div()
        .w(px(13.))
        .h(px(13.))
        .flex()
        .flex_col()
        .items_center()
        .justify_between()
        .child(div().text_xs().line_height(relative(0.8)).font_weight(FontWeight::BOLD).text_color(color).child("^"))
        .child(div().w_full().h(px(4.)).border_b_1().border_l_1().border_r_1().border_color(color))
}

fn render_save_icon(color: Hsla) -> impl IntoElement {
    div()
        .w(px(13.))
        .h(px(13.))
        .border_1()
        .border_color(color)
        .rounded_xs()
        .flex()
        .flex_col()
        .items_center()
        .child(div().w(px(7.)).h(px(4.)).border_b_1().border_l_1().border_r_1().border_color(color))
}

fn render_settings_icon(color: Hsla) -> impl IntoElement {
    div()
        .w(px(14.))
        .h(px(11.))
        .flex()
        .flex_col()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .child(div().w(px(3.)).h(px(1.5)).bg(color))
                .child(div().w(px(4.)).h(px(4.)).rounded_full().border_1().border_color(color))
                .child(div().flex_1().h(px(1.5)).bg(color)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .child(div().flex_1().h(px(1.5)).bg(color))
                .child(div().w(px(4.)).h(px(4.)).rounded_full().border_1().border_color(color))
                .child(div().w(px(3.)).h(px(1.5)).bg(color)),
        )
}
