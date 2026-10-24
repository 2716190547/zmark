use gpui::prelude::*;
use gpui::*;

use crate::actions::{
    ExportHtml, SaveFile, ToggleDrawer, TogglePreviewMode, ToggleSourceMode, ToggleSplitMode,
};
use crate::state::{AppState, ViewMode};
use crate::views::drawer::render_drawer;
use crate::views::editor::render_editor_pane;
use crate::views::preview::render_preview_pane;

pub struct WorkspaceView {
    pub state: AppState,
    pub focus_handle: FocusHandle,
}

impl WorkspaceView {
    pub fn new(state: AppState, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = &self.state.theme;
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
            .unwrap_or("Untitled.md")
            .to_string();

        div()
            .key_context("Workspace")
            .track_focus(&self.focus_handle)
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
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.bg)
            .text_color(theme.text_primary)
            // Top Toolbar
            .child(
                div()
                    .h(px(40.))
                    .px_4()
                    .bg(theme.surface)
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .items_center()
                    .justify_between()
                    // Left area: Drawer toggle + Title
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.bg))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _window, cx| {
                                            this.state.toggle_drawer();
                                            cx.notify();
                                        }),
                                    )
                                    .child(if self.state.drawer_open {
                                        "◀ Close Drawer"
                                    } else {
                                        "☰ Directory (⌘B)"
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(file_title),
                                    )
                                    .when(is_dirty, |el| {
                                        el.child(
                                            div()
                                                .w(px(8.))
                                                .h(px(8.))
                                                .rounded_full()
                                                .bg(theme.accent),
                                        )
                                    }),
                            ),
                    )
                    // Center area: View Mode Switcher
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .bg(theme.bg)
                            .p_1()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .gap_1()
                            .child(render_mode_tab(
                                "Source (⌘1)",
                                view_mode == ViewMode::Source,
                                theme,
                                cx.listener(|this, _, _window, cx| {
                                    this.state.set_view_mode(ViewMode::Source);
                                    cx.notify();
                                }),
                            ))
                            .child(render_mode_tab(
                                "Split (⌘2)",
                                view_mode == ViewMode::Split,
                                theme,
                                cx.listener(|this, _, _window, cx| {
                                    this.state.set_view_mode(ViewMode::Split);
                                    cx.notify();
                                }),
                            ))
                            .child(render_mode_tab(
                                "Preview (⌘3)",
                                view_mode == ViewMode::Preview,
                                theme,
                                cx.listener(|this, _, _window, cx| {
                                    this.state.set_view_mode(ViewMode::Preview);
                                    cx.notify();
                                }),
                            )),
                    )
                    // Right area: Action buttons
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme.border)
                                    .text_xs()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.bg))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _window, _cx| {
                                            let html = this.state.export_standalone_html();
                                            if let Some(path) = &this.state.doc.path {
                                                let out_path = path.with_extension("html");
                                                let _ = std::fs::write(&out_path, html);
                                            }
                                        }),
                                    )
                                    .child("Export HTML (⌘⇧E)"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(theme.accent)
                                    .text_color(rgb(0xffffff))
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _window, cx| {
                                            let _ = this.state.save();
                                            cx.notify();
                                        }),
                                    )
                                    .child("Save (⌘S)"),
                            ),
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
                                .text_color(rgb(0xfef08a))
                                .child("⚠️ Warning: This file was modified externally on disk."),
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
                                        .child("Keep Local Edits"),
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
                                        .child("Reload From Disk"),
                                ),
                        ),
                )
            })
            // Main workspace content layout
            .child(
                div()
                    .flex_1()
                    .flex()
                    .overflow_hidden()
                    .when(self.state.drawer_open, |body| {
                        body.child(render_drawer(
                            &self.state.sibling_files,
                            self.state.doc.path.as_ref(),
                            theme,
                        ))
                    })
                    .child(match view_mode {
                        ViewMode::Source => div()
                            .flex_1()
                            .h_full()
                            .child(render_editor_pane(
                                &self.state.doc.buffer,
                                theme,
                                self.state.preferences.font_size,
                                is_dirty,
                            ))
                            .into_any_element(),
                        ViewMode::Preview => div()
                            .flex_1()
                            .h_full()
                            .child(render_preview_pane(
                                &self.state.doc.parsed,
                                theme,
                                self.state.preferences.max_column_width,
                                self.state.preferences.font_size,
                            ))
                            .into_any_element(),
                        ViewMode::Split => div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .child(
                                div()
                                    .w_1_2()
                                    .h_full()
                                    .border_r_1()
                                    .border_color(theme.border)
                                    .child(render_editor_pane(
                                        &self.state.doc.buffer,
                                        theme,
                                        self.state.preferences.font_size,
                                        is_dirty,
                                    )),
                            )
                            .child(
                                div()
                                    .w_1_2()
                                    .h_full()
                                    .child(render_preview_pane(
                                        &self.state.doc.parsed,
                                        theme,
                                        self.state.preferences.max_column_width,
                                        self.state.preferences.font_size,
                                    )),
                            )
                            .into_any_element(),
                    }),
            )
    }
}

fn render_mode_tab<F>(
    label: &'static str,
    active: bool,
    theme: &crate::theme::ThemeColors,
    handler: F,
) -> impl IntoElement
where
    F: Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
{
    let base = div()
        .px_3()
        .py_1()
        .rounded_sm()
        .text_xs()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, handler);

    if active {
        base.bg(theme.surface)
            .text_color(theme.accent)
            .font_weight(FontWeight::SEMIBOLD)
            .child(label)
    } else {
        base.text_color(theme.text_muted)
            .hover(|s| s.text_color(theme.text_primary))
            .child(label)
    }
}
