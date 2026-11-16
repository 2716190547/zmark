use gpui::{
    AppContext, Entity, EntityInputHandler, Focusable, Modifiers, MouseButton, Pixels, Point,
    VisualTestContext,
};
use zmark_app::{
    state::{AppState, ViewMode},
    views::workspace::{SourceCommand, WorkspaceView},
};
use zmark_core::document::Document;

fn source_window<'a>(
    cx: &'a mut gpui::TestAppContext,
    text: &str,
) -> (Entity<WorkspaceView>, &'a mut VisualTestContext) {
    let text = text.to_owned();
    let (root, cx) = cx.add_window_view(|window, cx| {
        zmark_app::theme::init_components(cx);
        zmark_app::actions::init_key_bindings(cx);
        let view = cx.new(|cx| {
            let mut state = AppState::new_isolated(Document::new_scratch_with_text(text));
            state.set_view_mode(ViewMode::Source);
            state.preferences.reduce_motion = true;
            state.preferences.auto_save = false;
            state.editor.set_cursor(0, 0);
            WorkspaceView::new(state, cx)
        });
        let focus = view.read(cx).focus_handle.clone();
        window.focus(&focus, cx);
        gpui_component::Root::new(view, window, cx)
    });
    let view = root.read_with(cx, |root, _| {
        root.view().clone().downcast::<WorkspaceView>().unwrap()
    });
    draw(cx);
    (view, cx)
}

#[gpui::test]
fn find_results_scroll_independently_and_click_keeps_the_selected_match(cx: &mut gpui::TestAppContext) {
    let text = (0..80).map(|index| format!("match {index}")).collect::<Vec<_>>().join("\n");
    let (view, cx) = source_window(cx, &text);
    cx.simulate_keystrokes("secondary-f");
    cx.simulate_input("match");
    draw(cx);
    let first = cx.debug_bounds("find_result_0").unwrap();
    let before = view.read_with(cx, |ws, _| ws.state.editor.cursor());
    cx.simulate_event(gpui::ScrollEvent { position: first.center(), delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(0.), gpui::px(-100.))),
        modifiers: Modifiers::none(), touch_phase: gpui::TouchPhase::Moved, source: gpui::ScrollSource::Pointer });
    draw(cx);
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.cursor()), before, "wheel on results must not edit or jump source");
    let later = cx.debug_bounds("find_result_6").expect("scroll exposes later result");
    cx.simulate_click(later.center(), Modifiers::none());
    draw(cx);
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.cursor().0), 6);
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.selected_text().unwrap()), "match");
}

#[gpui::test]
fn find_arrows_cycle_matches_only_while_search_input_is_focused(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "match\nmatch\nmatch");
    cx.simulate_keystrokes("secondary-f");
    cx.simulate_input("match");
    draw(cx);
    cx.update(|_, cx| assert_eq!(view.read(cx).state.editor.selection(), Some((0, 5))));

    for (keys, expected) in [
        ("down", (6, 11)),
        ("down", (12, 17)),
        ("down", (0, 5)),
        ("up", (12, 17)),
        ("enter", (0, 5)),
        ("shift-enter", (12, 17)),
    ] {
        cx.simulate_keystrokes(keys);
        draw(cx);
        cx.update(|_, cx| assert_eq!(view.read(cx).state.editor.selection(), Some(expected), "{keys}"));
    }
    cx.update(|window, cx| {
        view.update(cx, |ws, _| ws.state.editor.set_cursor(1, 2));
        let focus = view.read(cx).focus_handle.clone();
        window.focus(&focus, cx);
    });
    cx.simulate_keystrokes("down");
    draw(cx);
    cx.update(|_, cx| {
        let editor = &view.read(cx).state.editor;
        assert_eq!(editor.cursor(), (2, 2), "source arrows must still move the caret");
        assert!(editor.selection().is_none());
        assert_eq!(editor.to_text(), "match\nmatch\nmatch");
    });
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

