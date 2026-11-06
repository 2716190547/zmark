use glidemark_app::state::{AppState, ViewMode};
use glidemark_app::theme::{ThemeColors, ALL_PALETTES};
use glidemark_app::views::workspace::{calculate_title_header_padding, WorkspaceView};
use glidemark_core::i18n::{get_strings, SupportedLocale};
use gpui::TestAppContext;

#[test]
fn test_h3_title_header_padding_matrix() {
    let viewport_w = 1200.0;
    let col_w = 680.0;

    // 1. Collapsed drawer on macOS preserves 76.0px traffic light safe area
    let pad_collapsed = calculate_title_header_padding(true, false, false, ViewMode::Source, viewport_w, col_w, 54.0);
    assert_eq!(pad_collapsed, 76.0);

    // 2. Open drawer in Source & Split mode aligns title text start with measured text start
    // (padding follows the measured text start)
    let pad_source = calculate_title_header_padding(true, false, true, ViewMode::Source, viewport_w, col_w, 54.0);
    assert_eq!(pad_source, 54.0 - 8.0);
    assert_eq!(pad_source + 8.0, 54.0);

    let pad_split = calculate_title_header_padding(true, false, true, ViewMode::Split, viewport_w, col_w, 54.0);
    assert_eq!(pad_split, 54.0 - 8.0);

    // 3. Open drawer in Preview mode aligns title with centered reading column
    // available_w = 1200 - 250 = 950
    // left_margin = (950 - 680) * 0.5 = 135.0
    // padding = 135.0 - 8.0 = 127.0
    let pad_preview = calculate_title_header_padding(true, false, true, ViewMode::Preview, viewport_w, col_w, 54.0);
    assert_eq!(pad_preview, 127.0);

    // 4. Narrow window in Preview mode clamps to minimum 32px column margin (32 - 8 = 24.0px padding)
    let pad_narrow_preview = calculate_title_header_padding(true, false, true, ViewMode::Preview, 700.0, col_w, 54.0);
    assert_eq!(pad_narrow_preview, 24.0);
}

#[test]
fn test_h3_selection_tokens_and_status_labels() {
    // 1. Verify default and claude palettes have warm selection background
    for palette in ALL_PALETTES {
        for &is_dark in &[false, true] {
            let theme = ThemeColors::resolve(palette.id, is_dark);
            let sel_bg = theme.selection_bg();
            if palette.id == "default" || palette.id == "claude" {
                let hue_deg = sel_bg.h * 360.0;
                assert!(
                    hue_deg >= 25.0 && hue_deg <= 45.0,
                    "Palette {} (dark={}) selection_bg hue {}° not in warm [25°, 45°]",
                    palette.id, is_dark, hue_deg
                );
                assert!(
                    sel_bg.s >= 0.30,
                    "Palette {} (dark={}) selection_bg saturation {} below 0.30",
                    palette.id, is_dark, sel_bg.s
                );
                assert!(
                    sel_bg.a >= 0.10,
                    "Palette {} (dark={}) selection_bg alpha {} below 0.10",
                    palette.id, is_dark, sel_bg.a
                );
            } else {
                assert_eq!(sel_bg, theme.text_primary.opacity(0.07));
            }
        }
    }

    // 2. Verify status bar labels in i18n
    let zh_cn = get_strings(SupportedLocale::ZhCn);
    assert_eq!(zh_cn.status_line, "行");
    assert_eq!(zh_cn.status_chars, "字");

    let en = get_strings(SupportedLocale::En);
    assert_eq!(en.status_line, "Ln");
    assert_eq!(en.status_chars, "chars");
}

#[gpui::test]
async fn test_workspace_search_and_selection_integration(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_window, cx| {
        let state = AppState::new(glidemark_core::document::Document::new_scratch());
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    cx.update(|_window, app| {
        view.update(app, |workspace, cx| {
            // Initial state: search is not focused, no preview selection, search input exists
            assert!(!workspace.search_focused);
            assert!(workspace.preview_selection.is_none());
            assert!(workspace.search_input.is_some());

            // Update search query via Workspace async trigger
            workspace.update_search_query("test", cx);
            assert_eq!(workspace.state.search_query, "test");
            assert!(workspace.state.search_in_progress);

            // Clear search
            workspace.state.clear_search();
            assert_eq!(workspace.state.search_query, "");
            assert!(!workspace.state.search_in_progress);
        });
    });
}
