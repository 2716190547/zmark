use zmark_app::editor_engine::EditorEngine;

#[test]
fn source_replacement_is_atomic_undoable_and_moves_cursor_by_characters() {
    let source = "前缀 ![x](D:\\old.png) 尾缀\r\n第二行";
    let start = source.find("D:").unwrap();
    let end = start + "D:\\old.png".len();
    let range = start..end;
    for (col, expected) in [(1, 1), (9, 25), (21, 28)] {
        let mut editor = EditorEngine::new(source);
        editor.set_cursor(0, col);
        assert!(editor.replace_source_range(range.clone(), "D:\\old.png", "assets/图片%20x.png"));
        assert_eq!(editor.cursor().1, expected);
        assert!(editor
            .to_text()
            .starts_with("前缀 ![x](assets/图片%20x.png) 尾缀\r\n"));
        assert!(editor.undo());
        assert_eq!(editor.to_text(), source);
        assert!(editor.redo());
        assert!(editor.to_text().contains("assets/图片%20x.png"));
    }
}

#[test]
fn stale_or_oversized_replacements_are_rejected_without_touching_history() {
    let mut editor = EditorEngine::new("before");
    editor.insert_text("!");
    let before = editor.to_text();
    assert!(!editor.replace_source_range(0..6, "stale!", "new"));
    assert_eq!(editor.to_text(), before);
    assert!(editor.undo());
    assert_eq!(editor.to_text(), "before");

    let limit = 8 * 1024 * 1024;
    let before_limit = "a".repeat(limit);
    let mut editor = EditorEngine::new(&before_limit);
    assert!(!editor.replace_source_range(0..1, "a", "aa"));
    assert_eq!(editor.to_text(), before_limit);
    assert!(!editor.undo());
    let mut editor = EditorEngine::new(&"a".repeat(limit + 1));
    assert!(!editor.replace_source_range(0..2, "aa", "a"));
    assert_eq!(editor.to_text().len(), limit + 1);
}

#[test]
fn candidates_are_bounded_and_stale_files_are_rejected() {
    use zmark_app::local_images::{
        image_recovery_candidates, markdown_relative_image_path, validate_image_recovery,
    };
    let dir = tempfile::tempdir().unwrap();
    let document = dir.path().join("note.md");
    std::fs::write(&document, "").unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    let filename = "图片 #?%(a).png";
    std::fs::write(dir.path().join("assets").join(filename), b"image").unwrap();
    let url = "D:\\old\\%E5%9B%BE%E7%89%87%20%23%3F%25(a).png".to_string();
    let candidates = image_recovery_candidates(&url, Some(&document));

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        markdown_relative_image_path(&candidates[0].relative).unwrap(),
        "assets/%E5%9B%BE%E7%89%87%20%23%3F%25%28a%29.png"
    );
    assert!(validate_image_recovery(
        &url,
        Some(&document),
        &candidates[0]
    ));
    std::fs::write(dir.path().join(filename), b"other").unwrap();
    assert_eq!(image_recovery_candidates(&url, Some(&document)).len(), 2);
    std::fs::write(dir.path().join("assets").join(filename), b"changed size").unwrap();
    assert!(!validate_image_recovery(
        &url,
        Some(&document),
        &candidates[0]
    ));
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("link.png"), b"secret").unwrap();
        std::os::unix::fs::symlink(outside.path().join("link.png"), dir.path().join("link.png"))
            .unwrap();
        assert!(image_recovery_candidates("D:\\old\\link.png", Some(&document)).is_empty());
    }
    assert!(image_recovery_candidates("https://example.com/x", Some(&document)).is_empty());
}

