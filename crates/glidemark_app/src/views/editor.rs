use gpui::prelude::*;
use gpui::*;
use crate::theme::ThemeColors;

pub fn render_editor_pane(
    buffer: &str,
    theme: &ThemeColors,
    font_size: f32,
    is_dirty: bool,
) -> impl IntoElement {
    let lines: Vec<&str> = buffer.lines().collect();
    let total_lines = lines.len().max(1);

    div()
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .bg(theme.bg)
        .child(
            div()
                .h(px(32.))
                .px_4()
                .bg(theme.surface)
                .border_b_1()
                .border_color(theme.border)
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_muted)
                                .child("SOURCE EDITOR"),
                        )
                        .when(is_dirty, |el| {
                            el.child(
                                div()
                                    .text_xs()
                                    .text_color(theme.accent)
                                    .child("● Modified"),
                            )
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(format!("{} lines", total_lines)),
                ),
        )
        .child(
            div()
                .id("editor_scroll_container")
                .flex_1()
                .overflow_y_scroll()
                .p_4()
                .flex()
                .child(
                    // Line numbers gutter
                    div()
                        .w(px(40.))
                        .mr_4()
                        .text_right()
                        .flex()
                        .flex_col()
                        .text_size(px(font_size))
                        .line_height(relative(1.6))
                        .font_family("ui-monospace")
                        .text_color(theme.text_muted)
                        .children((1..=total_lines).map(|num| div().child(num.to_string()))),
                )
                .child(
                    // Code content area
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .text_size(px(font_size))
                        .line_height(relative(1.6))
                        .font_family("ui-monospace")
                        .text_color(theme.text_primary)
                        .children(if lines.is_empty() {
                            vec![div().child(" ")]
                        } else {
                            lines
                                .into_iter()
                                .map(|line| {
                                    if line.is_empty() {
                                        div().child(" ")
                                    } else {
                                        div().child(line.to_string())
                                    }
                                })
                                .collect()
                        }),
                ),
        )
}
