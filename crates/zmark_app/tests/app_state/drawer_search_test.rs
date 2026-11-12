use zmark_app::state::{AppState, FileTreeNode, WorkspaceRoot};
use zmark_core::document::Document;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_set_search_query_and_clear_search() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

    // Initial state
    assert_eq!(state.search_query, "");
    assert!(state.search_original_expanded.is_none());
    assert!(!state.search_in_progress);
    assert!(!state.search_truncated);
    assert!(state.has_search_matches());

    // Setting query marks search_in_progress as true before background execution
    state.set_search_query("test_file");
    assert_eq!(state.search_query, "test_file");
    assert!(state.search_original_expanded.is_some());
    assert!(state.search_in_progress);

    // Executing search marks search_in_progress as false
    state.execute_search();
    assert!(!state.search_in_progress);

    // Clearing query restores search_in_progress to false
    state.clear_search();
    assert_eq!(state.search_query, "");
    assert!(state.search_original_expanded.is_none());
    assert!(!state.search_in_progress);
    assert!(!state.search_truncated);
    assert!(state.has_search_matches());
}

#[test]
fn test_case_insensitive_matching() {
    let doc = Document::new_scratch();
    let state = AppState::new_isolated(doc);

    // Empty query matches anything
    assert!(state.file_matches_search("README.md"));
    assert!(state.file_matches_search("anything.txt"));

    let mut state = state;
    // Substring matching is case-insensitive
    state.set_search_query("README");
    assert!(state.file_matches_search("README.md"));
    assert!(state.file_matches_search("readme.md"));
    assert!(state.file_matches_search("ReadMe.markdown"));

    state.set_search_query("readme");
    assert!(state.file_matches_search("README.md"));
    assert!(state.file_matches_search("readme.md"));

    state.set_search_query("Read");
    assert!(state.file_matches_search("README.md"));
    assert!(state.file_matches_search("unread_items.md"));

    state.set_search_query("xyz");
    assert!(!state.file_matches_search("README.md"));
}

#[test]
fn test_auto_expansion_and_restoration() {
    let tmp = tempdir().unwrap();
    let root_path = tmp.path().join("project");
    let sub1 = root_path.join("src");
    let sub2 = sub1.join("components");
    fs::create_dir_all(&sub2).unwrap();

    let target_file = sub2.join("Button.md");
    fs::write(&target_file, "# Button Component").unwrap();

    let other_dir = root_path.join("tests");
    fs::create_dir_all(&other_dir).unwrap();
    fs::write(other_dir.join("test1.md"), "# Test").unwrap();

    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);
    state.add_workspace_root(root_path.clone()).unwrap();

    // Collapse all folders initially
    for node in state.workspace_roots[0].tree.iter_mut() {
        node.is_expanded = false;
        for child in node.children.iter_mut() {
            child.is_expanded = false;
        }
    }

    let src_node = state.workspace_roots[0].tree.iter().find(|n| n.name == "src").unwrap();
    assert!(!src_node.is_expanded, "src should be collapsed initially");

    // Search for Button -> marks search in progress, executing search auto-expands ancestors
    state.set_search_query("Button");
    assert_eq!(state.search_query, "Button");
    assert!(state.search_in_progress);

    state.execute_search();
    assert!(!state.search_in_progress);
    assert!(state.has_search_matches());

    let src_node_after = state.workspace_roots[0].tree.iter().find(|n| n.name == "src").unwrap();
    assert!(src_node_after.is_expanded, "src must be auto-expanded during search");

    let comp_node = src_node_after.children.iter().find(|n| n.name == "components").unwrap();
    assert!(comp_node.is_expanded, "components must be auto-expanded during search");

    // Clear search -> original expansion states should be restored
    state.clear_search();
    let src_node_restored = state.workspace_roots[0].tree.iter().find(|n| n.name == "src").unwrap();
    assert!(!src_node_restored.is_expanded, "src must be restored to collapsed after clearing search");
}

