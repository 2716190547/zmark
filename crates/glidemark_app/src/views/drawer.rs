use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

use crate::theme::ThemeColors;

pub fn render_drawer(
    sibling_files: &[PathBuf],
    current_path: Option<&PathBuf>,
    theme: &ThemeColors,
) -> impl IntoElement {
    div()
        .w(px(240.))
        .h_full()
        .bg(theme.surface)
        .border_r_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        .child(
            div()
                .p_3()
                .border_b_1()
                .border_color(theme.border)
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_muted)
                        .child("DIRECTORY FILES"),
                ),
        )
        .child(
            div()
                .id("drawer_file_list")
                .flex_1()
                .overflow_y_scroll()
                .p_2()
                .flex()
                .flex_col()
                .gap_1()
                .children(sibling_files.iter().map(|path| {
                    let is_active = current_path.map_or(false, |cp| cp == path);
                    let file_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("untitled.md")
                        .to_string();

                    let item = div()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .text_sm()
                        .cursor_pointer();

                    if is_active {
                        item.bg(theme.accent)
                            .text_color(rgb(0xffffff))
                            .font_weight(FontWeight::MEDIUM)
                            .child(file_name)
                    } else {
                        item.text_color(theme.text_primary)
                            .hover(|s| s.bg(theme.bg))
                            .child(file_name)
                    }
                })),
        )
}