fn caret_point(
    view: &Entity<WorkspaceView>,
    line: usize,
    col: usize,
    cx: &mut VisualTestContext,
) -> Point<Pixels> {
    cx.update(|window, cx| {
        let ws = view.read(cx);
        let rows = ws.source_rows.lock().unwrap();
        let row = rows
            .iter()
            .find(|row| {
                row.visual.physical_line_idx == line
                    && col >= row.visual.char_start
                    && col <= row.visual.char_start + row.visual.char_len
            })
            .expect("rendered source row");
        let x = zmark_app::views::editor::calculate_cursor_x_offset_tokens(
            window.text_system(),
            &row.tokens,
            col - row.visual.char_start,
            ws.state.preferences.font_size,
        );
        gpui::point(row.bounds.left() + gpui::px(x), row.bounds.center().y)
    })
}

#[gpui::test]
fn native_source_utf16_queries_match_the_display_across_crlf_and_emoji(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "中🙂\r\n\nend🎉");
    cx.update(|window, cx| view.update(cx, |ws, cx| {
        for composing in [false, true] {
            if composing {
                ws.state.editor.set_cursor(2, 1);
                ws.replace_and_mark_text_in_range(None, "字🙂", Some(1..1), window, cx);
            }
            let display = ws.source_display_lines().join("\n");
            let mut boundaries = vec![(0, 0)];
            let mut units = 0;
            for (byte, ch) in display.char_indices() {
                units += ch.len_utf16();
                boundaries.push((units, byte + ch.len_utf8()));
            }
            assert_eq!(ws.text_length_utf16(window, cx), Some(units));
            for &(start, a) in &boundaries {
                for &(end, b) in &boundaries {
                    let mut adjusted = None;
                    let got = ws.text_for_range(start..end, &mut adjusted, window, cx);
                    if start <= end {
                        assert_eq!(got.as_deref(), Some(&display[a..b]));
                        assert_eq!(adjusted, Some(start..end));
                    } else {
                        assert!(got.is_none());
                    }
                }
            }
            for offset in 0..=units + 1 {
                if !boundaries.iter().any(|&(unit, _)| unit == offset) {
                    assert!(ws.text_for_range(offset..offset, &mut None, window, cx).is_none());
                }
            }
        }
    }));
}

#[gpui::test]
fn native_source_hit_testing_uses_actual_rows_with_open_and_closed_drawer(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "abcdef\n你好世界\n🦀Rust\nabc `code` end");
    for open in [true, false] {
        cx.update(|_, cx| view.update(cx, |ws, cx| { ws.state.drawer_open = open; cx.notify(); }));
        draw(cx);
        for (line, text) in ["abcdef", "你好世界", "🦀Rust", "abc `code` end"].iter().enumerate() {
            for col in 0..=text.chars().count() {
                let pos = caret_point(&view, line, col, cx);
                cx.simulate_mouse_down(pos, MouseButton::Left, Modifiers::none());
                cx.simulate_mouse_up(pos, MouseButton::Left, Modifiers::none());
                draw(cx);
                assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.cursor()), (line, col), "drawer={open} {text:?}");
            }
        }
    }
}

#[gpui::test]
fn native_source_drag_highlight_copy_replace_and_undo(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "a中文\n🎉beta");
    let first = caret_point(&view, 0, 1, cx);
    let last = caret_point(&view, 1, 2, cx);
    cx.simulate_mouse_down(first, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(last, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(last, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selected_text()),
        Some("中文\n🎉b".into())
    );
    assert!(cx.debug_bounds("source_selection_0_1_3").is_some());
    cx.simulate_keystrokes("secondary-c");
    cx.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("中文\n🎉b")
        )
    });
    cx.simulate_input("X");
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "aXeta"
    );
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.revision), 1);
    cx.simulate_keystrokes("secondary-z");
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "a中文\n🎉beta"
    );
}

