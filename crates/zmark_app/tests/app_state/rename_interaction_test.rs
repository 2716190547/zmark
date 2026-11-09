use gpui::TestAppContext;
use zmark_app::state::{AppState, CloseDecision};
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;
use gpui::{Modifiers, MouseButton, MouseDownEvent, VisualTestContext};

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

#[gpui::test]
async fn unsaved_draft_can_start_inline_rename(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| {
        WorkspaceView::new(AppState::new_isolated(Document::new_scratch()), cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| ws.begin_rename(window, cx));
    });
    cx.update(|_, cx| assert!(view.read(cx).is_renaming));
}

#[test]
fn named_draft_stays_unsaved_and_prompts_on_close() {
    let mut draft = Document::new_scratch();
    draft.rename_scratch("Notes").unwrap();
    draft.set_buffer("text".into());
    draft.set_buffer(String::new());
    assert_eq!(draft.status, zmark_core::document::DocumentStatus::Modified);
    let mut state = AppState::new_isolated(Document::new_scratch());
    let key = state.active_key().cloned().unwrap();
    state.rename_file("Notes").unwrap();
    assert_eq!(state.doc().display_name_with_fallback("Untitled"), "Notes.md");
    assert!(state.doc().path.is_none());
    assert!(matches!(state.plan_close(&key), Some(CloseDecision::PromptDirty { .. })));
    assert!(state.rename_file("../outside").is_err());
    assert_eq!(state.doc().display_name_with_fallback("Untitled"), "Notes.md");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Notes.md");
    state.save_doc_as(&key, path.clone()).unwrap();
    assert!(path.exists());
    assert_eq!(state.doc().path.as_ref(), Some(&path));
}

#[gpui::test]
async fn double_click_draft_title_or_sidebar_starts_rename(cx: &mut TestAppContext) {
    for target in ["editor_document_title", "drawer_draft_item"] {
        let window = cx.add_window(|window, cx| {
            let mut state = AppState::new_isolated(Document::new_scratch());
            state.drawer_open = true;
            let view = WorkspaceView::new(state, cx);
            window.focus(&view.focus_handle);
            view
        });
        let view = window.root(cx).unwrap();
        let cx = &mut VisualTestContext::from_window(*window, cx);
        cx.run_until_parked();
        cx.update(|window, cx| { let _ = window.draw(cx); });
        std::thread::sleep(std::time::Duration::from_millis(220));
        cx.update(|window, cx| { let _ = window.draw(cx); });
        let bounds = cx.debug_bounds(target).expect("draft target is visible");
        cx.simulate_event(MouseDownEvent {
            position: bounds.center(),
            button: MouseButton::Left,
            modifiers: Modifiers::none(),
            click_count: 2,
            first_mouse: false,
        });
        cx.update(|_, cx| assert!(view.read(cx).is_renaming, "{target}"));
    }
}
