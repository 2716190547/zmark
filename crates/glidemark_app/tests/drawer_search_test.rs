use glidemark_app::state::{AppState, DocKey, FileTreeNode, OpenDoc, WorkspaceRoot};
use glidemark_core::document::Document;
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
    let stale_job = glidemark_app::state::SearchJob {
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

#[gpui::test]
async fn test_chinese_ime_entity_input_handler(cx: &mut gpui::TestAppContext) {
    use glidemark_app::views::drawer::SearchInput;
    use std::sync::atomic::{AtomicBool, Ordering};

    let (_view, cx) = cx.add_window_view(|_window, cx| {
        let focus = cx.focus_handle();
        SearchInput::new(focus).with_placeholder("Search...")
    });
    cx.run_until_parked();

    cx.update(|_window, cx| {
        let focus = cx.focus_handle();
        let query_changed = std::sync::Arc::new(AtomicBool::new(false));
        let qc = query_changed.clone();

        let mut input = SearchInput::new(focus)
            .with_placeholder("Search...")
            .on_change(move |_q, _window, _cx| {
                qc.store(true, Ordering::SeqCst);
            });

        // 1. Initial state
        assert_eq!(input.content.as_ref(), "");
        assert_eq!(input.marked_range, None);

        // 2. Composition (typing Chinese pinyin "zhongwen")
        input.handle_replace_and_mark_text(None, "zhongwen", None);
        assert_eq!(input.content.as_ref(), "zhongwen");
        assert_eq!(input.marked_range, Some(0..8));

        // 3. Confirm candidate (IME commits "中文")
        input.handle_replace_text(None, "中文");
        assert_eq!(input.content.as_ref(), "中文");
        assert_eq!(input.marked_range, None);

        // 4. Composition cancellation
        input.handle_replace_and_mark_text(None, "pinyin", None);
        assert!(input.marked_range.is_some());
        input.handle_unmark_text();
        assert_eq!(input.marked_range, None);

        // 5. Backspace, delete, paste, cursor movement
        input.set_text("test");

        input.handle_backspace();
        assert_eq!(input.content.as_ref(), "tes");

        input.handle_home();
        assert_eq!(input.selected_range, 0..0);

        input.handle_delete();
        assert_eq!(input.content.as_ref(), "es");

        input.handle_end();
        assert_eq!(input.selected_range, 2..2);

        input.handle_paste_text(" pasted");
        assert_eq!(input.content.as_ref(), "es pasted");
    });
}

#[gpui::test]
async fn test_search_input_single_pipeline_cursor_and_selection(cx: &mut gpui::TestAppContext) {
    use glidemark_app::views::drawer::SearchInput;
    use gpui::EntityInputHandler;

    let (view, cx) = cx.add_window_view(|_window, cx| {
        let focus = cx.focus_handle();
        SearchInput::new(focus).with_placeholder("Search...")
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        view.update(cx, |input, cx| {
            // Setup initial content "ab"
            input.set_text("ab");
            assert_eq!(input.content.as_ref(), "ab");
            assert_eq!(input.selected_range, 2..2);

            // a) Simulate input 'x' once via EntityInputHandler
            input.replace_text_in_range(None, "x", window, cx);
            assert_eq!(input.content.as_ref(), "abx", "Content must be 'abx', never 'abxx'");
            assert_eq!(input.selected_range, 3..3);

            // b) Move cursor left twice -> between 'a' and 'b' (index 1)
            input.handle_left();
            assert_eq!(input.selected_range, 2..2);
            input.handle_left();
            assert_eq!(input.selected_range, 1..1);

            // Input 'z' once via EntityInputHandler
            input.replace_text_in_range(None, "z", window, cx);
            assert_eq!(
                input.content.as_ref(),
                "azbx",
                "Content must be 'azbx', cursor must not jump to tail"
            );
            assert_eq!(
                input.selected_range, 2..2,
                "Cursor must be immediately after inserted 'z' (index 2)"
            );

            // c) Selection replacement: select range 1..3 ("zb") and input 'w'
            input.selected_range = 1..3;
            input.replace_text_in_range(None, "w", window, cx);
            assert_eq!(
                input.content.as_ref(),
                "awx",
                "Selection 1..3 ('zb') replaced by 'w' -> 'awx'"
            );
            assert_eq!(
                input.selected_range, 2..2,
                "Cursor must be at index 2 after replacing selection"
            );

            // d) Backspace / Delete in presence of selection only deletes selection
            // d.1 Backspace on selection
            input.set_text("hello world");
            input.selected_range = 5..11; // " world"
            input.handle_backspace();
            assert_eq!(
                input.content.as_ref(),
                "hello",
                "Backspace on selection 5..11 must delete only ' world'"
            );
            assert_eq!(input.selected_range, 5..5);

            // d.2 Delete on selection
            input.set_text("hello world");
            input.selected_range = 0..6; // "hello "
            input.handle_delete();
            assert_eq!(
                input.content.as_ref(),
                "world",
                "Delete on selection 0..6 must delete only 'hello '"
            );
            assert_eq!(input.selected_range, 0..0);

            // e) IME composition lifecycle:
            // e.1 replace_and_mark_text_in_range marks range
            input.set_text("find: ");
            input.replace_and_mark_text_in_range(None, "zhongwen", None, window, cx);
            assert_eq!(input.content.as_ref(), "find: zhongwen");
            assert_eq!(input.marked_range, Some(6..14));

            // e.2 confirm candidate replaces marked range and clears marked_range
            input.replace_text_in_range(None, "中文", window, cx);
            assert_eq!(input.content.as_ref(), "find: 中文");
            assert_eq!(input.marked_range, None);

            // e.3 composition cancellation via unmark_text
            input.replace_and_mark_text_in_range(None, "pinyin", None, window, cx);
            assert_eq!(input.content.as_ref(), "find: 中文pinyin");
            assert_eq!(input.marked_range, Some(12..18));
            input.unmark_text(window, cx);
            assert_eq!(input.marked_range, None);
        });
    });
}

#[gpui::test]
async fn test_chinese_ime_relative_selection_with_cjk_prefix(cx: &mut gpui::TestAppContext) {
    use glidemark_app::views::drawer::SearchInput;
    use gpui::EntityInputHandler;

    let (view, cx) = cx.add_window_view(|_window, cx| {
        let focus = cx.focus_handle();
        SearchInput::new(focus).with_placeholder("Search...")
    });
    cx.run_until_parked();

    cx.update(|window, cx| {
        view.update(cx, |input, cx| {
            // Content has 2 Chinese chars: "你好" (6 UTF-8 bytes, 2 UTF-16 code units)
            input.set_text("你好");
            assert_eq!(input.content.len(), 6);
            assert_eq!(input.selected_range, 6..6);

            // macOS IME calls replace_and_mark_text_in_range with new_selected_range_utf16:
            // e.g. typing "ma" (2 UTF-16 code units), caret at 1..1 within "ma"
            input.replace_and_mark_text_in_range(None, "ma", Some(1..1), window, cx);
            assert_eq!(input.content.as_ref(), "你好ma");
            assert_eq!(input.marked_range, Some(6..8));
            // Caret should be at UTF-8 offset 7 (between 'm' and 'a'), NOT out-of-bounds 9..9!
            assert_eq!(input.selected_range, 7..7);
        });
    });
}

#[gpui::test]
async fn test_search_input_selection_reversal_and_movement(cx: &mut gpui::TestAppContext) {
    use glidemark_app::views::drawer::SearchInput;

    let (_view, cx) = cx.add_window_view(|_window, cx| {
        let focus = cx.focus_handle();
        SearchInput::new(focus)
    });
    cx.run_until_parked();

    cx.update(|_window, cx| {
        let focus = cx.focus_handle();
        let mut input = SearchInput::new(focus);
        input.set_text("abcdef");
        input.move_to(3);
        assert_eq!(input.selected_range, 3..3);
        assert!(!input.selection_reversed);

        // Shift+Left: select to 2
        input.handle_select_left();
        assert_eq!(input.selected_range, 2..3);
        assert!(input.selection_reversed);
        assert_eq!(input.cursor_offset(), 2);

        // Press Left: collapses selection to start (2..2), resets selection_reversed
        input.handle_left();
        assert_eq!(input.selected_range, 2..2);
        assert!(!input.selection_reversed, "selection_reversed must be reset to false on move");
        assert_eq!(input.cursor_offset(), 2);

        // Shift+Right: select to 3
        input.handle_select_right();
        assert_eq!(input.selected_range, 2..3);
        assert!(!input.selection_reversed);
        assert_eq!(input.cursor_offset(), 3);
    });
}
