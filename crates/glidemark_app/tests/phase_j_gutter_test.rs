use glidemark_app::state::{AppState, ViewMode};
use glidemark_app::views::editor::editor_gutter_width;
use glidemark_app::views::workspace::WorkspaceView;
use glidemark_core::document::Document;
use gpui::{px, TestAppContext};

#[gpui::test]
async fn gutter_fits_large_numbers_and_title_aligns_in_open_drawer(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer("alpha beta\n".into());
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Source);
        state.drawer_open = true;
        WorkspaceView::new(state, cx)
    });
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let title = cx
        .debug_bounds("editor_document_title")
        .expect("title rendered");
    let text = cx
        .debug_bounds("editor_text_0")
        .expect("source text rendered");
    assert!(
        (f32::from(title.origin.x) - f32::from(text.origin.x)).abs() < 1.0,
        "title {} and source {} must align",
        title.origin.x,
        text.origin.x
    );
    cx.update(|window, cx| {
        let size = view.read(cx).state.preferences.font_size;
        let mut previous = 0.0;
        for count in [999, 1000, 10_000, 100_000] {
            let width = editor_gutter_width(window.text_system(), count, size);
            assert!(width > previous);
            let digits = count.to_string();
            let run = gpui::TextRun {
                len: digits.len(),
                font: gpui::font("ui-monospace"),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let actual = window
                .text_system()
                .shape_line(digits.into(), px(size * 0.85), &[run], None)
                .width;
            assert!(width >= f32::from(actual), "{count} must fit");
            previous = width;
        }
    });
}
