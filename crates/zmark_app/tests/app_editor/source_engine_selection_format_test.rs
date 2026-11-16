use zmark_app::editor_engine::EditorEngine;

#[test]
fn single_line_replacements_reuse_other_lines_and_preserve_unicode_history() {
    let original = "first untouched\na中🙂z\r\nlast untouched";
    let mut editor = EditorEngine::new(original);
    let pointer = editor.lines[0].as_ptr();
    editor.set_selection_offsets(17, 19, true);
    editor.insert_text("文");
    assert_eq!(editor.to_text(), "first untouched\na文z\r\nlast untouched");
    assert_eq!(editor.lines[0].as_ptr(), pointer, "a local edit must not rebuild every unmodified line");
    assert_eq!(editor.char_count, editor.to_text().chars().count());
    assert!(editor.undo());
    assert_eq!(editor.to_text(), original);
    assert_eq!(editor.selected_text().as_deref(), Some("中🙂"));
    assert!(editor.selection_reversed());
    assert!(editor.redo());
    assert_eq!(editor.to_text(), "first untouched\na文z\r\nlast untouched");
}

#[test]
fn local_edits_match_the_general_transaction_with_shared_lines_and_crlf() {
    for original in ["a中🙂z\r\nnext", "\r\n", "first\n中🙂\nlast", "", "🙂"] {
        for start in 0..=original.chars().count() {
            for end in start..=original.chars().count() {
                for replacement in ["", "中", "🙂", "\r", "x\ny"] {
                    let mut local = EditorEngine::new(original);
                    let mut general = local.clone();
                    let shared = local.lines.clone();
                    general.set_column_ranges(vec![start..end]);
                    local.replace_ranges(&[start..end], &[replacement.into()]).unwrap();
                    general.replace_ranges(&[start..end], &[replacement.into()]).unwrap();
                    assert_eq!(local.to_text(), general.to_text(), "{original:?} {start}..{end} {replacement:?}");
                    assert_eq!(local.cursor(), general.cursor());
                    assert_eq!(local.char_count, general.char_count);
                    assert_eq!(local.revision, general.revision);
                    assert_eq!(shared.as_ref(), &original.split('\n').map(str::to_owned).collect::<Vec<_>>());
                    if local.can_undo() {
                        local.undo();
                        assert_eq!(local.to_text(), original);
                        local.redo();
                        assert_eq!(local.to_text(), general.to_text());
                    }
                }
            }
        }
    }
}

#[test]
fn columns_keep_carets_for_repeated_unicode_edits_and_one_undo() {
    let original = "a中z\n\nb🎉z";
    let mut editor = EditorEngine::new(original);
    editor.set_column_ranges(vec![1..2, 4..4, 6..7, 4..4]);
    assert_eq!(editor.selected_text().as_deref(), Some("中\n\n🎉"));
    editor.insert_text("X");
    assert_eq!(editor.to_text(), "aXz\nX\nbXz");
    assert_eq!(editor.revision, 1);
    assert!(editor.is_column_selection());
    editor.insert_text("文");
    assert_eq!(editor.to_text(), "aX文z\nX文\nbX文z");
    editor.delete_backward();
    assert_eq!(editor.to_text(), "aXz\nX\nbXz");
    assert!(editor.undo());
    assert_eq!(editor.to_text(), "aX文z\nX文\nbX文z");
    assert!(
        editor.is_column_selection(),
        "undo restores the prior column carets"
    );
    assert_eq!(editor.selection_ranges(), vec![3..3, 7..7, 11..11]);
    assert!(editor.undo());
    assert_eq!(editor.to_text(), "aXz\nX\nbXz");
    assert!(editor.undo());
    assert_eq!(editor.to_text(), original);
}

#[test]
fn column_paste_retains_last_empty_line_and_rejects_mismatch_atomically() {
    let mut editor = EditorEngine::new("ab\ncd\nef");
    editor.set_column_ranges(vec![1..1, 4..4, 7..7]);
    let before = editor.clone();
    assert!(editor.paste_text("one\ntwo").is_err());
    assert_eq!(
        editor, before,
        "text, selection, revision and history must all stay unchanged"
    );
    editor.paste_text("中\r\n🎉\r\n").unwrap();
    assert_eq!(editor.to_text(), "a中b\nc🎉d\nef");
    assert_eq!(editor.revision, 1);
    editor.paste_text("!").unwrap();
    assert_eq!(editor.to_text(), "a中!b\nc🎉!d\ne!f");
}

#[test]
fn replacing_identical_text_collapses_selection_without_adding_history() {
    let mut editor = EditorEngine::new("abc");
    editor.set_selection_offsets(0, 2, false);
    editor.insert_text("ab");
    assert_eq!(editor.cursor(), (0, 2));
    assert!(editor.selection().is_none());
    assert_eq!(editor.revision, 0);
    assert!(!editor.can_undo());
}