#[gpui::test]
fn native_source_ime_preedit_is_display_only_and_commit_is_one_undo(cx: &mut gpui::TestAppContext) {
    let original = "你好🎉 end";
    let (view, cx) = source_window(cx, original);
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.state.editor.set_cursor(0, 3);
            ws.replace_and_mark_text_in_range(None, "pinyin", Some(2..2), window, cx);
            assert_eq!(ws.state.editor.to_text(), original);
            assert_eq!(ws.state.editor.revision, 0);
            assert_eq!(ws.marked_text_range(window, cx), Some(4..10));
            assert_eq!(
                ws.selected_text_range(false, window, cx).unwrap().range,
                6..6
            );
        })
    });
    draw(cx);
    assert!(view.read_with(cx, |ws, _| ws
        .source_display_lines()
        .join("\n")
        .contains("pinyin")));
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.replace_text_in_range(None, "中文", window, cx)
        })
    });
    draw(cx);
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "你好🎉中文 end"
    );
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.revision), 1);
    cx.simulate_keystrokes("secondary-z");
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        original
    );
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.replace_and_mark_text_in_range(None, "cancel", None, window, cx);
            ws.unmark_text(window, cx);
            assert_eq!(ws.state.editor.to_text(), original);
            assert!(ws.source_composition.is_none());
        })
    });
}

#[gpui::test]
fn native_source_ime_explicit_ranges_and_invalid_surrogates_are_atomic(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = source_window(cx, "你好😀 tail");
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.state.editor.set_cursor(0, 3);
            ws.replace_and_mark_text_in_range(None, "😀文", Some(1..1), window, cx);
            assert!(
                ws.source_composition.is_none(),
                "a half-surrogate selection is rejected"
            );
            ws.replace_and_mark_text_in_range(None, "pinyin", None, window, cx);
            ws.replace_and_mark_text_in_range(Some(6..8), "中", Some(1..1), window, cx);
            assert_eq!(ws.source_composition.as_ref().unwrap().text, "pi中in");
            assert_eq!(ws.state.editor.to_text(), "你好😀 tail");
            let before = ws.state.editor.clone();
            ws.replace_text_in_range(Some(999..1000), "bad", window, cx);
            assert_eq!(ws.state.editor, before);
            assert!(ws.source_composition.is_some());
            ws.replace_text_in_range(None, "中文", window, cx);
            assert_eq!(ws.state.editor.to_text(), "你好😀中文 tail");
            assert_eq!(ws.state.editor.revision, 1);
        })
    });
    draw(cx);
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            let bounds = ws.editor_content_bounds.unwrap();
            assert!(!ws
                .selection_bounds_for_range(0..6, bounds, window, cx)
                .is_empty());
            assert!(
                ws.bounds_for_range(4..6, bounds, window, cx)
                    .unwrap()
                    .size
                    .width
                    > gpui::px(2.)
            );
        })
    });
}

#[gpui::test]
fn native_source_ime_selection_stays_in_preedit_and_is_painted(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "a😀 tail");
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.state.editor.set_cursor(0, 2);
            ws.replace_and_mark_text_in_range(None, "拼音😀", Some(0..2), window, cx);
            ws.set_selected_text_range(5..7, window, cx);
            assert!(
                ws.source_composition.is_some(),
                "IME selection must not cancel its preedit"
            );
            assert_eq!(
                ws.selected_text_range(false, window, cx).unwrap().range,
                5..7
            );
            assert_eq!(ws.state.editor.to_text(), "a😀 tail");
            assert_eq!(ws.state.editor.revision, 0);
        })
    });
    draw(cx);
    assert!(
        cx.debug_bounds("source_selection_0_4_5").is_some(),
        "selected preedit uses display character offsets"
    );
    assert!(
        cx.debug_bounds("source_marked_0_2_5").is_some(),
        "the entire preedit is marked, not only the caret"
    );
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.set_selected_text_range(6..6, window, cx);
            assert_eq!(
                ws.selected_text_range(false, window, cx).unwrap().range,
                5..7,
                "half-surrogate selection is rejected"
            );
            ws.replace_text_in_range(None, "中文", window, cx);
            assert_eq!(ws.state.editor.to_text(), "a😀中文 tail");
        })
    });
}

