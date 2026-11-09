use gpui::TestAppContext;
use zmark_app::state::AppState;
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;

#[gpui::test]
async fn active_file_can_start_rename_from_title_or_sidebar(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note.md");
    std::fs::write(&path, "content").unwrap();
    let (view, cx) = cx.add_window_view(|_, cx| {
        WorkspaceView::new(AppState::new_isolated(Document::from_file(path).unwrap()), cx)
    });

    cx.update(|window, cx| {
        view.update(cx, |ws, cx| ws.begin_rename(window, cx));
    });
    cx.update(|_window, cx| {
        let ws = view.read(cx);
        assert!(ws.is_renaming);
        assert_eq!(ws.rename_buffer, "note.md");
    });
}