#[test]
fn undo_and_indent_restore_real_ranges_and_cursor() {
    let mut editor = EditorEngine::new("ab\ncd");
    editor.set_column_ranges(vec![1..1, 4..4]);
    editor.indent_lines(2, false);
    assert_eq!(editor.selection_ranges(), vec![3..3, 8..8]);
    editor.insert_text("X");
    assert_eq!(editor.to_text(), "  aXb\n  cXd");
    editor.undo();
    assert_eq!(editor.selection_ranges(), vec![3..3, 8..8]);
    editor.undo();
    assert_eq!(editor.to_text(), "ab\ncd");
    assert_eq!(editor.selection_ranges(), vec![1..1, 4..4]);
}

#[test]
fn bold_is_a_reversible_toggle_and_preserves_the_selected_content() {
    let mut editor = EditorEngine::new("a中文🎉z");
    editor.set_selection_offsets(1, 4, true);
    editor.toggle_bold();
    assert_eq!(editor.to_text(), "a**中文🎉**z");
    assert_eq!(editor.selected_text().as_deref(), Some("中文🎉"));
    assert!(editor.selection_reversed());
    editor.toggle_bold();
    assert_eq!(editor.to_text(), "a中文🎉z");
    assert_eq!(editor.selected_text().as_deref(), Some("中文🎉"));
    editor.set_selection_offsets(0, editor.char_count, false);
    editor.toggle_bold();
    editor.set_selection_offsets(0, editor.char_count, false);
    editor.toggle_bold();
    assert_eq!(editor.to_text(), "a中文🎉z");
}

#[test]
fn numbered_lists_preserve_indentation_and_skip_fenced_code() {
    let original = "  alpha\n\n  beta\n```rust\n42. untouched\n```";
    let mut editor = EditorEngine::new(original);
    editor.select_all();
    editor.toggle_numbered_list();
    assert_eq!(
        editor.to_text(),
        "  1. alpha\n\n  2. beta\n```rust\n42. untouched\n```"
    );
    editor.toggle_numbered_list();
    assert_eq!(editor.to_text(), original);
    let mut editor = EditorEngine::new(&format!("{}. alpha", usize::MAX));
    editor.insert_newline();
    assert!(
        editor.to_text().ends_with('\n'),
        "overflow must not panic or wrap numbering"
    );
}

#[test]
fn multiline_indent_does_not_include_the_next_line_at_column_zero() {
    let mut editor = EditorEngine::new("a\nb\nc");
    editor.set_selection_offsets(0, 4, false);
    editor.indent_lines(2, false);
    assert_eq!(editor.to_text(), "  a\n  b\nc");
    editor.indent_lines(2, true);
    assert_eq!(editor.to_text(), "a\nb\nc");
    assert!(editor.undo());
    assert_eq!(editor.to_text(), "  a\n  b\nc");
}

#[test]
fn crlf_navigation_and_deletion_keep_one_physical_line_break() {
    let mut editor = EditorEngine::new("ab\r\ncd");
    editor.set_cursor(0, 2);
    editor.move_right();
    assert_eq!(editor.cursor(), (1, 0), "CRLF is one navigation boundary");
    editor.move_left();
    assert_eq!(editor.cursor(), (0, 2));
    editor.delete_forward();
    assert_eq!(editor.to_text(), "abcd");
    assert!(editor.undo());
    editor.set_cursor(1, 0);
    editor.delete_backward();
    assert_eq!(editor.to_text(), "abcd");
    assert!(editor.undo());
    editor.set_cursor(0, 2);
    editor.insert_newline();
    assert_eq!(
        editor.to_text(),
        "ab\r\n\r\ncd",
        "Enter preserves the document's newline convention"
    );
}

#[test]
fn numbering_and_enter_never_change_nested_or_long_fenced_code() {
    let original = "````text\n```\n42. code\n~~~\n````\nplain";
    let mut editor = EditorEngine::new(original);
    editor.select_all();
    editor.toggle_numbered_list();
    assert_eq!(
        editor.to_text(),
        "````text\n```\n42. code\n~~~\n````\n1. plain"
    );
    editor.set_cursor(2, 8);
    editor.insert_newline();
    assert!(
        editor.to_text().contains("42. code\n\n~~~"),
        "Enter in code inserts only a line break, not a list marker"
    );
}

#[test]
fn duplicate_column_boundaries_and_large_insertions_are_safe() {
    let mut editor = EditorEngine::new("abc");
    editor.set_column_ranges(vec![0..2, 1..1, 2..2]);
    editor.insert_text("X");
    assert_eq!(
        editor.to_text(),
        "Xc",
        "a boundary shared by visual rows is edited once"
    );
    let before = editor.clone();
    let huge = "x".repeat(8 * 1024 * 1024 + 1);
    assert!(editor.replace_ranges(&[0..0], &[huge]).is_err());
    assert_eq!(editor, before);
}

#[test]
fn word_movement_and_multiclick_ranges_use_character_boundaries() {
    let mut editor = EditorEngine::new("alpha 中文🎉 beta\nlast");
    editor.set_cursor(0, 0);
    editor.move_by_word(true, true);
    assert_eq!(editor.selected_text().as_deref(), Some("alpha "));
    editor.select_word(0, 7);
    assert_eq!(editor.selected_text().as_deref(), Some("中文"));
    editor.select_line(0);
    assert_eq!(
        editor.selected_text().as_deref(),
        Some("alpha 中文🎉 beta\n")
    );
}
