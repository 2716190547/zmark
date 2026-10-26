use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use crate::theme::ThemeColors;

pub fn render_editor_pane(
    lines: &[String],
    cursor: (usize, usize),
    theme: &ThemeColors,
    font_size: f32,
    is_dirty: bool,
    title: &str,
    scroll_handle: &ScrollHandle,
    on_click_line: Arc<dyn Fn(usize, usize, &mut Window, &mut App)>,
) -> impl IntoElement {
    let total_lines = lines.len().max(1);
    let (cursor_line, cursor_col) = cursor;

    div()
        .flex_1()
        .min_h(px(0.))
        .h_full()
        .flex()
        .flex_col()
        .bg(theme.surface)
        .child(
            div()
                .h(px(32.))
                .px_4()
                .bg(theme.bg)
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
                                .child(title.to_string()),
                        )
                        .when(is_dirty, |el| {
                            el.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .w(px(6.))
                                            .h(px(6.))
                                            .rounded_full()
                                            .bg(theme.accent),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.accent)
                                            .child("Modified"),
                                    ),
                            )
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_muted)
                        .child(format!("{} lines  Ln {}, Col {}", total_lines, cursor_line + 1, cursor_col + 1)),
                ),
        )
        .child(
            div()
                .id("editor_scroll_container")
                .track_scroll(scroll_handle)
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scroll()
                .p_4()
                .flex()
                .flex_col()
                .children(lines.iter().enumerate().map(|(line_idx, line)| {
                    let is_active = line_idx == cursor_line;
                    let line_content = line.clone();
                    let on_click_line = on_click_line.clone();

                    let row = div()
                        .flex()
                        .items_start()
                        .min_h(px(font_size * 1.5))
                        .px_1()
                        .rounded_sm()
                        .cursor_text()
                        .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
                            let x_offset = f32::from(event.position.x - px(56.)).max(0.0);
                            let char_width = font_size * 0.6;
                            let col = (x_offset / char_width) as usize;
                            on_click_line(line_idx, col, window, cx);
                        });

                    let styled_row = if is_active {
                        row.bg(theme.bg)
                    } else {
                        row.hover(|s| s.bg(theme.bg))
                    };

                    styled_row
                        .child(
                            div()
                                .w(px(36.))
                                .flex_shrink_0()
                                .mr_3()
                                .text_right()
                                .text_size(px(font_size * 0.85))
                                .font_family("ui-monospace")
                                .text_color(if is_active { theme.accent } else { theme.text_muted })
                                .child((line_idx + 1).to_string()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(px(font_size))
                                .font_family("ui-monospace")
                                .text_color(theme.text_primary)
                                .child(if is_active {
                                    let char_count = line_content.chars().count();
                                    let col = cursor_col.min(char_count);
                                    let before: String = line_content.chars().take(col).collect();
                                    let after: String = line_content.chars().skip(col).collect();

                                    div()
                                        .flex()
                                        .flex_wrap()
                                        .items_center()
                                        .child(div().child(if before.is_empty() { String::new() } else { before }))
                                        .child(
                                            div()
                                                .w(px(2.))
                                                .h(px(font_size * 1.2))
                                                .bg(theme.accent)
                                                .mx(px(0.5)),
                                        )
                                        .child(div().child(if after.is_empty() { String::new() } else { after }))
                                        .into_any_element()
                                } else {
                                    div()
                                        .child(if line_content.is_empty() { " ".to_string() } else { line_content })
                                        .into_any_element()
                                }),
                        )
                })),
        )
}