#[test]
fn test_restoration_preserves_pre_search_expanded_folders() {
    let tmp = tempdir().unwrap();
    let root_path = tmp.path().join("repo");
    let dir_kept = root_path.join("docs");
    let dir_auto = root_path.join("archive");
    fs::create_dir_all(&dir_kept).unwrap();
    fs::create_dir_all(&dir_auto).unwrap();

    fs::write(dir_kept.join("guide.md"), "# Guide").unwrap();
    fs::write(dir_auto.join("old_spec.md"), "# Old Spec").unwrap();

    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);
    state.add_workspace_root(root_path.clone()).unwrap();

    // Explicitly set docs as expanded and archive as collapsed
    for node in state.workspace_roots[0].tree.iter_mut() {
        if node.name == "docs" {
            node.is_expanded = true;
        } else if node.name == "archive" {
            node.is_expanded = false;
        }
    }

    // Search for "old_spec" in archive
    state.set_search_query("old_spec");
    assert!(state.search_in_progress);
    state.execute_search();
    assert!(!state.search_in_progress);

    let archive_node = state.workspace_roots[0].tree.iter().find(|n| n.name == "archive").unwrap();
    assert!(archive_node.is_expanded, "archive must be expanded during search");

    // Clear search
    state.clear_search();

    // Verify docs is still expanded, and archive is collapsed back
    let docs_node = state.workspace_roots[0].tree.iter().find(|n| n.name == "docs").unwrap();
    let archive_node_final = state.workspace_roots[0].tree.iter().find(|n| n.name == "archive").unwrap();
    assert!(docs_node.is_expanded, "docs was originally expanded and must remain expanded");
    assert!(!archive_node_final.is_expanded, "archive was originally collapsed and must be restored to collapsed");
}

#[test]
fn test_search_truncated_flag() {
    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);

    state.workspace_roots.push(WorkspaceRoot {
        path: PathBuf::from("/mock/root"),
        tree: vec![FileTreeNode {
            path: PathBuf::from("/mock/root/file.md"),
            name: "file.md".to_string(),
            is_dir: false,
            is_expanded: false,
            children: Vec::new(),
            mtime: None,
        }],
        is_expanded: true,
        is_missing: false,
        truncated: true, // Marked truncated!
        entries_used: 100,
        files_used: 50,
        deferred: false,
    });

    state.set_search_query("file");
    assert!(state.search_in_progress);
    state.execute_search();
    assert!(!state.search_in_progress);
    assert!(state.search_truncated, "search_truncated must be true when root is truncated");

    state.clear_search();
    assert!(!state.search_truncated, "search_truncated must reset to false on clear");
}

#[test]
fn test_background_search_lifecycle() {
    let tmp = tempdir().unwrap();
    let root_path = tmp.path().join("repo");
    fs::create_dir_all(&root_path).unwrap();
    fs::write(root_path.join("async_target.md"), "# Target").unwrap();

    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);
    state.add_workspace_root(root_path).unwrap();

    // 1. Initial typing initiates search_in_progress
    state.set_search_query("async_target");
    let gen1 = state.search_generation;
    assert!(state.search_in_progress);
    assert_eq!(state.search_query, "async_target");

    // 2. Background thread execution
    let job = state.prepare_search_job().expect("Search job should be preparable");
    let handle = std::thread::spawn(move || job.execute());
    let result = handle.join().expect("Background search thread failed");

    // 3. Applying results marks search_in_progress as false
    let applied = state.apply_search_result(result);
    assert!(applied);
    assert!(!state.search_in_progress);
    assert!(state.has_search_matches());

    // 4. Stale results with old generation are rejected
    state.set_search_query("newer_query");
    let stale_job = zmark_app::state::SearchJob {
        generation: gen1, // Stale generation
        query: "async_target".to_string(),
        roots: Vec::new(),
        scan_budget: state.scan_budget.clone(),
    };
    let stale_result = stale_job.execute();
    let stale_applied = state.apply_search_result(stale_result);
    assert!(!stale_applied, "Stale search result must be rejected");
    assert!(state.search_in_progress);

    // 5. Clear cancels in-progress search
    state.clear_search();
    assert!(!state.search_in_progress);
}

