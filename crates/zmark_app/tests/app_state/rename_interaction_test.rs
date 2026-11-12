use gpui::TestAppContext;
use gpui::{EntityInputHandler, Modifiers, MouseButton, MouseDownEvent, VisualTestContext};
use zmark_app::state::{AppState, CloseDecision};
use zmark_app::views::workspace::WorkspaceView;
use zmark_core::document::Document;

#[gpui::test]
async fn active_file_can_start_rename_from_title_or_sidebar(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note.md");
    std::fs::write(&path, "content").unwrap();
    let (view, cx) = cx.add_window_view(|_, cx| {
        WorkspaceView::new(
            AppState::new_isolated(Document::from_file(path).unwrap()),
            cx,
        )
    });

    cx.update(|window, cx| {
        view.update(cx, |ws, cx| ws.begin_rename(window, cx));
    });
    cx.update(|_window, cx| {
        let ws = view.read(cx);
        assert!(ws.is_renaming);
        assert_eq!(ws.rename_input.as_ref().unwrap().read(cx).value().as_ref(), "note.md");
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

#[gpui::test]
async fn native_rename_undo_and_escape_do_not_change_the_document_or_disk(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note.md");
    std::fs::write(&path, "original").unwrap();
    let (view, cx) = cx.add_window_view(|_, cx| {
        WorkspaceView::new(AppState::new_isolated(Document::from_file(path.clone()).unwrap()), cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| ws.begin_rename(window, cx));
        window.draw(cx).clear(cx);
    });
    // A committed IME candidate is one event; simulate_input sends separate keystrokes.
    cx.update(|window, cx| {
        let input = view.read(cx).rename_input.as_ref().unwrap().clone();
        input.update(cx, |input, cx| {
            input.replace_and_mark_text_in_range(None, "weitijiao", None, window, cx);
            input.replace_text_in_range(None, "未提交", window, cx);
        });
    });
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(view.read(cx).rename_input.as_ref().unwrap().read(cx).value().as_ref(), "未提交.md"));
    cx.simulate_keystrokes("secondary-z");
    cx.update(|_, cx| assert_eq!(view.read(cx).rename_input.as_ref().unwrap().read(cx).value().as_ref(), "note.md"));
    cx.simulate_input("cancelled");
    cx.simulate_keystrokes("escape");
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert!(!ws.is_renaming, "Escape cancels, it must not commit or clear the name");
        assert_eq!(ws.state.doc().path.as_ref(), Some(&path));
        assert_eq!(ws.state.editor.to_text(), "original");
        assert_eq!(ws.state.editor.revision, 0);
    });
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
    assert!(!dir.path().join("cancelled.md").exists());
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
    assert_eq!(
        state.doc().display_name_with_fallback("Untitled"),
        "Notes.md"
    );
    assert!(state.doc().path.is_none());
    assert!(matches!(
        state.plan_close(&key),
        Some(CloseDecision::PromptDirty { .. })
    ));
    assert!(state.rename_file("../outside").is_err());
    assert_eq!(
        state.doc().display_name_with_fallback("Untitled"),
        "Notes.md"
    );
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
            window.focus(&view.focus_handle, cx);
            view
        });
        let view = window.root(cx).unwrap();
        let cx = &mut VisualTestContext::from_window(*window, cx);
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
        cx.executor().advance_clock(std::time::Duration::from_millis(220));
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
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

