use zmark_app::state::{AppState, DocKey};
use zmark_core::document::Document;
use std::fs;

#[test]
fn disk_checks_preserve_dirty_text_and_reload_only_clean_active_documents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note.md");
    fs::write(&path, "original").unwrap();
    let mut state = AppState::new_isolated(Document::from_file(path.clone()).unwrap());
    state.editor.insert_text(" local");
    state.mark_editor_dirty();
    fs::write(&path, "external").unwrap();
    assert!(!state.check_disk_changes().unwrap());
    assert!(state.has_conflict());
    assert_eq!(state.editor.to_text(), "original local");
    assert!(state.editor.can_undo());
    state.reload_active_document().unwrap();
    assert_eq!(state.editor.to_text(), "external");
    assert!(!state.is_dirty());
    fs::write(&path, "new external").unwrap();
    assert!(state.check_disk_changes().unwrap());
    assert_eq!(state.editor.to_text(), "new external");
    assert!(!state.is_dirty());
}

#[gpui::test]
fn manual_and_automatic_save_failures_keep_text_and_show_an_error(cx: &mut gpui::TestAppContext) {
    use zmark_app::views::workspace::WorkspaceView;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note.md");
    fs::write(&path, "original").unwrap();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut state = AppState::new_isolated(Document::from_file(path.clone()).unwrap());
        state.preferences.reduce_motion = true;
        state.preferences.auto_save = false;
        state.preferences.auto_save_on_blur = false;
        WorkspaceView::new(state, cx)
    });
    cx.update(|_, cx| view.update(cx, |ws, _| {
        ws.state.editor.insert_text(" local");
        ws.state.mark_editor_dirty();
    }));
    fs::remove_file(&path).unwrap();
    cx.update(|_, cx| view.update(cx, |ws, cx| ws.trigger_save(cx)));
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert!(ws.error_banner.is_some());
        assert!(ws.state.is_dirty());
        assert_eq!(ws.state.editor.to_text(), "original local");
    });
    cx.update(|_, cx| view.update(cx, |ws, cx| {
        ws.error_banner = None;
        ws.state.preferences.auto_save = true;
        ws.trigger_auto_save(cx);
    }));
    cx.executor().advance_clock(std::time::Duration::from_millis(1100));
    cx.run_until_parked();
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert!(ws.error_banner.is_some());
        assert!(ws.state.is_dirty());
        assert_eq!(ws.state.editor.to_text(), "original local");
        assert!(!path.exists());
    });
}

#[gpui::test]
fn reactivation_detects_disk_changes_without_overwriting_editor_input(cx: &mut gpui::TestAppContext) {
    use zmark_app::views::workspace::WorkspaceView;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note.md");
    fs::write(&path, "original").unwrap();
    let (view, cx) = cx.add_window_view(|window, cx| {
        let mut state = AppState::new_isolated(Document::from_file(path.clone()).unwrap());
        state.preferences.auto_save = false;
        state.preferences.auto_save_on_blur = false;
        let mut ws = WorkspaceView::new(state, cx);
        ws.install_window_activation_observer(window, cx);
        ws
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|window, _| assert!(window.is_window_active()));
    cx.deactivate_window();
    cx.update(|window, _| assert!(!window.is_window_active()));
    cx.update(|_, cx| view.update(cx, |ws, _| {
        ws.state.editor.insert_text(" local");
        ws.state.mark_editor_dirty();
    }));
    fs::write(&path, "external").unwrap();
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert!(view.read(cx).state.has_conflict());
        assert_eq!(view.read(cx).state.editor.to_text(), "original local");
        assert_eq!(fs::read_to_string(&path).unwrap(), "external");
    });
}

