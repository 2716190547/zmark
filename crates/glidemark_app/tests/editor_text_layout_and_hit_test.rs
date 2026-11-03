use glidemark_app::editor_engine::char_unit_width;
use glidemark_app::views::editor::{
    byte_offset_to_char_index, calculate_click_char_col_layout, calculate_cursor_x_offset_layout,
    calculate_editor_left_offset, char_index_to_byte_offset, DRAWER_WIDTH,
    EDITOR_CONTAINER_PADDING_X, EDITOR_GUTTER_MARGIN_RIGHT, EDITOR_GUTTER_WIDTH,
    EDITOR_ROW_PADDING_X, EDITOR_TEXT_START_OFFSET,
};
use gpui::{div, IntoElement, Render, TestAppContext, Window};

struct TestView;

impl Render for TestView {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

#[gpui::test]
async fn test_text_layout_hit_testing_and_cursor_reciprocity(cx: &mut TestAppContext) {
    let (_view, cx) = cx.add_window_view(|_window, _cx| TestView);
    cx.run_until_parked();

    cx.update(|window, _app| {
        let text_system = window.text_system();
        let font_size = 14.0;

        // 1. Geometry contracts
        assert_eq!(DRAWER_WIDTH, 250.0);
        assert_eq!(EDITOR_CONTAINER_PADDING_X, 16.0);
        assert_eq!(EDITOR_ROW_PADDING_X, 4.0);
        assert_eq!(EDITOR_GUTTER_WIDTH, 36.0);
        assert_eq!(EDITOR_GUTTER_MARGIN_RIGHT, 12.0);
        assert_eq!(EDITOR_TEXT_START_OFFSET, 68.0);
        assert_eq!(calculate_editor_left_offset(true, 0.0), 318.0);
        assert_eq!(calculate_editor_left_offset(false, 0.0), 68.0);
        assert_eq!(calculate_editor_left_offset(true, 500.0), 318.0);
        assert_eq!(calculate_editor_left_offset(false, 500.0), 68.0);

        // 2. Tab width in editor engine
        assert_eq!(char_unit_width('\t'), 4);
        assert_eq!(char_unit_width('a'), 1);
        assert_eq!(char_unit_width('中'), 2);

        // 3. Byte offset <-> char index conversions
        let mixed = "Hi 世界!";
        assert_eq!(char_index_to_byte_offset(mixed, 0), 0);
        assert_eq!(char_index_to_byte_offset(mixed, 3), 3); // ' ' is 1 byte, so index of '世' is 3
        assert_eq!(char_index_to_byte_offset(mixed, 4), 6); // '世' is 3 bytes (3..6)
        assert_eq!(char_index_to_byte_offset(mixed, 5), 9); // '界' is 3 bytes (6..9)
        assert_eq!(char_index_to_byte_offset(mixed, 6), 10); // '!' is 1 byte (9..10)

        assert_eq!(byte_offset_to_char_index(mixed, 0), 0);
        assert_eq!(byte_offset_to_char_index(mixed, 3), 3);
        assert_eq!(byte_offset_to_char_index(mixed, 6), 4);
        assert_eq!(byte_offset_to_char_index(mixed, 9), 5);
        assert_eq!(byte_offset_to_char_index(mixed, 10), 6);

        // 4. ASCII layout and cursor reciprocity
        let ascii_sample = "hello world";
        let x0 = calculate_cursor_x_offset_layout(text_system, ascii_sample, 0, font_size);
        assert_eq!(x0, 0.0);
        let x5 = calculate_cursor_x_offset_layout(text_system, ascii_sample, 5, font_size);
        assert!(x5 > 0.0, "col 5 must be right of col 0");

        let col_at_x0 = calculate_click_char_col_layout(text_system, ascii_sample, 0.0, font_size);
        assert_eq!(col_at_x0, 0);

        let col_at_x5 = calculate_click_char_col_layout(text_system, ascii_sample, x5 + 0.1, font_size);
        assert_eq!(col_at_x5, 5);

        let end_col = ascii_sample.chars().count();
        let x_end = calculate_cursor_x_offset_layout(text_system, ascii_sample, end_col, font_size);
        assert!(x_end > x5);
        let col_at_end = calculate_click_char_col_layout(text_system, ascii_sample, x_end + 50.0, font_size);
        assert_eq!(col_at_end, end_col);

        // 5. CJK / English mixed layout
        let cjk_sample = "你好，GlideMark 世界";
        let char_count = cjk_sample.chars().count();
        let mut prev_x = 0.0;
        for i in 1..=char_count {
            let cur_x = calculate_cursor_x_offset_layout(text_system, cjk_sample, i, font_size);
            assert!(
                cur_x > prev_x,
                "Cursor offset must strictly increase at char {}: {} > {}",
                i,
                cur_x,
                prev_x
            );
            prev_x = cur_x;
        }

        // Click hit test matches every character position
        for i in 0..char_count {
            let x = calculate_cursor_x_offset_layout(text_system, cjk_sample, i, font_size);
            let hit = calculate_click_char_col_layout(text_system, cjk_sample, x + 0.1, font_size);
            assert_eq!(hit, i, "Click at cursor pos for char {} must resolve back to {}", i, i);
        }

        // 6. Empty string handling
        assert_eq!(calculate_cursor_x_offset_layout(text_system, "", 0, font_size), 0.0);
        assert_eq!(calculate_click_char_col_layout(text_system, "", 50.0, font_size), 0);
        assert_eq!(calculate_click_char_col_layout(text_system, ascii_sample, -10.0, font_size), 0);
    });
}
