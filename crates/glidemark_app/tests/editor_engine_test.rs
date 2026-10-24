use glidemark_app::editor_engine::EditorEngine;

#[test]
fn test_editor_typing_and_newline() {
    let mut editor = EditorEngine::new("Hello");
    assert_eq!(editor.cursor(), (0, 5));

    editor.insert_text(" World");
    assert_eq!(editor.to_text(), "Hello World");
    assert_eq!(editor.cursor(), (0, 11));

    editor.insert_newline();
    assert_eq!(editor.to_text(), "Hello World\n");
    assert_eq!(editor.cursor(), (1, 0));

    editor.insert_text("Line 2");
    assert_eq!(editor.to_text(), "Hello World\nLine 2");
    assert_eq!(editor.cursor(), (1, 6));
}

#[test]
fn test_editor_backspace_and_line_merging() {
    let mut editor = EditorEngine::new("Line 1\nLine 2");
    editor.set_cursor(1, 0);

    // Backspace at line 1 col 0 merges with line 0
    editor.delete_backward();
    assert_eq!(editor.to_text(), "Line 1Line 2");
    assert_eq!(editor.cursor(), (0, 6));

    // Backspace deletes char
    editor.delete_backward();
    assert_eq!(editor.to_text(), "Line Line 2");
    assert_eq!(editor.cursor(), (0, 5));
}

#[test]
fn test_editor_arrow_navigation() {
    let mut editor = EditorEngine::new("ABC\nDEF");
    editor.set_cursor(0, 3);

    editor.move_down();
    assert_eq!(editor.cursor(), (1, 3));

    editor.move_left();
    assert_eq!(editor.cursor(), (1, 2));

    editor.move_to_line_start();
    assert_eq!(editor.cursor(), (1, 0));

    editor.move_to_line_end();
    assert_eq!(editor.cursor(), (1, 3));
}

#[test]
fn test_editor_undo_redo() {
    let mut editor = EditorEngine::new("A");
    editor.insert_text("B");
    assert_eq!(editor.to_text(), "AB");

    editor.undo();
    assert_eq!(editor.to_text(), "A");

    editor.redo();
    assert_eq!(editor.to_text(), "AB");
}