#[gpui::test]
async fn clicking_recovery_changes_only_selected_reference_and_can_undo(
    cx: &mut gpui::TestAppContext,
) {
    use zmark_app::state::{AppState, ViewMode};
    use zmark_app::views::workspace::WorkspaceView;
    use zmark_core::document::Document;
    use gpui::{Modifiers, MouseButton, VisualTestContext};
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    std::fs::write(dir.path().join("assets/pic.png"), b"fixture").unwrap();
    let source = "![first](D:\\old\\pic.png)\n\n![second](D:\\old\\pic.png)";
    let document = dir.path().join("note.md");
    std::fs::write(&document, source).unwrap();
    let window = cx.add_window(|window, cx| {
        let mut state = AppState::new_isolated(Document::from_file(document.clone()).unwrap());
        state.set_view_mode(ViewMode::Preview);
        let ws = WorkspaceView::new(state, cx);
        window.focus(&ws.focus_handle);
        ws
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    for _ in 0..8 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    let first = cx
        .debug_bounds("recover_image:0:assets/pic.png")
        .expect("recovery button appears");
    cx.simulate_mouse_down(first.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(first.center(), MouseButton::Left, Modifiers::none());
    for _ in 0..8 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    cx.update(|_, cx| {
        view.update(cx, |ws, _| {
            assert_eq!(
                ws.state.editor.to_text(),
                "![first](assets/pic.png)\n\n![second](D:\\old\\pic.png)"
            );
            assert!(ws.state.editor.undo());
            assert_eq!(ws.state.editor.to_text(), source);
        })
    });
}

#[gpui::test]
async fn entity_like_filename_round_trips_through_preview(cx: &mut gpui::TestAppContext) {
    use zmark_app::state::{AppState, ViewMode};
    use zmark_app::views::workspace::WorkspaceView;
    use zmark_core::document::Document;
    use gpui::{Modifiers, MouseButton, VisualTestContext};
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    let image_path = dir.path().join("assets/a&amp;b.png");
    image::RgbaImage::from_pixel(1, 1, image::Rgba([10, 20, 30, 255]))
        .save(&image_path)
        .unwrap();
    let document = dir.path().join("note.md");
    std::fs::write(&document, "![a](D:\\old\\a%26amp%3Bb.png)").unwrap();
    let window = cx.add_window(|window, cx| {
        let mut state = AppState::new_isolated(Document::from_file(document.clone()).unwrap());
        state.set_view_mode(ViewMode::Preview);
        let ws = WorkspaceView::new(state, cx);
        window.focus(&ws.focus_handle);
        ws
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    for _ in 0..8 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    let button = cx
        .debug_bounds("recover_image:0:assets/a&amp;b.png")
        .expect("candidate button");
    cx.simulate_mouse_down(button.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(button.center(), MouseButton::Left, Modifiers::none());
    for _ in 0..8 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    cx.update(|_, cx| {
        view.update(cx, |ws, cx| {
            assert_eq!(ws.state.editor.to_text(), "![a](assets/a%26amp;b.png)");
            ws.parse_now_and_refresh();
            cx.notify();
        })
    });
    for _ in 0..8 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    assert!(cx
        .debug_bounds("local_image:assets/a%26amp;b.png")
        .is_some());
    let resolved =
        zmark_app::local_images::resolve_image_path("assets/a%26amp;b.png", Some(&document))
            .unwrap();
    assert_eq!(
        resolved.canonicalize().unwrap(),
        image_path.canonicalize().unwrap()
    );
}

#[gpui::test]
async fn recovery_undo_redo_shortcuts_follow_autosave_to_disk(cx: &mut gpui::TestAppContext) {
    use zmark_app::state::{AppState, ViewMode};
    use zmark_app::views::workspace::WorkspaceView;
    use zmark_core::document::Document;
    use gpui::{Modifiers, MouseButton, VisualTestContext};
    use std::time::Duration;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    std::fs::write(dir.path().join("assets/pic.png"), b"fixture").unwrap();
    let document = dir.path().join("note.md");
    let original = "![x](D:\\old\\pic.png)";
    std::fs::write(&document, original).unwrap();
    let window = cx.add_window(|window, cx| {
        let mut state = AppState::new_isolated(Document::from_file(document.clone()).unwrap());
        state.preferences.auto_save = true;
        state.preferences.auto_save_delay_ms = 20;
        state.set_view_mode(ViewMode::Preview);
        let ws = WorkspaceView::new(state, cx);
        window.focus(&ws.focus_handle);
        ws
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    for _ in 0..8 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    let button = cx.debug_bounds("recover_image:0:assets/pic.png").unwrap();
    cx.simulate_mouse_down(button.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(button.center(), MouseButton::Left, Modifiers::none());
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(&document).unwrap(),
        "![x](assets/pic.png)"
    );
    cx.simulate_keystrokes("secondary-z");
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(view.read(cx).state.editor.to_text(), original));
    assert_eq!(std::fs::read_to_string(&document).unwrap(), original);
    cx.simulate_keystrokes("cmd-shift-z");
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(&document).unwrap(),
        "![x](assets/pic.png)"
    );
}

#[gpui::test]
async fn pending_autosaves_survive_switching_and_editing_another_document(
    cx: &mut gpui::TestAppContext,
) {
    use zmark_app::state::{AppState, ViewMode};
    use zmark_app::views::workspace::WorkspaceView;
    use zmark_core::document::Document;
    use gpui::{Modifiers, MouseButton, VisualTestContext};
    use std::time::Duration;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    std::fs::write(dir.path().join("assets/pic.png"), b"fixture").unwrap();
    let first = dir.path().join("first.md");
    let second = dir.path().join("second.md");
    let original = "![x](D:\\old\\pic.png)";
    let second_original = "Second document";
    std::fs::write(&first, original).unwrap();
    std::fs::write(&second, second_original).unwrap();
    let window = cx.add_window(|window, cx| {
        let mut state = AppState::new_isolated(Document::from_file(first.clone()).unwrap());
        state.preferences.auto_save = true;
        state.preferences.auto_save_delay_ms = 100;
        state.set_view_mode(ViewMode::Preview);
        let ws = WorkspaceView::new(state, cx);
        window.focus(&ws.focus_handle);
        ws
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    for _ in 0..8 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    let button = cx.debug_bounds("recover_image:0:assets/pic.png").unwrap();
    cx.simulate_mouse_down(button.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(button.center(), MouseButton::Left, Modifiers::none());
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.state.load_file(second.clone()).unwrap();
            ws.on_document_swapped(Some(window), cx);
        })
    });
    cx.simulate_keystrokes("end space c h a n g e d");
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(&first).unwrap(),
        "![x](assets/pic.png)"
    );
    assert_eq!(
        std::fs::read_to_string(&second).unwrap(),
        "Second document changed"
    );
}

#[gpui::test]
async fn pending_autosave_follows_renamed_document_key(cx: &mut gpui::TestAppContext) {
    use zmark_app::state::{AppState, ViewMode};
    use zmark_app::views::workspace::WorkspaceView;
    use zmark_core::document::Document;
    use gpui::{Modifiers, MouseButton, VisualTestContext};
    use std::time::Duration;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    std::fs::write(dir.path().join("assets/pic.png"), b"fixture").unwrap();
    let document = dir.path().join("before.md");
    let renamed = dir.path().join("after.md");
    std::fs::write(&document, "![x](D:\\old\\pic.png)").unwrap();
    let window = cx.add_window(|window, cx| {
        let mut state = AppState::new_isolated(Document::from_file(document.clone()).unwrap());
        state.preferences.auto_save = true;
        state.preferences.auto_save_delay_ms = 100;
        state.set_view_mode(ViewMode::Preview);
        let ws = WorkspaceView::new(state, cx);
        window.focus(&ws.focus_handle);
        ws
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    for _ in 0..8 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    let button = cx.debug_bounds("recover_image:0:assets/pic.png").unwrap();
    cx.simulate_mouse_down(button.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(button.center(), MouseButton::Left, Modifiers::none());
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.begin_rename(window, cx);
            ws.rename_input.update(cx, |input, _| input.set_text("after"));
        })
    });
    cx.simulate_keystrokes("enter");
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.run_until_parked();
    assert!(!document.exists());
    assert_eq!(
        std::fs::read_to_string(&renamed).unwrap(),
        "![x](assets/pic.png)"
    );
}

#[gpui::test]
async fn background_tasks_do_not_retain_workspace(cx: &mut gpui::TestAppContext) {
    use zmark_app::state::{AppState, ViewMode};
    use zmark_app::views::workspace::WorkspaceView;
    use zmark_core::document::Document;
    use gpui::{point, px, size, AppContext, Bounds};
    let mut state = AppState::new_isolated(Document::new_scratch());
    state.preferences.auto_save = true;
    state.preferences.auto_save_delay_ms = 60_000;
    state.set_view_mode(ViewMode::Preview);
    let view = cx.new(|cx| WorkspaceView::new(state, cx));
    let weak = view.downgrade();
    cx.update(|cx| {
        view.update(cx, |ws, cx| {
            ws.trigger_auto_save(cx);
            ws.is_preview_selecting = true;
            ws.preview_mouse_window_pos = Some(point(px(100.), px(499.)));
            ws.preview_content_bounds =
                Some(Bounds::new(point(px(0.), px(0.)), size(px(500.), px(500.))));
            ws.trigger_preview_autoscroll_if_needed(cx);
            assert!(ws.preview_autoscroll_task.is_some());
        })
    });
    drop(view);
    cx.run_until_parked();
    assert!(
        weak.update(cx, |_, _| ()).is_err(),
        "a pending timer must not retain its workspace"
    );
}

#[gpui::test]
async fn switching_documents_warms_only_their_fenced_languages_without_edit(
    cx: &mut gpui::TestAppContext,
) {
    use zmark_app::{
        state::{AppState, ViewMode},
        syntax::WarmState, views::workspace::WorkspaceView,
    };
    use zmark_core::document::Document;
    let dir = tempfile::tempdir().unwrap();
    let rust = dir.path().join("rust.md");
    let python = dir.path().join("python.md");
    std::fs::write(&rust, "```rust\nfn main() {}\n```").unwrap();
    std::fs::write(&python, "```python\ndef main(): pass\n```").unwrap();
    let window = cx.add_window(|window, cx| {
        let mut state = AppState::new_isolated(Document::from_file(rust.clone()).unwrap());
        state.set_view_mode(ViewMode::Preview);
        let view = WorkspaceView::new(state, cx);
        window.focus(&view.focus_handle);
        view
    });
    let view = window.root(cx).unwrap();
    let cx = &mut gpui::VisualTestContext::from_window(*window, cx);
    for _ in 0..6 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert_eq!(ws.syntax_warm_states.get("Rust"), Some(&WarmState::Ready));
        assert_eq!(
            ws.syntax_warm_states.get("Python"),
            Some(&WarmState::Waiting)
        );
        assert_eq!(ws.state.editor.revision, 0);
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.state.load_file(python.clone()).unwrap();
            ws.on_document_swapped(Some(window), cx);
        })
    });
    for _ in 0..6 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    cx.update(|_, cx| {
        let ws = view.read(cx);
        assert_eq!(ws.syntax_warm_states.get("Python"), Some(&WarmState::Ready));
        assert_eq!(
            ws.state.editor.to_text(),
            "```python\ndef main(): pass\n```"
        );
    });
}