#[gpui::test]
fn native_source_column_ime_does_not_mutate_other_rows_until_commit(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "ab\ncd\nef");
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.state.editor.set_column_ranges(vec![1..2, 4..5, 7..8]);
            ws.replace_and_mark_text_in_range(None, "pinyin", None, window, cx);
            assert_eq!(ws.state.editor.to_text(), "ab\ncd\nef");
            assert_eq!(ws.state.editor.revision, 0);
            assert_eq!(ws.source_display_lines().join("\n"), "ab\ncd\nepinyin");
            ws.replace_text_in_range(None, "中文", window, cx);
            assert_eq!(ws.state.editor.to_text(), "a中文\nc中文\ne中文");
            assert_eq!(ws.state.editor.revision, 1);
        })
    });
    draw(cx);
    cx.simulate_input("!");
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "a中文!\nc中文!\ne中文!"
    );
    cx.simulate_keystrokes("secondary-z secondary-z");
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "ab\ncd\nef"
    );
}

#[gpui::test]
fn native_source_shift_drag_preserves_its_original_anchor(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "alpha\nbeta\ngamma");
    let anchor = caret_point(&view, 1, 2, cx);
    cx.simulate_mouse_down(anchor, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(anchor, MouseButton::Left, Modifiers::none());
    let top = caret_point(&view, 0, 1, cx);
    let bottom = caret_point(&view, 2, 3, cx);
    let shift = Modifiers {
        shift: true,
        ..Modifiers::none()
    };
    cx.simulate_mouse_down(top, MouseButton::Left, shift);
    cx.simulate_mouse_move(bottom, Some(MouseButton::Left), shift);
    cx.simulate_mouse_up(bottom, MouseButton::Left, shift);
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selected_text()),
        Some("ta\ngam".into())
    );
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selection_anchor_offset()),
        Some(8)
    );
    draw(cx);
    let row = caret_point(&view, 2, 2, cx);
    cx.simulate_event(gpui::MouseDownEvent {
        button: MouseButton::Left,
        position: row,
        modifiers: Modifiers::none(),
        click_count: 3,
        first_mouse: false,
    });
    cx.simulate_mouse_up(row, MouseButton::Left, Modifiers::none());
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selected_text()),
        Some("gamma".into())
    );
}

#[gpui::test]
fn native_source_middle_drag_and_repeated_column_input(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "abc\ndef\nghi");
    let first = caret_point(&view, 0, 1, cx);
    let last = caret_point(&view, 2, 2, cx);
    cx.simulate_mouse_down(first, MouseButton::Middle, Modifiers::none());
    cx.simulate_mouse_move(last, Some(MouseButton::Middle), Modifiers::none());
    cx.simulate_mouse_up(last, MouseButton::Middle, Modifiers::none());
    draw(cx);
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selected_text()),
        Some("b\ne\nh".into())
    );
    cx.simulate_input("X");
    cx.simulate_input("中");
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "aX中c\ndX中f\ngX中i"
    );
    let before = view.read_with(cx, |ws, _| ws.state.editor.clone());
    cx.update(|window, cx| {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("1\n2".into()));
        view.update(cx, |ws, cx| {
            ws.source_command(SourceCommand::Paste, window, cx)
        });
    });
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.clone()), before);
    assert!(view.read_with(cx, |ws, _| ws.error_banner.is_some()));
}

