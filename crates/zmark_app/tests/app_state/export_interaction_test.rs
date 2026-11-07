//! HTML export must ask for a destination (including drafts) and report success.

use std::path::{Path, PathBuf};

use gpui::TestAppContext;
use zmark_app::state::{AppState, ViewMode};
use zmark_app::views::workspace::{export_suggested_path, WorkspaceView};
use zmark_core::document::Document;

#[test]
fn suggested_path_prefers_document_directory_and_sanitizes_drafts() {
    let home = PathBuf::from("/home/user");
    let (dir, name) = export_suggested_path(Some(Path::new("/notes/a.md")), "Untitled", &home);
    assert_eq!(dir, PathBuf::from("/notes"));
    assert_eq!(name, "a.html");

    let (dir, name) = export_suggested_path(None, "未命名", &home);
    assert_eq!(dir, home);
    assert_eq!(name, "未命名.html");

    let (_, name) = export_suggested_path(None, "a/b\\c.md", &home);
    assert_eq!(name, "a-b-c.html");
}

#[gpui::test]
async fn export_prompts_and_writes_the_chosen_path(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer("# Hello\n\nExport me.".to_string());
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    let target = std::env::temp_dir().join(format!(
        "zmark_export_{}_{}.html",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_file(&target);

    cx.update(|window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.trigger_export_html(window, cx);
        });
    });
    let expected_target = target.clone();
    cx.simulate_new_path_selection(move |_dir| Some(expected_target.clone()));
    cx.run_until_parked();

    assert!(target.exists(), "the chosen path must be written");
    let notice = cx.update(|_, cx| view.read(cx).export_notice.clone());
    assert!(notice.is_some(), "a success notice must be shown");
    let _ = std::fs::remove_file(&target);
}

#[gpui::test]
async fn export_write_failure_uses_the_error_banner(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer("Fail me.".to_string());
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    let unwritable = PathBuf::from(format!(
        "/zmark_nonexistent_{}/out.html",
        std::process::id()
    ));
    cx.update(|window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.trigger_export_html(window, cx);
        });
    });
    let expected = unwritable.clone();
    cx.simulate_new_path_selection(move |_dir| Some(expected.clone()));
    cx.run_until_parked();

    let (notice, banner) = cx.update(|_, cx| {
        let ws = view.read(cx);
        (ws.export_notice.clone(), ws.error_banner.clone())
    });
    assert!(notice.is_none());
    assert!(banner.is_some(), "write errors must surface in the error banner");
}

#[gpui::test]
async fn export_cancel_writes_nothing(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut doc = Document::new_scratch();
        doc.update_buffer("Cancel me.".to_string());
        let mut state = AppState::new_isolated(doc);
        state.set_view_mode(ViewMode::Split);
        WorkspaceView::new(state, cx)
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        view.update(cx, |workspace, cx| {
            workspace.trigger_export_html(window, cx);
        });
    });
    cx.simulate_new_path_selection(|_dir| None);
    cx.run_until_parked();

    let notice = cx.update(|_, cx| view.read(cx).export_notice.clone());
    assert!(notice.is_none(), "cancel must not show a success notice");
}