#[test]
fn test_root_removal_during_search_does_not_shift_indices_or_panic() {
    let tmp = tempdir().unwrap();
    let root_a_path = tmp.path().join("root_a");
    let root_b_path = tmp.path().join("root_b");
    fs::create_dir_all(root_a_path.join("sub_a")).unwrap();
    fs::create_dir_all(root_b_path.join("sub_b")).unwrap();
    fs::write(root_a_path.join("sub_a").join("file_a.md"), "# File A").unwrap();
    fs::write(root_b_path.join("sub_b").join("file_b.md"), "# File B").unwrap();

    let root_a = fs::canonicalize(&root_a_path).unwrap();
    let root_b = fs::canonicalize(&root_b_path).unwrap();

    let doc = Document::new_scratch();
    let mut state = AppState::new_isolated(doc);
    state.add_workspace_root(root_a_path).unwrap();
    state.add_workspace_root(root_b_path).unwrap();

    // Mark sub_b expanded before search
    for node in state.workspace_roots[1].tree.iter_mut() {
        if node.name == "sub_b" {
            node.is_expanded = true;
        }
    }

    // Set search query: original expanded state is saved keyed by PathBuf
    state.set_search_query("file");
    assert!(state.search_original_expanded.is_some());
    let saved = state.search_original_expanded.as_ref().unwrap();
    assert!(saved.contains_key(&root_a));
    assert!(saved.contains_key(&root_b));

    // Remove root_a (index 0). root_b shifts from index 1 to index 0!
    state.remove_workspace_root(0).unwrap();
    assert_eq!(state.workspace_roots.len(), 1);
    assert_eq!(state.workspace_roots[0].path, root_b);

    // Verify root_a was cleanly removed from saved expanded map
    let saved_after_remove = state.search_original_expanded.as_ref().unwrap();
    assert!(!saved_after_remove.contains_key(&root_a), "Removed root must be removed from saved map");
    assert!(saved_after_remove.contains_key(&root_b), "Remaining root must remain in saved map");

    // Clearing search must not panic and must restore root_b's expansion
    state.clear_search();
    assert_eq!(state.search_query, "");
    assert!(state.search_original_expanded.is_none());

    let sub_b_restored = state.workspace_roots[0].tree.iter().find(|n| n.name == "sub_b").unwrap();
    assert!(sub_b_restored.is_expanded, "root_b's sub_b folder must be restored properly without index shift");
}

fn native_search(
    cx: &mut gpui::TestAppContext,
) -> (
    gpui::Entity<zmark_app::views::workspace::WorkspaceView>,
    gpui::Entity<zmark_app::views::drawer::SearchInput>,
    &mut gpui::VisualTestContext,
) {
    use gpui::{AppContext as _, Focusable as _};
    use zmark_app::views::workspace::WorkspaceView;
    let (root, cx) = cx.add_window_view(|window, cx| {
        let mut state = AppState::new_isolated(Document::new_scratch());
        state.drawer_open = true;
        state.preferences.reduce_motion = true;
        let view = cx.new(|cx| WorkspaceView::new(state, cx));
        gpui_component::Root::new(view, window, cx)
    });
    cx.run_until_parked();
    let view = cx.update(|_, cx| root.read(cx).view().clone().downcast::<WorkspaceView>().unwrap());
    let input = cx.update(|window, cx| {
        let input = view.read(cx).search_input.as_ref().unwrap().clone();
        let focus = input.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        input
    });
    cx.run_until_parked();
    (view, input, cx)
}