#[gpui::test]
fn native_source_multiclick_shift_click_and_context_actions(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "alpha 中文\nbeta");
    let word = caret_point(&view, 0, 7, cx);
    cx.simulate_event(gpui::MouseDownEvent {
        button: MouseButton::Left,
        position: word,
        modifiers: Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_mouse_up(word, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selected_text()),
        Some("中文".into())
    );
    let selected = view.read_with(cx, |ws, _| ws.state.editor.selection());
    cx.simulate_mouse_down(word, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(word, MouseButton::Right, Modifiers::none());
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selection()),
        selected
    );
    cx.update(|window, cx| window.dispatch_action(Box::new(zmark_app::actions::SourceCopy), cx));
    cx.run_until_parked();
    cx.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some("中文")
        )
    });
    cx.update(|window, cx| window.dispatch_action(Box::new(zmark_app::actions::SourceCut), cx));
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "alpha \nbeta"
    );
    cx.simulate_keystrokes("secondary-z");
    draw(cx);
    let first = caret_point(&view, 0, 2, cx);
    let last = caret_point(&view, 1, 2, cx);
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(1));
    cx.simulate_mouse_down(first, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(first, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_down(
        last,
        MouseButton::Left,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    cx.simulate_mouse_up(
        last,
        MouseButton::Left,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selected_text()),
        Some("pha 中文\nbe".into())
    );
}

#[gpui::test]
fn native_source_columns_use_visual_rows_and_clip_short_or_empty_rows(
    cx: &mut gpui::TestAppContext,
) {
    let original = format!("{}\n\nx", "a".repeat(210));
    let (view, cx) = source_window(cx, &original);
    let (first, last) = cx.update(|_, cx| {
        let ws = view.read(cx);
        let rows = ws.source_rows.lock().unwrap();
        let first = rows.iter().find(|row| row.index == 0).unwrap();
        let last = rows
            .iter()
            .find(|row| row.visual.physical_line_idx == 2)
            .unwrap();
        (
            gpui::point(
                first.bounds.left() + gpui::px(first.positions[5]),
                first.bounds.center().y,
            ),
            gpui::point(
                first.bounds.left() + gpui::px(first.positions[7]),
                last.bounds.center().y,
            ),
        )
    });
    let layout = view.read_with(cx, |ws, _| ws.cached_visual_lines.clone());
    cx.simulate_mouse_down(first, MouseButton::Middle, Modifiers::none());
    cx.simulate_mouse_move(last, Some(MouseButton::Middle), Modifiers::none());
    draw(cx);
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selected_text()),
        Some("aa\naa\n\n".into()),
        "visual rows: {:?}",
        view.read_with(cx, |ws, _| ws.cached_visual_lines.clone())
    );
    let cached = view.read_with(cx, |ws, _| {
        ws.source_drag.as_ref().unwrap().column_positions.len()
    });
    cx.simulate_mouse_move(last, Some(MouseButton::Middle), Modifiers::none());
    draw(cx);
    assert_eq!(
        view.read_with(cx, |ws, _| ws
            .source_drag
            .as_ref()
            .unwrap()
            .column_positions
            .len()),
        cached
    );
    assert!(
        view.read_with(cx, |ws, _| std::sync::Arc::ptr_eq(
            &layout,
            &ws.cached_visual_lines
        )),
        "selection movement must not rewrap the document"
    );
    cx.simulate_mouse_up(last, MouseButton::Middle, Modifiers::none());
    cx.simulate_input("Z");
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.revision), 1);
    assert!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text().ends_with("\nZ\nxZ")),
        "short rows never gain alignment spaces"
    );
    cx.simulate_keystrokes("secondary-z");
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        original
    );
}