#[gpui::test]
async fn recovery_respects_manual_save_preference(cx: &mut gpui::TestAppContext) {
    use zmark_app::state::{AppState, ViewMode};
    use zmark_app::views::workspace::WorkspaceView;
    use zmark_core::document::Document;
    use gpui::{Modifiers, MouseButton, VisualTestContext};
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    std::fs::write(dir.path().join("assets/pic.png"), b"fixture").unwrap();
    let document = dir.path().join("note.md");
    let original = "![x](D:\\old\\pic.png)";
    std::fs::write(&document, original).unwrap();
    let window = cx.add_window(|window, cx| {
        let mut state = AppState::new_isolated(Document::from_file(document.clone()).unwrap());
        state.preferences.auto_save = false;
        state.set_view_mode(ViewMode::Preview);
        let ws = WorkspaceView::new(state, cx);
        window.focus(&ws.focus_handle);
        ws
    });
    let view = window.root(cx).unwrap();
    let cx = &mut VisualTestContext::from_window(*window, cx);
    for _ in 0..8 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            let _ = window.draw(cx);
        });
    }
    let button = cx.debug_bounds("recover_image:0:assets/pic.png").unwrap();
    cx.simulate_mouse_down(button.center(), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(button.center(), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(2));
    cx.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(&document).unwrap(),
        original,
        "manual save preference must keep recovery in memory"
    );
    cx.simulate_keystrokes("cmd-s");
    cx.run_until_parked();
    assert_eq!(
        std::fs::read_to_string(&document).unwrap(),
        "![x](assets/pic.png)"
    );
    cx.update(|_, cx| assert_eq!(view.read(cx).state.editor.to_text(), "![x](assets/pic.png)"));
}