#[gpui::test]
async fn test_chinese_ime_entity_input_handler(cx: &mut gpui::TestAppContext) {
    use gpui::{ClipboardItem, EntityInputHandler};
    let (view, input, cx) = native_search(cx);
    cx.update(|window, cx| input.update(cx, |input, cx| {
        assert_eq!(input.value().as_ref(), "");
        assert_eq!(input.marked_text_range(window, cx), None);
        input.replace_and_mark_text_in_range(None, "zhongwen", None, window, cx);
        assert_eq!(input.value().as_ref(), "zhongwen");
        assert_eq!(input.marked_text_range(window, cx), Some(0..8));
    }));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(view.read(cx).state.search_query, "", "preedit must not launch a search"));
    cx.update(|window, cx| input.update(cx, |input, cx| {
        input.replace_text_in_range(None, "中文", window, cx);
        assert_eq!(input.value().as_ref(), "中文");
        assert_eq!(input.marked_text_range(window, cx), None);
    }));
    cx.run_until_parked();
    cx.update(|_, cx| assert_eq!(view.read(cx).state.search_query, "中文"));
    cx.update(|window, cx| input.update(cx, |input, cx| {
        input.replace_and_mark_text_in_range(None, "pinyin", None, window, cx);
        assert!(input.marked_text_range(window, cx).is_some());
        input.unmark_text(window, cx);
        assert_eq!(input.marked_text_range(window, cx), None);
        input.set_value("test", window, cx);
    }));
    cx.simulate_keystrokes("backspace");
    cx.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "tes"));
    cx.simulate_keystrokes("home");
    cx.update(|_, cx| assert_eq!(input.read(cx).selected_range(), 0..0));
    cx.simulate_keystrokes("delete");
    cx.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "es"));
    cx.simulate_keystrokes("end");
    cx.update(|_, cx| assert_eq!(input.read(cx).selected_range(), 2..2));
    cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string(" pasted".into())));
    cx.simulate_keystrokes("secondary-v");
    cx.update(|_, cx| {
        assert_eq!(input.read(cx).value().as_ref(), "es pasted");
        assert_eq!(view.read(cx).state.editor.revision, 0, "native input must not edit the document");
    });
}

#[gpui::test]
async fn test_search_input_single_pipeline_cursor_and_selection(cx: &mut gpui::TestAppContext) {
    use gpui::EntityInputHandler;
    let (_, input, cx) = native_search(cx);
    cx.update(|window, cx| input.update(cx, |input, cx| {
        input.set_value("ab", window, cx);
        assert_eq!(input.value().as_ref(), "ab");
        assert_eq!(input.selected_range(), 2..2);
        input.replace_text_in_range(None, "x", window, cx);
        assert_eq!(input.value().as_ref(), "abx", "never double-insert 'x'");
        assert_eq!(input.selected_range(), 3..3);
    }));
    cx.simulate_keystrokes("left");
    cx.update(|_, cx| assert_eq!(input.read(cx).selected_range(), 2..2));
    cx.simulate_keystrokes("left");
    cx.update(|window, cx| input.update(cx, |input, cx| {
        assert_eq!(input.selected_range(), 1..1);
        input.replace_text_in_range(None, "z", window, cx);
        assert_eq!(input.value().as_ref(), "azbx", "insertion must not jump to the tail");
        assert_eq!(input.selected_range(), 2..2);
        input.set_selected_range(1..3, cx);
        input.replace_text_in_range(None, "w", window, cx);
        assert_eq!(input.value().as_ref(), "awx");
        assert_eq!(input.selected_range(), 2..2);
        input.set_value("hello world", window, cx);
        input.set_selected_range(5..11, cx);
    }));
    cx.simulate_keystrokes("backspace");
    cx.update(|window, cx| input.update(cx, |input, cx| {
        assert_eq!(input.value().as_ref(), "hello", "Backspace deletes only the selection");
        assert_eq!(input.selected_range(), 5..5);
        input.set_value("hello world", window, cx);
        input.set_selected_range(0..6, cx);
    }));
    cx.simulate_keystrokes("delete");
    cx.update(|window, cx| input.update(cx, |input, cx| {
        assert_eq!(input.value().as_ref(), "world", "Delete deletes only the selection");
        assert_eq!(input.selected_range(), 0..0);
        input.set_value("find: ", window, cx);
        input.replace_and_mark_text_in_range(None, "zhongwen", None, window, cx);
        assert_eq!(input.value().as_ref(), "find: zhongwen");
        assert_eq!(input.marked_text_range(window, cx), Some(6..14));
        input.replace_text_in_range(None, "中文", window, cx);
        assert_eq!(input.value().as_ref(), "find: 中文");
        assert_eq!(input.marked_text_range(window, cx), None);
        input.replace_and_mark_text_in_range(None, "pinyin", None, window, cx);
        assert_eq!(input.value().as_ref(), "find: 中文pinyin");
        // The public native IME range is UTF-16, while selected_range is UTF-8.
        assert_eq!(input.marked_text_range(window, cx), Some(8..14));
        input.unmark_text(window, cx);
        assert_eq!(input.marked_text_range(window, cx), None);
    }));
}