#[gpui::test]
fn native_source_parked_pointer_autoscroll_uses_repainted_rows(cx: &mut gpui::TestAppContext) {
    let content = (0..150)
        .map(|index| format!("line {index}: 中文 abcdefghijklmnopqrstuvwxyz"))
        .collect::<Vec<_>>()
        .join("\n");
    let (view, cx) = source_window(cx, &content);
    let first = caret_point(&view, 0, 2, cx);
    let bounds = view.read_with(cx, |ws, _| ws.editor_content_bounds.unwrap());
    let edge = gpui::point(first.x, bounds.bottom() - gpui::px(2.));
    cx.simulate_mouse_down(first, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(edge, Some(MouseButton::Left), Modifiers::none());
    draw(cx);
    let anchor = view.read_with(cx, |ws, _| ws.state.editor.selection().unwrap().0);
    for _ in 0..20 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(17));
        draw(cx);
    }
    let (cursor, expected, offset, active) = cx.update(|window, cx| {
        let ws = view.read(cx);
        let (_, line, col) = ws.hit_source_point(edge, window).unwrap();
        (
            ws.state.editor.cursor(),
            (line, col),
            ws.editor_scroll_handle.0.borrow().base_handle.offset().y,
            ws.source_autoscroll_task.is_some(),
        )
    });
    assert!(
        offset < gpui::px(-20.),
        "parked pointer must keep scrolling"
    );
    assert_eq!(
        cursor, expected,
        "head must use the actual post-scroll character"
    );
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selection().unwrap().0),
        anchor
    );
    assert!(active);
    cx.simulate_mouse_up(edge, MouseButton::Left, Modifiers::none());
    assert!(
        view.read_with(cx, |ws, _| ws.source_autoscroll_task.is_none()
            && ws.source_drag.is_none())
    );
    let stopped = view.read_with(cx, |ws, _| {
        ws.editor_scroll_handle.0.borrow().base_handle.offset()
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(100));
    draw(cx);
    assert_eq!(
        view.read_with(cx, |ws, _| ws
            .editor_scroll_handle
            .0
            .borrow()
            .base_handle
            .offset()),
        stopped
    );
}

#[gpui::test]
fn native_source_click_after_preedit_rehits_original_text_geometry(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "alpha beta");
    let original_point = caret_point(&view, 0, 8, cx);
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.state.editor.set_cursor(0, 2);
            ws.replace_and_mark_text_in_range(None, "LONGPREEDIT", None, window, cx);
        })
    });
    draw(cx);
    cx.simulate_mouse_down(original_point, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(original_point, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "alpha beta"
    );
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.cursor()), (0, 8));
    assert!(view.read_with(cx, |ws, _| ws.source_composition.is_none()
        && ws.source_drag.is_none()));
}

#[gpui::test]
fn native_source_composition_cancels_on_document_mode_and_focus_changes(
    cx: &mut gpui::TestAppContext,
) {
    let (view, cx) = source_window(cx, "original");
    let directory = tempfile::tempdir().unwrap();
    let original_path = directory.path().join("original.md");
    cx.update(|_, cx| {
        view.update(cx, |ws, _| {
            let key = ws.state.active_key().unwrap().clone();
            ws.state.save_doc_as(&key, original_path.clone()).unwrap();
        })
    });
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.replace_and_mark_text_in_range(None, "pending", None, window, cx);
            ws.set_view_mode(ViewMode::Preview, cx);
        })
    });
    draw(cx);
    assert!(view.read_with(cx, |ws, _| ws.source_composition.is_none()));
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "original"
    );
    cx.update(|_, cx| view.update(cx, |ws, cx| ws.set_view_mode(ViewMode::Split, cx)));
    draw(cx);
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            window.focus(&ws.focus_handle, cx);
            ws.replace_and_mark_text_in_range(None, "pending", None, window, cx);
        })
    });
    cx.simulate_keystrokes("secondary-f");
    draw(cx);
    cx.update(|window, cx| {
        let ws = view.read(cx);
        assert!(
            ws.source_composition.is_none(),
            "real search focus cancels composition: drawer={}, source_focus={}, search_focus={}",
            ws.state.drawer_open,
            ws.focus_handle.is_focused(window),
            ws.search_input
                .as_ref()
                .is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window))
        );
    });
    let before = view.read_with(cx, |ws, _| ws.state.editor.clone());
    cx.simulate_input("search text");
    cx.simulate_keystrokes("secondary-z secondary-b");
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.clone()), before);
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            window.focus(&ws.focus_handle, cx);
            ws.replace_and_mark_text_in_range(None, "pending", None, window, cx);
            ws.begin_rename(window, cx);
        })
    });
    draw(cx);
    assert!(
        view.read_with(cx, |ws, _| ws.source_composition.is_none()),
        "native rename focus cancels composition"
    );
    cx.simulate_input("temporary name");
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.clone()), before);
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            window.focus(&ws.focus_handle, cx);
            ws.replace_and_mark_text_in_range(None, "pending", None, window, cx);
        })
    });
    cx.simulate_keystrokes("secondary-n");
    draw(cx);
    assert!(
        view.read_with(cx, |ws, _| ws.source_composition.is_none()),
        "document swap cancels composition"
    );
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.to_text()), "");
    cx.simulate_input("new document");
    cx.simulate_keystrokes("secondary-w");
    draw(cx);
    assert!(view.read_with(cx, |ws, _| ws.state.pending_close.is_some()));
    let before = view.read_with(cx, |ws, _| ws.state.editor.clone());
    // S/D deliberately activate modal buttons; exercise ordinary input without dismissing it.
    cx.update(|window, cx| {
        view.update(cx, |ws, cx| {
            ws.replace_text_in_range(None, "must not edit modal", window, cx)
        })
    });
    cx.simulate_input("xyz");
    cx.simulate_keystrokes("secondary-z secondary-b");
    assert!(view.read_with(cx, |ws, _| ws.state.pending_close.is_some()));
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.clone()), before);
    assert_eq!(std::fs::read_to_string(&original_path).unwrap(), "original");
}