#[gpui::test]
fn save_as_dialog_keeps_its_original_document_when_another_file_is_activated(cx: &mut gpui::TestAppContext) {
    use zmark_app::views::workspace::WorkspaceView;
    let dir = tempfile::tempdir().unwrap();
    let other = dir.path().join("other.md");
    let target = dir.path().join("draft.md");
    fs::write(&other, "other document").unwrap();
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut state = AppState::new_isolated(Document::new_scratch());
        state.preferences.auto_save = false;
        state.preferences.auto_save_on_blur = false;
        state.editor.insert_text("original draft");
        WorkspaceView::new(state, cx)
    });
    cx.update(|_, cx| view.update(cx, |ws, cx| ws.trigger_save(cx)));
    cx.update(|_, cx| view.update(cx, |ws, _| ws.state.open_file(other.clone()).unwrap()));
    let selected = target.clone();
    cx.simulate_new_path_selection(move |_| Some(selected.clone()));
    cx.run_until_parked();
    assert_eq!(fs::read_to_string(&target).unwrap(), "original draft");
    assert_eq!(fs::read_to_string(&other).unwrap(), "other document");
    cx.update(|_, cx| assert_eq!(view.read(cx).state.doc().path, Some(other.canonicalize().unwrap())));
}

#[gpui::test]
fn closing_a_draft_after_save_as_removes_the_new_document_key(cx: &mut gpui::TestAppContext) {
    use zmark_app::views::workspace::WorkspaceView;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("draft.md");
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut state = AppState::new_isolated(Document::new_scratch_with_text("draft content".into()));
        state.preferences.auto_save = false;
        state.preferences.auto_save_on_blur = false;
        WorkspaceView::new(state, cx)
    });
    cx.update(|_, cx| view.update(cx, |ws, cx| {
        let key = ws.state.active_key().cloned().unwrap();
        ws.prompt_save_as_and_close(key, cx);
    }));
    let selected = target.clone();
    cx.simulate_new_path_selection(move |_| Some(selected.clone()));
    cx.run_until_parked();
    assert_eq!(fs::read_to_string(&target).unwrap(), "draft content");
    cx.update(|_, cx| assert!(view.read(cx).state.open_documents().is_empty()));
}

#[test]
fn concurrent_new_files_never_overwrite_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let threads: Vec<_> = (0..4).map(|index| {
        let path = dir.path().to_path_buf();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            let mut state = AppState::new_isolated(Document::new_scratch());
            barrier.wait();
            let created = state.create_file_in_dir(&path).unwrap();
            fs::write(&created, format!("writer {index}")).unwrap();
            (created, format!("writer {index}"))
        })
    }).collect();
    let files: Vec<_> = threads.into_iter().map(|thread| thread.join().unwrap()).collect();
    for (path, expected) in &files { assert_eq!(&fs::read_to_string(path).unwrap(), expected); }
    let unique: std::collections::HashSet<_> = files.iter().map(|(path, _)| path).collect();
    assert_eq!(unique.len(), 4);
}

#[test]
fn closing_last_document_clears_active_editor_and_allows_new_draft() {
    let mut state = AppState::new_isolated(Document::new_scratch());
    let original = state.active_key().cloned().unwrap();
    state.editor.insert_text("discarded draft");
    state.sync_from_editor();
    state.close_document_forced(&original);
    assert!(state.active_key().cloned().is_none());
    assert!(state.open_documents().is_empty());
    assert_eq!(state.editor.to_text(), "");
    assert!(!state.is_dirty());
    assert!(!state.has_conflict());
    state.focus_or_create_draft();
    assert!(state.active_key().unwrap().is_scratch());
    assert_eq!(state.editor.to_text(), "");
}

#[test]
fn failed_save_keeps_local_undo_and_content_across_switches() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("removed");
    fs::create_dir(&folder).unwrap();
    let first = folder.join("first.md");
    let second = dir.path().join("second.md");
    fs::write(&first, "original").unwrap();
    fs::write(&second, "second").unwrap();
    let mut state = AppState::new_isolated(Document::from_file(first.clone()).unwrap());
    state.editor.insert_text(" local");
    state.sync_from_editor();
    let edited = state.editor.to_text();
    fs::remove_file(&first).unwrap();
    fs::remove_dir(&folder).unwrap();
    assert!(state.save().is_err());
    assert!(state.is_dirty());
    assert_eq!(state.editor.to_text(), edited);
    assert!(state.editor.undo());
    state.sync_from_editor();
    assert_eq!(state.editor.to_text(), "original");
    state.editor.insert_text(" retained");
    state.sync_from_editor();
    let retained = state.editor.to_text();
    state.open_file(second).unwrap();
    state.switch_document(&DocKey::Path(first));
    assert_eq!(state.editor.to_text(), retained);
    assert!(state.is_dirty());
}
