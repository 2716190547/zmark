//! A downloaded update must surface a restart banner exactly once per version.

use gpui::TestAppContext;
use zmark_app::state::{AppState, ViewMode};
use zmark_app::updater::UpdateStatus;
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;

#[gpui::test]
async fn downloaded_update_shows_and_dismisses_the_restart_banner(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer("# Update".to_string());
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Preview);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.state.preferences.reduce_motion = true;
            workspace.update_status = UpdateStatus::Downloaded("9.9.9".to_string());
            cx.notify();
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("update_ready_banner").is_some(),
        "a downloaded update must render the restart banner"
    );

    cx.update(|window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.dismiss_update_banner("9.9.9", cx);
        });
        let _ = window.draw(cx);
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("update_ready_banner").is_none(),
        "Later must hide the banner for this session"
    );
}