#[gpui::test]
async fn clicking_outside_the_rename_box_leaves_rename_without_changes(cx: &mut TestAppContext) {
    let window = cx.add_window(|window, cx| {
        let view = WorkspaceView::new(AppState::new_isolated(Document::new_scratch()), cx);
        window.focus(&view.focus_handle, cx);
        view
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    cx.update(|window, cx| view.update(cx, |ws, cx| ws.begin_rename(window, cx)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let inside = cx
        .debug_bounds("tab_rename_box")
        .expect("rename box visible")
        .center();
    let mouse_down = |position| MouseDownEvent {
        position,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    };
    cx.simulate_event(mouse_down(inside));
    cx.update(|_, cx| assert!(view.read(cx).is_renaming, "click inside keeps editing"));
    cx.update(|window, cx| {
        let input = view.read(cx).rename_input.as_ref().unwrap().clone();
        input.update(cx, |input, cx| input.set_value("Cancelled.md", window, cx));
    });
    let outside = gpui::point(inside.x + gpui::px(400.), inside.y + gpui::px(300.));
    cx.simulate_event(mouse_down(outside));
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert!(!ws.is_renaming, "click outside ends rename");
        assert_eq!(
            ws.state.doc().display_name_with_fallback("-"),
            "-",
            "unchanged name is not committed"
        );
        assert_eq!(
            ws.state.doc().status,
            zmark_core::document::DocumentStatus::Clean
        );
    });
}

#[gpui::test]
async fn rename_input_accepts_ime_text_caret_moves_and_enter_commits(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("note.md");
    std::fs::write(&path, "content").unwrap();
    let window = cx.add_window(|window, cx| {
        let view = WorkspaceView::new(
            AppState::new_isolated(Document::from_file(path.clone()).unwrap()),
            cx,
        );
        window.focus(&view.focus_handle, cx);
        view
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    cx.update(|window, cx| view.update(cx, |ws, cx| ws.begin_rename(window, cx)));
    cx.run_until_parked();
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });

    // Rename selects the stem, not the suffix; Right collapses it before ".md".
    cx.update(|_, cx| assert_eq!(view.read(cx).rename_input.as_ref().unwrap().read(cx).selected_range(), 0..4));
    cx.simulate_keystrokes("right");
    cx.update(|window, cx| {
        let input = view.read(cx).rename_input.as_ref().unwrap().clone();
        input.update(cx, |i, cx| {
            assert_eq!(
                i.cursor(),
                4,
                "arrow keys move the caret inside the name"
            );
            i.replace_and_mark_text_in_range(None, "zhongwen", None, window, cx);
            i.replace_text_in_range(None, "中文", window, cx);
        });
        assert_eq!(input.read(cx).value().as_ref(), "note中文.md");
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert!(!ws.is_renaming, "enter leaves rename");
        assert_eq!(
            ws.state.doc().display_name_with_fallback("-"),
            "note中文.md"
        );
    });
    assert!(dir.path().join("note中文.md").exists());
}

#[gpui::test]
async fn folder_name_selects_and_renames_in_the_sidebar_without_switching_document(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    std::fs::create_dir_all(root.join("nested")).unwrap();
    std::fs::write(root.join("nested/note.md"), "original").unwrap();
    let window = cx.add_window(|window, cx| {
        let mut state = AppState::new_isolated(Document::new_scratch());
        state.add_workspace_root(root).unwrap();
        state.drawer_open = true;
        let view = WorkspaceView::new(state, cx);
        window.focus(&view.focus_handle, cx);
        view
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.executor().advance_clock(std::time::Duration::from_millis(210));
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let click = |position, click_count| MouseDownEvent {
        position,
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count,
        first_mouse: false,
    };
    let name = cx.debug_bounds("drawer_root_name_0").unwrap().center();
    cx.simulate_event(click(name, 1));
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert!(
            !ws.state.workspace_roots[0].is_expanded,
            "name must not toggle"
        );
        assert!(ws.selected_directory.is_some());
    });
    cx.simulate_event(click(name, 2));
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    assert!(cx.debug_bounds("sidebar_rename_box").is_some());
    assert!(cx.debug_bounds("tab_rename_box").is_none());
    cx.update(|window, cx| {
        let ws = view.read(cx);
        assert!(ws.state.active_key().unwrap().is_scratch());
        let input = ws.rename_input.as_ref().unwrap().clone();
        input.update(cx, |input, cx| input.replace_text_in_range(None, "新目录", window, cx));
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert!(!ws.is_renaming);
        assert!(ws.state.active_key().unwrap().is_scratch());
        assert_eq!(
            ws.state.workspace_roots[0].path.file_name().unwrap(),
            "新目录"
        );
        assert_eq!(
            std::fs::read_to_string(ws.state.workspace_roots[0].path.join("nested/note.md"))
                .unwrap(),
            "original"
        );
    });
}

#[gpui::test]
async fn folder_transition_reverses_without_a_jump_and_collapsed_children_are_inert(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("note.md"), "original").unwrap();
    let window = cx.add_window(|window, cx| {
        let mut state = AppState::new_isolated(Document::new_scratch());
        state.add_workspace_root(dir.path().to_path_buf()).unwrap();
        state.drawer_open = true;
        let view = WorkspaceView::new(state, cx);
        window.focus(&view.focus_handle, cx);
        view
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.executor().advance_clock(std::time::Duration::from_millis(210));
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let root = cx.update(|_, cx| view.read(cx).state.workspace_roots[0].path.clone());
    cx.update(|_, cx| {
        view.update(cx, |ws, cx| {
            ws.toggle_folder(0, &root, cx);
            ws.folder_transitions.get_mut(&root).unwrap().started -=
                std::time::Duration::from_millis(300);
            let current = ws.folder_transitions[&root].value(std::time::Instant::now());
            ws.toggle_folder(0, &root, cx);
            assert!((ws.folder_transitions[&root].from - current).abs() < 0.01);
        })
    });
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    let selector: &'static str =
        Box::leak(format!("drawer_file:{}", root.join("note.md").display()).into_boxed_str());
    let child = cx.debug_bounds(selector).unwrap().center();
    let fading_height = cx.debug_bounds("drawer_roots_section").unwrap().size.height;
    cx.simulate_click(child, Modifiers::none());
    cx.update(|_, cx| {
        assert!(
            view.read(cx).state.active_key().unwrap().is_scratch(),
            "fading children are not clickable"
        )
    });
    cx.update(|_, cx| view.update(cx, |ws, _| ws.state.preferences.reduce_motion = true));
    cx.update(|window, cx| {
        let _ = window.draw(cx);
    });
    cx.update(|_, cx| assert!(view.read(cx).folder_transitions.is_empty()));
    // GPUI keeps old debug entries; current container geometry proves removal.
    assert!(
        cx.debug_bounds("drawer_roots_section").unwrap().size.height
            < fading_height - gpui::px(30.)
    );
}