#[gpui::test]
async fn test_chinese_ime_relative_selection_with_cjk_prefix(cx: &mut gpui::TestAppContext) {
    use gpui::EntityInputHandler;
    let (_, input, cx) = native_search(cx);
    cx.update(|window, cx| input.update(cx, |input, cx| {
        input.set_value("你好", window, cx);
        assert_eq!(input.value().len(), 6);
        assert_eq!(input.selected_range(), 6..6);
        input.replace_and_mark_text_in_range(None, "ma", Some(1..1), window, cx);
        assert_eq!(input.value().as_ref(), "你好ma");
        assert_eq!(input.marked_text_range(window, cx), Some(2..4));
        assert_eq!(input.selected_range(), 7..7, "caret is between m and a, not at byte 9");
    }));
}

#[gpui::test]
async fn native_search_clipboard_history_and_escape_leave_source_untouched(cx: &mut gpui::TestAppContext) {
    let (view, input, cx) = native_search(cx);
    cx.simulate_input("中文abc");
    cx.simulate_keystrokes("shift-left shift-left secondary-c");
    assert_eq!(cx.read_from_clipboard().and_then(|item| item.text()).as_deref(), Some("bc"));
    cx.simulate_keystrokes("secondary-x");
    cx.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "中文a"));
    cx.simulate_keystrokes("secondary-z");
    cx.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "中文abc"));
    cx.simulate_keystrokes("secondary-shift-z");
    cx.update(|_, cx| assert_eq!(input.read(cx).value().as_ref(), "中文a"));
    cx.simulate_keystrokes("escape");
    cx.update(|_, cx| {
        assert!(input.read(cx).value().is_empty());
        assert!(view.read(cx).state.search_query.is_empty());
        assert_eq!(view.read(cx).state.editor.revision, 0);
        assert!(view.read(cx).state.editor.to_text().is_empty());
    });
}

#[gpui::test]
async fn test_search_input_selection_reversal_and_movement(cx: &mut gpui::TestAppContext) {
    let (_, input, cx) = native_search(cx);
    cx.update(|window, cx| input.update(cx, |input, cx| {
        input.set_value("abcdef", window, cx);
        input.set_selected_range(3..3, cx);
        assert_eq!(input.selected_range(), 3..3);
        assert!(!input.is_selection_reversed());
    }));
    cx.simulate_keystrokes("shift-left");
    cx.update(|_, cx| {
        let input = input.read(cx);
        assert_eq!(input.selected_range(), 2..3);
        assert!(input.is_selection_reversed());
        assert_eq!(input.cursor(), 2);
    });
    cx.simulate_keystrokes("left");
    cx.update(|_, cx| {
        let input = input.read(cx);
        assert_eq!(input.selected_range(), 2..2);
        assert!(!input.is_selection_reversed());
        assert_eq!(input.cursor(), 2);
    });
    cx.simulate_keystrokes("shift-right");
    cx.update(|_, cx| {
        let input = input.read(cx);
        assert_eq!(input.selected_range(), 2..3);
        assert!(!input.is_selection_reversed());
        assert_eq!(input.cursor(), 3);
    });
}
