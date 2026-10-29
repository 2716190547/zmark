use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use crate::theme::ThemeColors;

pub fn render_editor_pane(
    lines: Arc<Vec<String>>,
    cursor: (usize, usize),
    theme: &ThemeColors,
    font_size: f32,
    is_dirty: bool,
    scroll_handle: &UniformListScrollHandle,
    on_click_line: Arc<dyn Fn(usize, usize, &mut Window, &mut App)>,
) -> impl IntoElement {
    let line_count = lines.len().max(1);
    let (cursor_line, cursor_col) = cursor;
    let theme = theme.clone();
    let list_theme = theme.clone();
    let scroll_x = (-f32::from(scroll_handle.0.borrow().base_handle.offset().x)).max(0.0);

    div()
        .flex_1()
        .min_h(px(0.))
        .h_full()
        .relative()
        .flex()
        .flex_col()
        .bg(theme.surface)
        .child(
            uniform_list("editor_lines", line_count, move |range, _window, _cx| {
                range
                    .map(|line_idx| {
                        let line = lines.get(line_idx).map(String::as_str).unwrap_or("");
                        let is_active = line_idx == cursor_line;
                        let on_click_line = on_click_line.clone();

                        let row = div()
                            .min_w_full()
                            .flex()
                            .items_center()
                            .h(px(font_size * 1.5))
                            .px_1()
                            .rounded_sm()
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
                                let x_offset = (f32::from(event.position.x - px(68.)) + scroll_x).max(0.0);
                                let char_width = font_size * 0.6;
                                let col = (x_offset / char_width) as usize;
                                on_click_line(line_idx, col, window, cx);
                            });

                        let styled_row = if is_active {
                            row.bg(list_theme.bg)
                        } else {
                            row.hover(|s| s.bg(list_theme.bg))
                        };

                        styled_row
                            .child(
                                div()
                                    .w(px(36.))
                                    .flex_shrink_0()
                                    .mr_3()
                                    .text_right()
                                    .text_size(px(font_size * 0.85))
                                    .line_height(relative(1.5))
                                    .font_family("ui-monospace")
                                    .text_color(if is_active { list_theme.accent } else { list_theme.text_muted })
                                    .child((line_idx + 1).to_string()),
                            )
                            .child(if is_active {
                                let char_count = line.chars().count();
                                let col = cursor_col.min(char_count);
                                let before: String = line.chars().take(col).collect();
                                let after: String = line.chars().skip(col).collect();

                                div()
                                    .flex_1()
                                    .whitespace_nowrap()
                                    .text_size(px(font_size))
                                    .line_height(relative(1.5))
                                    .font_family("ui-monospace")
                                    .text_color(list_theme.text_primary)
                                    .flex()
                                    .items_center()
                                    .child(before)
                                    .child(
                                        div()
                                            .w(px(2.))
                                            .h(px(font_size * 1.2))
                                            .bg(list_theme.accent)
                                            .mx(px(0.5)),
                                    )
                                    .child(after)
                                    .into_any_element()
                            } else {
                                div()
                                    .flex_1()
                                    .whitespace_nowrap()
                                    .text_size(px(font_size))
                                    .line_height(relative(1.5))
                                    .font_family("ui-monospace")
                                    .text_color(list_theme.text_primary)
                                    .child(if line.is_empty() { " ".to_string() } else { line.to_string() })
                                    .into_any_element()
                            })
                    })
                    .collect()
            })
            .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
            .track_scroll(scroll_handle.clone())
            .flex_1()
            .w_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .p_4(),
        )
        .child(
            div()
                .absolute()
                .bottom_3()
                .right_3()
                .flex()
                .items_center()
                .gap_1p5()
                .px_2()
                .py_1()
                .rounded_full()
                .bg(theme.bg)
                .border_1()
                .border_color(theme.border)
                .text_size(px(10.))
                .font_family("ui-monospace")
                .text_color(theme.text_muted)
                .when(is_dirty, |el| {
                    el.child(div().w(px(5.)).h(px(5.)).rounded_full().bg(theme.accent))
                })
                .child(format!("Ln {}, Col {}", cursor_line + 1, cursor_col + 1)),
        )
}