#[gpui::test]
fn native_source_format_actions_and_hidden_source_isolation(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "alpha\nbeta");
    cx.simulate_keystrokes("secondary-a secondary-b");
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "**alpha\nbeta**"
    );
    cx.simulate_keystrokes("secondary-b");
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "alpha\nbeta"
    );
    cx.update(|_, cx| assert!(!view.read(cx).source_menu().is_empty()));
    cx.simulate_keystrokes("secondary-shift-7");
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.to_text()),
        "1. alpha\n2. beta"
    );
    cx.update(|_, cx| view.update(cx, |ws, cx| ws.set_view_mode(ViewMode::Preview, cx)));
    draw(cx);
    let before = view.read_with(cx, |ws, _| ws.state.editor.clone());
    cx.simulate_input("must not edit");
    cx.simulate_keystrokes("secondary-x secondary-v secondary-b secondary-z secondary-shift-z");
    assert_eq!(view.read_with(cx, |ws, _| ws.state.editor.clone()), before);
}

#[gpui::test]
fn source_press_in_padding_or_below_text_moves_the_caret_and_clears_the_selection(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "alpha beta\ngamma delta");
    // Select a range by dragging inside the text.
    let first = caret_point(&view, 0, 1, cx);
    let last = caret_point(&view, 1, 4, cx);
    cx.simulate_mouse_down(first, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(last, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(last, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert!(view.read_with(cx, |ws, _| ws.state.editor.selected_text().is_some()));

    // Press in the empty space far below the last line: the selection must go away.
    let bounds = view.read_with(cx, |ws, _| ws.editor_content_bounds).expect("editor bounds");
    let below = gpui::point(bounds.left() + gpui::px(200.), bounds.bottom() - gpui::px(6.));
    cx.simulate_mouse_down(below, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(below, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selected_text()),
        None,
        "a press below the text must collapse the selection"
    );
}

#[gpui::test]
fn source_drag_started_in_the_left_padding_selects_text(cx: &mut gpui::TestAppContext) {
    let (view, cx) = source_window(cx, "alpha beta\ngamma delta");
    let bounds = view.read_with(cx, |ws, _| ws.editor_content_bounds).expect("editor bounds");
    let start = gpui::point(bounds.left() + gpui::px(2.), caret_point(&view, 0, 0, cx).y);
    let end = caret_point(&view, 1, 5, cx);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    draw(cx);
    assert_eq!(
        view.read_with(cx, |ws, _| ws.state.editor.selected_text()),
        Some("alpha beta\ngamma".into()),
        "dragging out of the padding selects from the nearest text"
    );
}
