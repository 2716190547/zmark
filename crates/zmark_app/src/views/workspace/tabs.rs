//! Optional tab bar in the canvas header (`Preferences::show_tab_bar`).
//!
//! Tabs are derived from `AppState::open_docs` on every render; the only state
//! kept here is the horizontal scroll offset. Reordering writes straight back to
//! `open_docs`, so the restored session keeps the same order.

use super::*;
use crate::state::DocKey;
use zmark_core::document::DocumentStatus;

/// Payload carried while a tab is being dragged to a new position.
#[derive(Clone)]
pub(super) struct DraggedTab {
    key: DocKey,
    title: String,
    text: Hsla,
    bg: Hsla,
    border: Hsla,
}

impl Render for DraggedTab {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(TAB_H))
            .px_2p5()
            .flex()
            .items_center()
            .rounded(px(6.))
            .bg(self.bg)
            .border_1()
            .border_color(self.border)
            .shadow_md()
            .opacity(0.9)
            .text_size(px(13.))
            .font_family(crate::platform::ui_font())
            .text_color(self.text)
            .child(self.title.clone())
    }
}

const TAB_H: f32 = 26.0;
const TAB_MAX_W: f32 = 220.0;

impl WorkspaceView {
    /// Keeps the active tab inside the visible part of the tab bar.
    pub(crate) fn reveal_active_tab(&self) {
        if !self.state.preferences.show_tab_bar {
            return;
        }
        if let Some(ix) = self.state.active.as_ref()
            .and_then(|key| self.state.open_docs.iter().position(|d| &d.key == key))
        {
            self.tab_bar_scroll.scroll_to_item(ix);
        }
    }

    pub(super) fn activate_document_key(&mut self, key: DocKey, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.active.as_ref() == Some(&key) {
            return;
        }
        self.state.switch_document(&key);
        self.on_document_swapped(Some(window), cx);
        cx.notify();
    }

    pub(super) fn render_tab_bar(
        &self,
        theme: &crate::theme::ThemeColors,
        active_dirty: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let strings = self.state.strings;
        let active = self.state.active.clone();
        let renaming = self.is_renaming && !self.rename_in_sidebar;
        let selected_bg = theme.control_selection_bg();
        let hover_bg = theme.hover_bg();

        let tabs = self.state.open_docs.iter().enumerate().map(|(ix, open)| {
            let key = open.key.clone();
            let is_active = active.as_ref() == Some(&key);
            let dirty = if is_active { active_dirty } else { open.doc.status != DocumentStatus::Clean };
            let title = open.doc.display_name_with_fallback(strings.untitled_file);
            let tooltip = match &open.doc.path {
                Some(path) => path.to_string_lossy().into_owned(),
                None => strings.draft_unsaved_hint.to_string(),
            };
            let group: SharedString = format!("doc_tab_{ix}").into();
            let text = if is_active { theme.text_primary } else { theme.text_muted };

            let label = if renaming && is_active && self.rename_input.is_some() {
                div()
                    .id("tab_rename_box")
                    .key_context("InlineRename")
                    .debug_selector(|| "tab_rename_box".into())
                    .w(px(160.))
                    .h(px(20.))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                        this.finish_rename(false, cx);
                    }))
                    .child(Styled::h(
                        crate::views::drawer::render_input(self.rename_input.as_ref().unwrap(), false),
                        px(20.),
                    ))
                    .into_any_element()
            } else {
                div()
                    .min_w(px(0.))
                    .truncate()
                    .child(title.clone())
                    .into_any_element()
            };

            // One 16px slot shows the dirty dot, swapped for × while hovered, so
            // the tab never changes width.
            let close_key = key.clone();
            let middle_key = key.clone();
            let close_slot = div()
                .relative()
                .flex_none()
                .size(px(16.))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(!dirty, |el| el.invisible())
                        .group_hover(group.clone(), |s| s.invisible())
                        .child(div().size(px(6.)).rounded_full().bg(theme.text_muted)),
                )
                .child(
                    div()
                        .id(("doc_tab_close", ix))
                        .absolute()
                        .inset_0()
                        .rounded(px(4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .invisible()
                        .group_hover(group.clone(), |s| s.visible())
                        .hover(|s| s.bg(hover_bg))
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            if !this.workspace_shortcut_blocked() {
                                this.trigger_close_doc(&close_key, Some(window), cx);
                            }
                        }))
                        .child(svg().path("icons/x.svg").size(px(12.)).text_color(theme.text_muted)),
                );

            let click_key = key.clone();
            let drop_key = key.clone();
            div()
                .id(("doc_tab", ix))
                .group(group)
                .debug_selector(move || format!("doc_tab_{ix}"))
                .flex_none()
                .max_w(px(TAB_MAX_W))
                .h(px(TAB_H))
                .pl_2p5()
                .pr_1()
                .flex()
                .items_center()
                .gap_1()
                .rounded(px(6.))
                .text_size(px(13.))
                .line_height(px(18.))
                .text_color(text)
                .cursor_pointer()
                .map(|el| if is_active { el.bg(selected_bg).font_weight(FontWeight::MEDIUM) } else { el.hover(|s| s.bg(hover_bg)) })
                .tooltip(move |_window, cx| {
                    let t = tooltip.clone();
                    cx.new(|_| TooltipView { text: t }).into()
                })
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if this.workspace_shortcut_blocked() {
                        return;
                    }
                    if this.state.active.as_ref() != Some(&click_key) {
                        this.activate_document_key(click_key.clone(), window, cx);
                    } else if event.click_count == 2 {
                        this.begin_rename(window, cx);
                    }
                }))
                .on_mouse_down(MouseButton::Middle, cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    if !this.workspace_shortcut_blocked() {
                        this.trigger_close_doc(&middle_key, Some(window), cx);
                    }
                }))
                .on_drag(
                    DraggedTab { key: key.clone(), title, text: theme.text_primary, bg: theme.surface, border: theme.border },
                    |tab, _offset, _window, cx| cx.new(|_| tab.clone()),
                )
                .drag_over::<DraggedTab>(move |s, dragged, _, _| {
                    if dragged.key == drop_key { s } else { s.bg(hover_bg) }
                })
                .on_drop(cx.listener(move |this, dragged: &DraggedTab, _window, cx| {
                    if this.state.move_document(&dragged.key, ix) {
                        this.reveal_active_tab();
                        cx.notify();
                    }
                }))
                .child(label)
                .child(close_slot)
        });

        div()
            .id("canvas_tab_bar")
            .debug_selector(|| "canvas_tab_bar".into())
            .flex()
            .items_center()
            .gap_0p5()
            .min_w(px(0.))
            .h_full()
            .overflow_x_scroll()
            .track_scroll(&self.tab_bar_scroll)
            .children(tabs)
            .into_any_element()
    }
}
