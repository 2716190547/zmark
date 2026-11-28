use gpui::prelude::*;
use gpui::*;
use zmark_core::ast::{BlockElement, InlineGroup, InlineItem, ParsedDocument};
use zmark_core::i18n::I18nStrings;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::theme::ThemeColors;

mod selection;
mod mermaid;
mod virtual_list;
mod measured_blocks;
pub use selection::*;
pub use virtual_list::*;

pub type OnCopyCallback = Arc<dyn Fn(String, u64, &mut Window, &mut App) + Send + Sync>;
/// Marks a lightbox request for a local image snapshot (as opposed to a Mermaid diagram,
/// whose callback hash is the diagram source hash).
pub const LOCAL_IMAGE_LIGHTBOX_HASH: u64 = 0;

pub type OnOpenLightboxCallback = Arc<dyn Fn(std::path::PathBuf, u64, Option<(f32, f32)>, &mut Window, &mut App) + Send + Sync>;
pub type OnPreviewSelectionCallback = Arc<dyn Fn(Option<PreviewSelection>, bool, &mut Window, &mut App) + Send + Sync>;
/// Toggles a task item: marker source range, current checked state.
pub type OnToggleTaskCallback = Arc<dyn Fn(std::ops::Range<usize>, bool, &mut Window, &mut App) + Send + Sync>;
pub type OnImageRecoveryCallback = Arc<dyn Fn(std::ops::Range<usize>, String, crate::local_images::RecoveryCandidate, &mut Window, &mut App) + Send + Sync>;
pub type OnSelectHeadingCallback = Arc<dyn Fn(usize, &mut Window, &mut App) + Send + Sync>;
pub type OnPreviewAutoscrollCallback = Arc<dyn Fn(f32, &mut Window, &mut App) + Send + Sync>;
/// A press on block `ix`: rendered char offset, click count, window position.
pub type OnBlockPressCallback = Arc<dyn Fn(usize, usize, usize, Point<Pixels>, &mut Window, &mut App) + Send + Sync>;
pub type OnPreviewWheelCallback = Arc<dyn Fn(&ScrollEvent, &mut Window, &mut App) + Send + Sync>;

#[derive(Clone, Default)]
pub struct PreviewCallbacks {
    pub on_copy: Option<OnCopyCallback>,
    pub on_open_lightbox: Option<OnOpenLightboxCallback>,
    pub on_selection_change: Option<OnPreviewSelectionCallback>,
    pub on_autoscroll: Option<OnPreviewAutoscrollCallback>,
    pub on_image_recovery: Option<OnImageRecoveryCallback>,
    pub on_toggle_task: Option<OnToggleTaskCallback>,
    pub on_select_heading: Option<OnSelectHeadingCallback>,
    pub on_wheel: Option<OnPreviewWheelCallback>,
    /// Replaces the plain selection start on block presses (block editing).
    pub on_block_press: Option<OnBlockPressCallback>,
}

pub fn render_preview_pane_with_callbacks(
    parsed: Arc<ParsedDocument>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    max_column_width: f32,
    font_size: f32,
    scroll_state: &PreviewScrollState,
    visible_range: std::ops::Range<usize>,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    callbacks: PreviewCallbacks,
    selection: Option<PreviewSelection>,
    is_selecting: bool,
    regions: Arc<RegionsRegistry>,
    syntax_cache: Arc<std::sync::Mutex<zmark_core::syntax::SyntaxCache>>,
    failures: &[crate::syntax::VisibleSyntaxFailure],
    images: crate::local_images::SharedImages,
    reduce_motion: bool,
    block_editor: Option<crate::views::workspace::BlockEditorSlot>,
) -> impl IntoElement {
    let theme = theme.clone();
    let item_theme = theme.clone();
    let item_parsed = parsed.clone();
    let item_callbacks = callbacks.clone();
    let item_regions = regions.clone();
    let item_syntax_cache = syntax_cache.clone();
    let measurement_sink = scroll_state.measurement_sink();
    let sizes = scroll_state.sizes();
    let origins = scroll_state.origins();
    let total = scroll_state.total_height();
    let scroll_handle = scroll_state.scroll_handle().clone();
    let pane_width = f32::from(scroll_handle.bounds().size.width);
    let media_width = if pane_width > 1.0 { pane_width - 64.0 } else { max_column_width }
        .min(max_column_width).max(1.0);
    let wheel_cb = callbacks.on_wheel.clone();
    regions.lock().unwrap().clear();

    let mut pane = div()
        .flex_1()
        .w_full()
        .min_w(px(0.))
        .min_h(px(0.))
        .h_full()
        .flex()
        .flex_col()
        .bg(theme.surface);

    if let Some(ref on_selection_change) = callbacks.on_selection_change {
        let on_sel_up = on_selection_change.clone();
        pane = pane.on_mouse_up(MouseButton::Left, move |_event, window, cx| {
            if is_selecting {
                on_sel_up(selection, false, window, cx);
            }
        });

        // Blocks only handle presses inside their own box. A press anywhere else in the pane
        // (side margins, gaps between blocks, empty space) is handled here: it clears the old
        // selection and starts a new one from the nearest text, like a browser or Notes.
        // Block handlers run first and call `stop_propagation`, so this never double-handles.
        let on_sel_down = on_selection_change.clone();
        let on_sel_move = on_selection_change.clone();
        let regions_down = regions.clone();
        let regions_move = regions.clone();
        let parsed_down = parsed.clone();
        let parsed_move = parsed.clone();
        pane = pane
            .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
                let pos = {
                    let reg = regions_down.lock().unwrap();
                    hit_test_point_across_all_regions(&reg, window.text_system(), event.position, &parsed_down.blocks)
                };
                match pos {
                    Some(pos) => on_sel_down(Some(PreviewSelection::new(pos, pos)), true, window, cx),
                    None => on_sel_down(None, false, window, cx),
                }
            })
            .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                if !is_selecting {
                    return;
                }
                if event.pressed_button != Some(MouseButton::Left) {
                    on_sel_move(selection, false, window, cx);
                    return;
                }
                let Some(sel) = selection else { return };
                let head = {
                    let reg = regions_move.lock().unwrap();
                    hit_test_point_across_all_regions(&reg, window.text_system(), event.position, &parsed_move.blocks)
                };
                if let Some(head) = head {
                    on_sel_move(Some(PreviewSelection::new(sel.anchor, head)), true, window, cx);
                }
            });
    }

    let has_toc = item_parsed
        .blocks
        .iter()
        .any(|b| matches!(b, BlockElement::TableOfContents));
    let toc_headings: Vec<zmark_core::ast::HeadingItem> = if has_toc {
        item_parsed.headings()
    } else {
        Vec::new()
    };

    let item_toc = toc_headings;
    let item_images = images.clone();
    let build_block = move |ix: usize| -> Option<AnyElement> {
        if let Some(slot) = block_editor.as_ref().filter(|slot| slot.blocks.contains(&ix)) {
            // Later blocks the editor covers measure as empty so the size table stays true.
            return Some(if ix == slot.blocks.start { (slot.build)() } else { div().into_any_element() });
        }
        let Some(block) = item_parsed.blocks.get(ix) else { return None; };
        let flat = zmark_core::ast::flatten_block(block, "0");
        let block_char_len = flat.text.chars().count();
        let selected_char_range = selection.and_then(|s| s.char_range_for_block(ix, block_char_len));

        let block_bounds_cell = Arc::new(std::cell::Cell::new(Bounds::default()));
        let bounds_for_canvas = block_bounds_cell.clone();
        let block_container = div()
            .relative()
            .w_full()
            .max_w(px(max_column_width))
            .mx_auto()
            .child(
                canvas(
                    move |bounds, _window, _cx| bounds_for_canvas.set(bounds),
                    |_bounds, _, _window, _cx| {},
                )
                .absolute()
                .size_full(),
            )
            .child(match block {
                BlockElement::TableOfContents => render_toc_block(
                    &item_toc,
                    item_callbacks.on_select_heading.as_ref(),
                    &item_theme,
                    strings,
                    ix,
                    font_size,
                ),
                _ => render_block_element(
                    block,
                    &item_theme,
                    strings,
                    font_size,
                    copied_code_hash,
                    item_callbacks.on_copy.as_ref(),
                    item_callbacks.on_open_lightbox.as_ref(),
                    item_callbacks.on_image_recovery.as_ref(),
                    item_callbacks.on_toggle_task.as_ref(),
                    selected_char_range,
                    ix,
                    item_regions.clone(),
                    item_syntax_cache.clone(),
                    &item_images,
                    &item_parsed.code_blocks,
                    &flat,
                    "0",
                    media_width,
                    reduce_motion,
                ),
            });

        // Positioned and measured by `MeasuredBlocks` in the same frame.
        let mut item = div()
            .w_full()
            .px_8()
            .pb_4()
            .child(block_container);

        if let Some(ref on_selection_change) = item_callbacks.on_selection_change {
            let on_sel_down = on_selection_change.clone();
            let on_sel_move = on_selection_change.clone();
            let on_autoscroll = item_callbacks.on_autoscroll.clone();
            let on_block_press = item_callbacks.on_block_press.clone();
            let text_for_down: Arc<str> = Arc::from(flat.text.as_str());
            let text_for_move = Arc::clone(&text_for_down);
            let bounds_cell_down = block_bounds_cell.clone();
            let bounds_cell_move = block_bounds_cell.clone();
            let regions_down = item_regions.clone();
            let regions_move = item_regions.clone();

            item = item
                .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
                    let text_system = window.text_system();
                    let offset = {
                        let reg = regions_down.lock().unwrap();
                        hit_test_char_offset_in_block(
                            text_system,
                            &reg,
                            ix,
                            &text_for_down,
                            event.position,
                            font_size,
                            bounds_cell_down.get(),
                        )
                    };
                    cx.stop_propagation();
                    if let Some(ref press) = on_block_press {
                        press(ix, offset, event.click_count, event.position, window, cx);
                        return;
                    }
                    let pos = PreviewCharPosition::new(ix, offset);
                    on_sel_down(Some(PreviewSelection::new(pos, pos)), true, window, cx);
                })
                .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                    if is_selecting {
                        if event.pressed_button != Some(MouseButton::Left) {
                            on_sel_move(selection, false, window, cx);
                            return;
                        }
                        // This block owns the drag while the pointer is over it; the pane-level
                        // fallback must not recompute (and overwrite) the head.
                        cx.stop_propagation();
                        let text_system = window.text_system();
                        let offset = {
                            let reg = regions_move.lock().unwrap();
                            hit_test_char_offset_in_block(
                                text_system,
                                &reg,
                                ix,
                                &text_for_move,
                                event.position,
                                font_size,
                                bounds_cell_move.get(),
                            )
                        };
                        let head = PreviewCharPosition::new(ix, offset);
                        if let Some(sel) = selection {
                            on_sel_move(Some(PreviewSelection::new(sel.anchor, head)), true, window, cx);
                        }
                        if let Some(ref auto_cb) = on_autoscroll {
                            let mouse_y = f32::from(event.position.y);
                            if let Some(vel) = detect_edge_autoscroll(mouse_y, 0.0, 800.0) {
                                auto_cb(vel, window, cx);
                            }
                        }
                    }
                });
        }

        Some(item.into_any_element())
    };
    let content = measured_blocks::MeasuredBlocks::new(
        visible_range,
        origins,
        sizes,
        total,
        measurement_sink,
        Box::new(build_block),
    );

    let mut scroll_view = div()
        .id("preview_scroll_view")
        .flex_1()
        .w_full()
        .min_w(px(0.))
        .min_h(px(0.))
        .h_full()
        .overflow_y_scroll()
        .track_scroll(&scroll_handle)
        .child(content);

    if let Some(cb) = wheel_cb {
        scroll_view = scroll_view.on_scroll(move |event: &ScrollEvent, window, cx| {
            cb(event, window, cx);
        });
    }

    pane = pane.child(scroll_view);

    if !failures.is_empty() {
        let first = &failures[0];
        let badge_text = if failures.len() == 1 {
            format!("L{} {} · {}", first.source_line, first.reason, strings.syntax_plain_readable)
        } else {
            format!("L{} {} · {} (+{})", first.source_line, first.reason, strings.syntax_plain_readable, failures.len() - 1)
        };
        let tooltip_text = failures
            .iter()
            .map(|f| format!("L{} {} · {}", f.source_line, f.reason, strings.syntax_plain_readable))
            .collect::<Vec<_>>()
            .join("\n");

        let on_sel_down = callbacks.on_selection_change.clone();
        let on_sel_move = callbacks.on_selection_change.clone();
        let regions_down = regions.clone();
        let regions_move = regions.clone();
        let parsed_down = parsed.clone();
        let parsed_move = parsed.clone();

        pane = pane.child(
            div()
                .absolute()
                .bottom_3()
                .right_5()
                .flex()
                .items_center()
                .gap_1p5()
                .px_2()
                .py_1()
                .rounded_full()
                .bg(theme.bg)
                .border_1()
                .border_color(theme.border)
                .text_size(px(10.))
                .font_family(crate::platform::ui_font())
                .text_color(theme.text_muted)
                .id("preview_syntax_failure_badge")
                .debug_selector(|| "preview_syntax_failure_badge".into())
                .tooltip(move |_window, cx| {
                    let t = tooltip_text.clone();
                    cx.new(|_| CodeTooltipView { text: t }).into()
                })
                .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
                    if let Some(ref sel_cb) = on_sel_down {
                        let text_system = window.text_system();
                        let reg = regions_down.lock().unwrap();
                        if let Some(pos) = hit_test_point_across_all_regions(&reg, text_system, event.position, &parsed_down.blocks) {
                            sel_cb(Some(PreviewSelection::new(pos, pos)), true, window, cx);
                        }
                    }
                })
                .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                    if is_selecting {
                        if let Some(ref sel_cb) = on_sel_move {
                            if event.pressed_button != Some(MouseButton::Left) {
                                sel_cb(selection, false, window, cx);
                                return;
                            }
                            if let Some(sel) = selection {
                                let text_system = window.text_system();
                                let reg = regions_move.lock().unwrap();
                                if let Some(head) = hit_test_point_across_all_regions(&reg, text_system, event.position, &parsed_move.blocks) {
                                    sel_cb(Some(PreviewSelection::new(sel.anchor, head)), true, window, cx);
                                }
                            }
                        }
                    }
                })
                .child(badge_text),
        );
    }

    pane
}

/// Narrowest a table column may shrink to before the table scrolls sideways.
const TABLE_MIN_CELL_WIDTH: f32 = 72.0;

fn visual_text_units(text: &str) -> f32 {
    text.chars()
        .map(|ch| if ch.is_ascii() { 1.0 } else { 1.85 })
        .sum::<f32>()
}

/// Computes `(min_width_px, flex_grow_weight)` per column and total `min_table_width`.
fn compute_table_column_layout(
    headers: &[InlineGroup],
    rows: &[Vec<InlineGroup>],
) -> (Vec<(f32, f32)>, f32) {
    let cols = headers
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or(0))
        .max(1);
    let mut max_units = vec![0.0_f32; cols];
    for (idx, h) in headers.iter().enumerate() {
        max_units[idx] = max_units[idx].max(visual_text_units(&h.plain_text()));
    }
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            if idx < cols {
                max_units[idx] = max_units[idx].max(visual_text_units(&cell.plain_text()));
            }
        }
    }

    let layout: Vec<(f32, f32)> = max_units
        .into_iter()
        .map(|units| {
            let min_w = 48.0 + (units.min(18.0) / 18.0) * 40.0;
            let weight = units.clamp(3.0, 48.0).powf(0.75);
            (min_w, weight)
        })
        .collect();
    let total_min_w: f32 = layout.iter().map(|(min_w, _)| *min_w).sum();
    (layout, total_min_w.max(TABLE_MIN_CELL_WIDTH))
}

fn table_cell_box(el: gpui::Div, min_w: f32, weight: f32) -> gpui::Div {
    let mut el = el.min_w(px(min_w));
    el.style().flex_grow = Some(weight);
    el.style().flex_shrink = Some(1.0);
    el.style().flex_basis = Some(Length::Definite(px(0.).into()));
    el
}

fn render_toc_block(
    headings: &[zmark_core::ast::HeadingItem],
    on_select_heading: Option<&OnSelectHeadingCallback>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    block_ix: usize,
    font_size: f32,
) -> AnyElement {
    // One step below body text, never smaller than the sidebar's 13px rows.
    let toc_size = px((font_size - 2.0).max(13.0));
    let min_level = headings.iter().map(|h| h.level).min().unwrap_or(1);
    let mut card = div()
        .id(SharedString::from(format!("preview_toc_{block_ix}")))
        .w_full()
        .min_w(px(0.))
        .my_2()
        .px_4()
        .py_3()
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .bg(theme.blockquote_bg())
        .flex()
        .flex_col()
        .gap_1p5()
        .child(
            div()
                .flex()
                .items_center()
                .gap_1p5()
                .text_size(toc_size)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.text_muted)
                .child(svg().path("icons/list.svg").size(px(13.)).text_color(theme.text_muted))
                .child(strings.tab_outline),
        );

    if headings.is_empty() {
        card = card.child(
            div()
                .text_size(toc_size)
                .text_color(theme.text_muted)
                .child(strings.outline_empty),
        );
    } else {
        card = card.children(headings.iter().enumerate().map(|(idx, heading)| {
            let indent = (heading.level.saturating_sub(min_level)) as f32 * 14.0;
            let is_top = heading.level <= min_level;
            let text_color = if is_top {
                theme.text_primary
            } else {
                theme.text_muted
            };
            let target_block = heading.block_index;
            let mut row = div()
                .id(SharedString::from(format!("preview_toc_item_{block_ix}_{idx}")))
                .pl(px(6.0 + indent))
                .pr_2()
                .py_1()
                .rounded(px(5.))
                .text_size(toc_size)
                .text_color(text_color)
                .when(is_top, |el| el.font_weight(FontWeight::MEDIUM))
                .hover(|s| s.bg(theme.hover_bg()).text_color(theme.text_primary))
                .child(heading.text.clone());

            if let Some(cb) = on_select_heading {
                let cb = cb.clone();
                row = row.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    move |_, window, cx| {
                        cx.stop_propagation();
                        cb(target_block, window, cx);
                    },
                );
            }
            row
        }));
    }

    card.into_any_element()
}

/// Top/bottom margins for heading levels (top ≈ 2–3× bottom).
pub fn heading_spacing(level: u8) -> (f32, f32) {
    match level {
        1 => (32.0, 12.0),
        2 => (24.0, 8.0),
        3 => (18.0, 6.0),
        _ => (12.0, 4.0),
    }
}

fn intersect_selection(
    selected: Option<&std::ops::Range<usize>>,
    start: usize,
    len: usize,
) -> Option<std::ops::Range<usize>> {
    let range = selected?;
    let end = start.saturating_add(len);
    if range.start >= end || range.end <= start {
        return None;
    }
    let local_start = range.start.saturating_sub(start).min(len);
    let local_end = range.end.saturating_sub(start).min(len);
    (local_start < local_end).then_some(local_start..local_end)
}

fn render_block_element(
    block: &BlockElement,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    base_font_size: f32,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<&OnCopyCallback>,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
    on_image_recovery: Option<&OnImageRecoveryCallback>,
    on_toggle_task: Option<&OnToggleTaskCallback>,
    selected_char_range: Option<std::ops::Range<usize>>,
    block_ix: usize,
    regions: Arc<RegionsRegistry>,
    syntax_cache: Arc<std::sync::Mutex<zmark_core::syntax::SyntaxCache>>,
    images: &crate::local_images::SharedImages,
    code_blocks: &[zmark_core::ast::ParsedCodeBlock],
    flat: &zmark_core::ast::FlattenResult,
    path: &str,
    media_width: f32,
    reduce_motion: bool,
) -> AnyElement {
    match block {
        BlockElement::Heading { level, content } => {
            let (size_mult, border_bottom) = match level {
                1 => (1.8, true),
                2 => (1.45, true),
                3 => (1.25, false),
                _ => (1.1, false),
            };
            let (pt, pb) = heading_spacing(*level);

            let font_size = base_font_size * size_mult;
            let mut el = div()
                .w_full()
                .min_w(px(0.))
                .text_size(px(font_size))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_primary)
                .pt(px(pt))
                .pb(px(pb));

            if border_bottom {
                el = el.border_b_1().border_color(theme.heading_rule());
            }

            let seg = flat.segments.iter().find(|s| s.child_path == path);
            let flat_char_offset = seg.map(|s| s.char_range.start).unwrap_or(0);
            let el_range = seg.and_then(|s| {
                intersect_selection(selected_char_range.as_ref(), s.char_range.start, s.char_range.len())
            });

            el = el.child(render_styled_inlines(content, theme, font_size, el_range, images, on_image_recovery, on_open_lightbox, reduce_motion));

            with_text_probe(
                el,
                TextRegionInfo {
                    block_ix,
                    text: content.plain_text(),
                    font_name: crate::platform::ui_font(),
                    font_size,
                    line_height_factor: 1.75,
                    padding_left: 0.0,
                    padding_top: pt,
                    padding_right: 0.0,
                    flat_char_offset,
                    content_key: None,
                    fence_open_line: None,
                },
                regions,
            )
            .into_any_element()
        }
        BlockElement::Paragraph(content) => {
            let seg = flat.segments.iter().find(|s| s.child_path == path);
            let flat_char_offset = seg.map(|s| s.char_range.start).unwrap_or(0);
            let el_range = seg.and_then(|s| {
                intersect_selection(selected_char_range.as_ref(), s.char_range.start, s.char_range.len())
            });

            let el = div()
                .w_full()
                .min_w(px(0.))
                .text_size(px(base_font_size))
                .line_height(relative(crate::platform::preview_line_height()))
                .text_color(theme.text_primary)
                .child(render_styled_inlines(content, theme, base_font_size, el_range, images, on_image_recovery, on_open_lightbox, reduce_motion));

            with_text_probe(
                el,
                TextRegionInfo {
                    block_ix,
                    text: content.plain_text(),
                    font_name: crate::platform::ui_font(),
                    font_size: base_font_size,
                    line_height_factor: crate::platform::preview_line_height(),
                    padding_left: 0.0,
                    padding_top: 0.0,
                    padding_right: 0.0,
                    flat_char_offset,
                    content_key: None,
                    fence_open_line: None,
                },
                regions,
            )
            .into_any_element()
        }
        BlockElement::FencedCode { lang, code } => {
            if lang.as_deref() == Some("mermaid") {
                {
                    let hash = {
                        use std::hash::{Hash, Hasher};
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        code.hash(&mut hasher);
                        hasher.finish()
                    };
                    fade_in_card(
                        render_mermaid_diagram(code, theme, strings, base_font_size, on_open_lightbox, media_width),
                        reduce_motion,
                        format!("mermaid_fade:{hash:016x}:{}", theme.is_dark),
                    )
                }
            } else {
                let seg = flat.segments.iter().find(|s| s.child_path == path);
                let flat_char_offset = seg.map(|s| s.char_range.start).unwrap_or(0);
                let el_range = seg.and_then(|s| {
                    intersect_selection(selected_char_range.as_ref(), s.char_range.start, s.char_range.len())
                });

                let occurrence_path = format!("{}{}", block_ix, &path[1..]);
                render_code_block(
                    lang.as_deref(),
                    code,
                    theme,
                    strings,
                    base_font_size,
                    copied_code_hash,
                    on_copy,
                    el_range,
                    block_ix,
                    flat_char_offset,
                    regions,
                    &syntax_cache,
                    code_blocks.iter().find(|b| b.child_path == occurrence_path),
                    reduce_motion,
                )
            }
        }
        BlockElement::BlockQuote(blocks) => {
            let mut child_elements = Vec::new();

            for (i, b) in blocks.iter().enumerate() {
                let child_path = format!("{}.{}", path, i);
                child_elements.push(render_block_element(
                    b,
                    theme,
                    strings,
                    base_font_size,
                    copied_code_hash,
                    on_copy,
                    on_open_lightbox,
                    on_image_recovery,
                    on_toggle_task,
                    selected_char_range.clone(),
                    block_ix,
                    regions.clone(),
                    syntax_cache.clone(),
                    images,
                    code_blocks,
                    flat,
                    &child_path,
                    media_width - 28.0,
                    reduce_motion,
                ));
            }

            div()
                .w_full()
                .min_w(px(0.))
                .border_l(px(3.))
                .border_color(theme.blockquote_border())
                .bg(theme.blockquote_bg())
                .pl(px(14.))
                .pr(px(12.))
                .py(px(10.))
                .rounded_r_md()
                .opacity(0.88)
                .flex()
                .flex_col()
                .gap_2()
                .children(child_elements)
                .into_any_element()
        }
        BlockElement::List { ordered, items } => render_list_element(
            *ordered,
            items,
            theme,
            strings,
            base_font_size,
            0,
            copied_code_hash,
            on_copy,
            on_open_lightbox,
            on_image_recovery,
            on_toggle_task,
            selected_char_range,
            block_ix,
            regions,
            syntax_cache.clone(),
            images,
            code_blocks,
            flat,
            path,
            media_width,
            reduce_motion,
        ),
        BlockElement::Table { headers, rows } => {
            let (col_layout, min_table_w) = compute_table_column_layout(headers, rows);
            let mut cur_offset = 0;
            let mut header_cells = Vec::new();
            for (col_idx, h) in headers.iter().enumerate() {
                let (min_w, weight) = col_layout
                    .get(col_idx)
                    .copied()
                    .unwrap_or((TABLE_MIN_CELL_WIDTH, 1.0));
                let text = h.plain_text();
                let len = text.chars().count();
                let cell_start = cur_offset;
                let cell_range = intersect_selection(selected_char_range.as_ref(), cell_start, len);
                cur_offset += len + 1;
                header_cells.push(
                    with_text_probe(
                        table_cell_box(div(), min_w, weight)
                            .p_3()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(base_font_size * 0.95))
                            .text_color(theme.text_primary)
                            .child(render_styled_inlines(h, theme, base_font_size * 0.95, cell_range, images, on_image_recovery, on_open_lightbox, reduce_motion)),
                        TextRegionInfo {
                            block_ix,
                            text: text.clone(),
                            font_name: crate::platform::ui_font(),
                            font_size: base_font_size * 0.95,
                            line_height_factor: 1.75,
                            padding_left: 12.0,
                            padding_top: 12.0,
                            padding_right: 12.0,
                            flat_char_offset: cell_start,
                            content_key: None,
                            fence_open_line: None,
                        },
                        regions.clone(),
                    )
                );
            }

            let mut row_cells = Vec::new();
            for row in rows {
                let mut cells = Vec::new();
                for (col_idx, cell) in row.iter().enumerate() {
                    let (min_w, weight) = col_layout
                        .get(col_idx)
                        .copied()
                        .unwrap_or((TABLE_MIN_CELL_WIDTH, 1.0));
                    let text = cell.plain_text();
                    let len = text.chars().count();
                    let cell_start = cur_offset;
                    let cell_range = intersect_selection(selected_char_range.as_ref(), cell_start, len);
                    cur_offset += len + 1;
                    cells.push(
                        with_text_probe(
                            table_cell_box(div(), min_w, weight)
                                .p_3()
                                .text_size(px(base_font_size * 0.95))
                                .text_color(theme.text_primary)
                                .child(render_styled_inlines(cell, theme, base_font_size * 0.95, cell_range, images, on_image_recovery, on_open_lightbox, reduce_motion)),
                            TextRegionInfo {
                                block_ix,
                                text: text.clone(),
                                font_name: crate::platform::ui_font(),
                                font_size: base_font_size * 0.95,
                                line_height_factor: 1.75,
                                padding_left: 12.0,
                                padding_top: 12.0,
                                padding_right: 12.0,
                                flat_char_offset: cell_start,
                                content_key: None,
                                fence_open_line: None,
                            },
                            regions.clone(),
                        )
                    );
                }
                row_cells.push(
                    div()
                        .border_b_1()
                        .border_color(theme.table_row_border())
                        .flex()
                        .children(cells),
                );
            }

            div()
                .id("preview_table_scroll")
                .w_full()
                .min_w(px(0.))
                .overflow_x_scroll().restrict_scroll_to_axis()
                .my_2()
                .child(
                    div()
                        .w_full()
                        // Fit the pane and wrap cell text; only tables with many
                        // columns exceed it and scroll horizontally.
                        .min_w(px(min_table_w))
                        .rounded_md()
                        .border_1()
                        .border_color(theme.table_border())
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .bg(theme.table_header_bg())
                                .border_b_1()
                                .border_color(theme.table_border())
                                .flex()
                                .children(header_cells),
                        )
                        .children(row_cells),
                )
                .into_any_element()
        }
        BlockElement::TableOfContents => div().into_any_element(),
        BlockElement::Spacer { lines } => div()
            .w_full()
            .h(px((*lines as f32) * 14.0))
            .into_any_element(),
        BlockElement::Rule => div()
            .w_full()
            .my_4()
            .border_b_1()
            .border_color(theme.border)
            .into_any_element(),
    }
}

fn render_styled_inlines(
    group: &InlineGroup,
    theme: &ThemeColors,
    font_size: f32,
    selected_char_range: Option<std::ops::Range<usize>>,
    images: &crate::local_images::SharedImages,
    on_image_recovery: Option<&OnImageRecoveryCallback>,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
    reduce_motion: bool,
) -> AnyElement {
    if !group.0.iter().any(|item| matches!(item, InlineItem::Image { .. })) {
        return render_styled_text_only(group, theme, font_size, selected_char_range).into_any_element();
    }
    let mut offset = 0;
    div().w_full().min_w(px(0.)).flex().flex_col().gap_2()
        .children(group.0.iter().map(|item| {
            let single = InlineGroup(vec![item.clone()]);
            let len = single.plain_text().chars().count();
            let local_selection = selected_char_range.as_ref().and_then(|range| {
                let start = range.start.saturating_sub(offset).min(len);
                let end = range.end.saturating_sub(offset).min(len);
                (start < end).then_some(start..end)
            });
            offset += len;
            match item {
                InlineItem::Image { alt, url, source_range, .. } => {
                    let (state, known_size) = {
                        let mut images = images.lock().unwrap();
                        (images.request(url), images.known_size(url))
                    };
                    match state {
                        crate::local_images::ImageState::Ready(image) => {
                            let reference = url.clone();
                            let (ratio, max_display_width) = image_display_box(image.width, image.height);
                            let element = img(image.image_source())
                                .absolute().inset_0().size_full()
                                .object_fit(ObjectFit::Contain)
                                .debug_selector(|| format!("local_image:{url}"))
                                .with_fallback(move || div().child(format!("图片显示失败 · {reference}")).into_any_element());
                            let description = if alt.is_empty() { url.clone() } else { format!("{alt} · {url}") };
                            let snapshot = image.path().to_path_buf();
                            let natural_size = image.natural_size;
                            let mut container = div().id(SharedString::from(format!("local_image_container:{url}")))
                                .debug_selector(|| format!("local_image_container:{url}"))
                                .relative().w_full().max_w(px(max_display_width))
                                .max_h(px(max_display_width / ratio)).min_w(px(0.))
                                .tooltip(move |_, cx| cx.new(|_| CodeTooltipView { text: description.clone() }).into());
                            // Percentage-sized Img otherwise falls back to intrinsic SVG height.
                            container.style().aspect_ratio = Some(ratio);
                            if let Some(open) = on_open_lightbox {
                                let open = open.clone();
                                container = container.cursor(CursorStyle::PointingHand).on_mouse_down(
                                    MouseButton::Left,
                                    move |_, window, cx| {
                                        cx.stop_propagation();
                                        open(snapshot.clone(), LOCAL_IMAGE_LIGHTBOX_HASH, Some(natural_size), window, cx)
                                    },
                                );
                            }
                            // Fade in once when the decoded image arrives instead of popping in.
                            crate::views::motion::animated(
                                container.child(element),
                                reduce_motion,
                                format!("local_image_fade:{url}"),
                                220,
                                |el, delta| el.opacity(delta),
                            )
                        }
                        crate::local_images::ImageState::RecoveryCandidates(candidates) => {
                            let mut status = div().w_full().min_w(px(0.)).p_3().rounded_md().border_1()
                                .border_color(theme.border).text_color(theme.text_muted)
                                .child(format!("{alt} · Windows 图片路径无法直接使用 · {url}"));
                            if candidates.is_empty() || source_range.is_none() || on_image_recovery.is_none() {
                                status = status.child("请手动修复图片路径");
                            } else {
                                for candidate in candidates {
                                    let callback = on_image_recovery.unwrap().clone();
                                    let range = source_range.clone().unwrap();
                                    let reference = url.clone();
                                    let label = candidate.relative.to_string_lossy().into_owned();
                                    status = status.child(div().id(SharedString::from(format!("recover_image:{}:{label}", range.start)))
                                        .debug_selector(|| format!("recover_image:{}:{label}", range.start))
                                        .cursor_pointer().text_color(theme.accent)
                                        .child(format!("使用 {label}"))
                                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                            cx.stop_propagation();
                                            callback(range.clone(), reference.clone(), candidate.clone(), window, cx);
                                        }));
                                }
                            }
                            status.into_any_element()
                        }
                        state => {
                            if matches!(state, crate::local_images::ImageState::Waiting) {
                                // Decoded before: reserve exactly the box the image will take,
                                // so nothing below moves when it arrives again.
                                if let Some((width, height)) = known_size {
                                    let (ratio, max_display_width) = image_display_box(width, height);
                                    let mut placeholder = div().relative().w_full().max_w(px(max_display_width))
                                        .max_h(px(max_display_width / ratio)).min_w(px(0.))
                                        .rounded_md().bg(theme.hover_bg())
                                        .debug_selector(|| format!("local_image_status:{url}"));
                                    placeholder.style().aspect_ratio = Some(ratio);
                                    return placeholder.into_any_element();
                                }
                                // Loading: reserve the space with a quiet block instead of flashing
                                // a text box that the image then replaces (the "stutter").
                                return div().w_full().min_w(px(0.)).h(px(LOADING_IMAGE_HEIGHT))
                                    .rounded_md().bg(theme.hover_bg())
                                    .debug_selector(|| format!("local_image_status:{url}"))
                                    .into_any_element();
                            }
                            let reason = match state {
                                crate::local_images::ImageState::Failed(reason) => reason,
                                _ => String::new(),
                            };
                            div().w_full().min_w(px(0.)).p_3().rounded_md().border_1()
                                .border_color(theme.border).text_color(theme.text_muted)
                                .debug_selector(|| format!("local_image_status:{url}"))
                                .child(format!("{alt} · {reason} · {url}")).into_any_element()
                        }
                    }
                }
                _ => render_styled_text_only(&single, theme, font_size, local_selection).into_any_element(),
            }
        })).into_any_element()
}

pub fn render_styled_text_only(
    group: &InlineGroup,
    theme: &ThemeColors,
    _font_size: f32,
    selected_char_range: Option<std::ops::Range<usize>>,
) -> StyledText {
    let mut text = String::new();
    let mut highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();

    for item in &group.0 {
        let start = text.len();
        match item {
            InlineItem::Text(s) => {
                text.push_str(s);
            }
            InlineItem::Bold(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        font_weight: Some(FontWeight::BOLD),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Italic(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        font_style: Some(FontStyle::Italic),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Strikethrough(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        color: Some(theme.text_muted),
                        strikethrough: Some(StrikethroughStyle {
                            thickness: px(1.),
                            color: Some(theme.text_muted),
                        }),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Code(s) => {
                text.push_str(s);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        color: Some(theme.syntax_inline_code),
                        background_color: Some(theme.code_bg),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Link { text: link_text, .. } => {
                text.push_str(link_text);
                let end = text.len();
                if start < end {
                    highlights.push((start..end, HighlightStyle {
                        color: Some(theme.syntax_link),
                        underline: Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(theme.syntax_link),
                            wavy: false,
                        }),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Image { alt, .. } => {
                text.push_str(alt);
            }
            InlineItem::LineBreak => {
                text.push('\n');
            }
        }
    }

    #[cfg(feature = "test-observation")]
    obs::log_highlight(&text, &selected_char_range);

    if let Some(char_range) = selected_char_range {
        let byte_range = char_range_to_byte_range(&text, char_range);
        highlights = apply_system_selection_to_highlights(&text, &highlights, byte_range, theme.selection_bg(), theme.text_selection_foreground());
    }

    if text.is_empty() {
        text.push(' ');
    }

    StyledText::new(text).with_highlights(highlights)
}

pub mod obs {
    use gpui::{Bounds, HighlightStyle, Pixels};
    #[cfg(feature = "test-observation")]
    use std::sync::{Mutex, OnceLock};

    #[cfg(feature = "test-observation")]
    type BoundsLog = Mutex<Vec<(String, Bounds<Pixels>)>>;
    #[cfg(feature = "test-observation")]
    type RangeLog = Mutex<Vec<(String, Option<std::ops::Range<usize>>)>>;
    #[cfg(feature = "test-observation")]
    type CodeHighlightsLog = Mutex<Vec<(String, Vec<(std::ops::Range<usize>, HighlightStyle)>)>>;
    #[cfg(feature = "test-observation")]
    type EditorTokensLog = Mutex<Vec<(usize, Vec<crate::views::editor::MarkdownToken>)>>;

    #[cfg(feature = "test-observation")]
    fn bounds_log() -> &'static BoundsLog {
        static L: OnceLock<BoundsLog> = OnceLock::new();
        L.get_or_init(Default::default)
    }
    #[cfg(feature = "test-observation")]
    fn range_log() -> &'static RangeLog {
        static L: OnceLock<RangeLog> = OnceLock::new();
        L.get_or_init(Default::default)
    }
    #[cfg(feature = "test-observation")]
    fn code_highlights_log() -> &'static CodeHighlightsLog {
        static L: OnceLock<CodeHighlightsLog> = OnceLock::new();
        L.get_or_init(Default::default)
    }
    #[cfg(feature = "test-observation")]
    fn editor_tokens_log() -> &'static EditorTokensLog {
        static L: OnceLock<EditorTokensLog> = OnceLock::new();
        L.get_or_init(Default::default)
    }

    #[allow(dead_code)]
    #[cfg(feature = "test-observation")]
    pub fn reset() {
        bounds_log().lock().unwrap().clear();
        range_log().lock().unwrap().clear();
        code_highlights_log().lock().unwrap().clear();
        editor_tokens_log().lock().unwrap().clear();
    }
    #[allow(dead_code)]
    #[cfg(not(feature = "test-observation"))]
    #[inline(always)]
    pub fn reset() {}

    #[allow(dead_code)]
    #[cfg(feature = "test-observation")]
    pub fn text_bounds(key: &str) -> Option<(f32, f32, f32, f32)> {
        bounds_log().lock().unwrap().iter().rev().find(|(k, _)| k == key).map(|(_, b)| {
            (
                f32::from(b.origin.x),
                f32::from(b.origin.y),
                f32::from(b.size.width),
                f32::from(b.size.height),
            )
        })
    }
    #[allow(dead_code)]
    #[cfg(not(feature = "test-observation"))]
    #[inline(always)]
    pub fn text_bounds(_key: &str) -> Option<(f32, f32, f32, f32)> {
        None
    }

    #[allow(dead_code)]
    #[cfg(feature = "test-observation")]
    pub fn highlights() -> Vec<(String, Option<std::ops::Range<usize>>)> {
        range_log().lock().unwrap().clone()
    }
    #[allow(dead_code)]
    #[cfg(not(feature = "test-observation"))]
    #[inline(always)]
    pub fn highlights() -> Vec<(String, Option<std::ops::Range<usize>>)> {
        Vec::new()
    }

    #[allow(dead_code)]
    #[cfg(feature = "test-observation")]
    pub fn last_code_highlights() -> Option<Vec<(std::ops::Range<usize>, HighlightStyle)>> {
        code_highlights_log().lock().unwrap().last().map(|(_, h)| h.clone())
    }
    #[allow(dead_code)]
    #[cfg(not(feature = "test-observation"))]
    #[inline(always)]
    pub fn last_code_highlights() -> Option<Vec<(std::ops::Range<usize>, HighlightStyle)>> {
        None
    }

    #[allow(dead_code)]
    #[cfg(feature = "test-observation")]
    pub fn editor_tokens(phys_idx: usize) -> Option<Vec<crate::views::editor::MarkdownToken>> {
        editor_tokens_log().lock().unwrap().iter().rev().find(|(idx, _)| *idx == phys_idx).map(|(_, t)| t.clone())
    }
    #[allow(dead_code)]
    #[cfg(not(feature = "test-observation"))]
    #[inline(always)]
    pub fn editor_tokens(_phys_idx: usize) -> Option<Vec<crate::views::editor::MarkdownToken>> {
        None
    }

    #[allow(dead_code)]
    #[cfg(feature = "test-observation")]
    pub fn last_editor_tokens() -> Option<Vec<crate::views::editor::MarkdownToken>> {
        editor_tokens_log().lock().unwrap().last().map(|(_, t)| t.clone())
    }
    #[allow(dead_code)]
    #[cfg(not(feature = "test-observation"))]
    #[inline(always)]
    pub fn last_editor_tokens() -> Option<Vec<crate::views::editor::MarkdownToken>> {
        None
    }

    #[allow(dead_code)]
    #[cfg(feature = "test-observation")]
    pub(crate) fn log_bounds(key: &str, bounds: Bounds<Pixels>) {
        bounds_log().lock().unwrap().push((key.to_string(), bounds));
    }
    #[allow(dead_code)]
    #[cfg(not(feature = "test-observation"))]
    #[inline(always)]
    pub(crate) fn log_bounds(_key: &str, _bounds: Bounds<Pixels>) {}

    #[allow(dead_code)]
    #[cfg(feature = "test-observation")]
    pub(crate) fn log_highlight(text: &str, range: &Option<std::ops::Range<usize>>) {
        range_log().lock().unwrap().push((text.to_string(), range.clone()));
    }
    #[allow(dead_code)]
    #[cfg(not(feature = "test-observation"))]
    #[inline(always)]
    pub(crate) fn log_highlight(_text: &str, _range: &Option<std::ops::Range<usize>>) {}

    #[allow(dead_code)]
    #[cfg(feature = "test-observation")]
    pub(crate) fn log_code_highlights(code: &str, highlights: &[(std::ops::Range<usize>, HighlightStyle)]) {
        code_highlights_log().lock().unwrap().push((code.to_string(), highlights.to_vec()));
    }
    #[allow(dead_code)]
    #[cfg(not(feature = "test-observation"))]
    #[inline(always)]
    pub(crate) fn log_code_highlights(_code: &str, _highlights: &[(std::ops::Range<usize>, HighlightStyle)]) {}

    #[allow(dead_code)]
    #[cfg(feature = "test-observation")]
    pub(crate) fn log_editor_tokens(phys_idx: usize, tokens: &[crate::views::editor::MarkdownToken]) {
        editor_tokens_log().lock().unwrap().push((phys_idx, tokens.to_vec()));
    }
    #[allow(dead_code)]
    #[cfg(not(feature = "test-observation"))]
    #[inline(always)]
    pub(crate) fn log_editor_tokens(_phys_idx: usize, _tokens: &[crate::views::editor::MarkdownToken]) {}
}

/// Preview diagrams are stored as rendered SVG textures. Keep enough around
/// for the current viewport and a little scroll-back, but never the whole
/// document: one 1000pt diagram can be ~14 MB of RGBA at GPUI's 2x raster.
pub const MAX_DIAGRAM_CACHE: usize = 16;
/// Upper bound for the decoded CPU textures retained by the preview cache.
/// GPUI also keeps a matching GPU atlas texture while the asset is loaded.
pub const MAX_DIAGRAM_CACHE_BYTES: usize = 32 * 1024 * 1024;

struct DiagramCacheEntry {
    path: std::path::PathBuf,
    bytes: usize,
    /// Frame in which the preview last asked for this diagram.
    used_frame: u64,
    image_requested: bool,
}

pub struct DiagramCache {
    entries: std::collections::HashMap<(u64, bool), DiagramCacheEntry>,
    order: std::collections::VecDeque<(u64, bool)>,
    evicted: Vec<std::path::PathBuf>,
    max_size: usize,
    max_bytes: usize,
    bytes: usize,
    frame: u64,
}

impl DiagramCache {
    pub fn new(max_size: usize) -> Self {
        Self::with_byte_budget(max_size, MAX_DIAGRAM_CACHE_BYTES)
    }

    pub fn with_byte_budget(max_size: usize, max_bytes: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
            evicted: Vec::new(),
            max_size,
            max_bytes,
            bytes: 0,
            frame: 0,
        }
    }

    /// Starts a new preview frame. Diagrams requested during a frame are
    /// visible in it and are never evicted until a later frame leaves them out.
    pub fn begin_frame(&mut self) {
        self.frame += 1;
    }

    pub fn get(&mut self, key: &(u64, bool)) -> Option<std::path::PathBuf> {
        let frame = self.frame;
        let entry = self.entries.get_mut(key)?;
        entry.used_frame = frame;
        entry.image_requested = true;
        let path = entry.path.clone();
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
            self.order.push_back(*key);
        }
        Some(path)
    }

    pub fn insert(&mut self, key: (u64, bool), path: std::path::PathBuf) {
        let bytes = diagram_texture_bytes(&path);
        self.insert_sized(key, path, bytes);
    }

    fn insert_sized(&mut self, key: (u64, bool), path: std::path::PathBuf, bytes: usize) {
        let frame = self.frame;
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.used_frame = frame;
            entry.image_requested = true;
            if entry.path != path {
                let old_path = std::mem::replace(&mut entry.path, path);
                let _ = std::fs::remove_file(&old_path);
                self.evicted.push(old_path);
            }
            self.bytes = self.bytes.saturating_sub(entry.bytes);
            entry.bytes = bytes;
            self.bytes += bytes;
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
            self.order.push_back(key);
        } else {
            self.entries.insert(key, DiagramCacheEntry { path, bytes, used_frame: frame, image_requested: true });
            self.order.push_back(key);
            self.bytes += bytes;
        }
    }

    /// Called after layout/paint has marked every visible diagram. The lightbox
    /// may still need an offscreen preview SVG as its source for sharp re-renders.
    pub fn finish_frame(&mut self, pinned: Option<&std::path::Path>) {
        if let Some(path) = pinned {
            for entry in self.entries.values_mut().filter(|entry| entry.path == path) {
                entry.used_frame = self.frame;
            }
        }
        self.evict_overflow();
    }

    /// Release decoded assets once, retaining the cheaper SVG and its layout cache.
    pub fn take_hidden_images(&mut self, pinned: Option<&std::path::Path>) -> Vec<std::path::PathBuf> {
        let frame = self.frame;
        self.entries.values_mut().filter_map(|entry| {
            if !entry.image_requested || entry.used_frame == frame || pinned == Some(entry.path.as_path()) {
                return None;
            }
            entry.image_requested = false;
            Some(entry.path.clone())
        }).collect()
    }

    fn evict_overflow(&mut self) {
        // Diagrams used in the current frame are on screen: evicting them would
        // delete the file the frame is about to draw and re-render it next
        // frame, forever. The cache may exceed its budget while they are visible
        // and shrinks as soon as they scroll out.
        while self.entries.len() > self.max_size || self.bytes > self.max_bytes {
            let frame = self.frame;
            let Some(pos) = self
                .order
                .iter()
                .position(|key| self.entries.get(key).is_some_and(|e| e.used_frame != frame))
            else {
                break;
            };
            let Some(evicted_key) = self.order.remove(pos) else { break; };
            let Some(entry) = self.entries.remove(&evicted_key) else { continue; };
            self.bytes = self.bytes.saturating_sub(entry.bytes);
            let _ = std::fs::remove_file(&entry.path);
            self.evicted.push(entry.path);
        }
    }

    /// Paths evicted since the last call. The caller must release their GPUI
    /// `RenderImage` assets; removing the file alone leaves textures resident.
    pub fn take_evicted(&mut self) -> Vec<std::path::PathBuf> {
        std::mem::take(&mut self.evicted)
    }

    pub fn has_pending_release(&self) -> bool {
        !self.evicted.is_empty()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// GPUI's image loader rasterizes SVG at `SMOOTH_SVG_SCALE_FACTOR = 2.0`.
fn diagram_texture_bytes(path: &std::path::Path) -> usize {
    let (width, height) = std::fs::read_to_string(path)
        .map(|svg| zmark_core::mermaid::extract_svg_dimensions(&svg))
        .unwrap_or((1000.0, 600.0));
    let scale = zmark_core::mermaid::GPUI_SVG_DECODER_SCALE;
    let width = (width.max(1.0) * scale).ceil().min(16_384.0) as u64;
    let height = (height.max(1.0) * scale).ceil().min(16_384.0) as u64;
    width
        .saturating_mul(height)
        .saturating_mul(4)
        .min(usize::MAX as u64) as usize
}

static DIAGRAM_CACHE: std::sync::LazyLock<std::sync::Mutex<DiagramCache>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(DiagramCache::new(MAX_DIAGRAM_CACHE)));

/// Drop the GPU/CPU image assets for diagrams evicted from the preview cache.
pub fn release_evicted_diagram_images(window: &mut Window, cx: &mut App) -> bool {
    let paths = {
        let mut cache = DIAGRAM_CACHE.lock().unwrap();
        cache.begin_frame();
        cache.take_evicted()
    };
    mermaid::retire(&paths, window, cx);
    if paths.is_empty() {
        return false;
    }
    for path in paths {
        crate::views::lightbox::release_lightbox_image(&path, window, cx);
    }
    true
}

pub fn themed_diagram_path(code_hash: u64, is_dark: bool, window: &mut Window, cx: &mut App) -> Option<std::path::PathBuf> {
    mermaid::themed_path(code_hash, is_dark, window, cx)
}

pub fn cached_diagram_path(code_hash: u64, is_dark: bool) -> Option<std::path::PathBuf> {
    DIAGRAM_CACHE.lock().unwrap().get(&(code_hash, is_dark))
}

/// Use GPUI's post-frame callback: RenderOnce marks visibility after Workspace::render.
pub fn finish_diagram_frame(pinned: Option<&std::path::Path>) -> bool {
    let mut cache = DIAGRAM_CACHE.lock().unwrap();
    cache.finish_frame(pinned);
    cache.has_pending_release()
}

/// Call after paint has marked visible diagrams, including a pinned lightbox source.
pub fn release_hidden_diagram_images(pinned: Option<&std::path::Path>, window: &mut Window, cx: &mut App) -> bool {
    let paths = DIAGRAM_CACHE.lock().unwrap().take_hidden_images(pinned);
    let released = !paths.is_empty();
    mermaid::release_images(&paths, window, cx);
    for path in paths {
        crate::views::lightbox::release_lightbox_image(&path, window, cx);
    }
    released
}

pub fn diagram_cache_has_pending_release() -> bool {
    DIAGRAM_CACHE.lock().unwrap().has_pending_release()
}

fn render_mermaid_container(
    path: std::path::PathBuf,
    image: Option<Arc<RenderImage>>,
    display: (f32, f32),
    code_hash: u64,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
) -> AnyElement {
    let mut container = div()
        .id(("mermaid_diagram_container", code_hash))
        .w_full()
        .min_w(px(0.))
        .my_3()
        .rounded_xl()
        .border_1()
        .border_color(theme.border)
        .bg(theme.surface)
        .p_4()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .relative()
        .overflow_x_scroll().restrict_scroll_to_axis();

    if let Some(cb) = on_open_lightbox {
        let cb = (*cb).clone();
        let p_click = path.clone();
        let cb_btn = cb.clone();
        let p_btn = path.clone();
        let btn = div()
            .id(("mermaid_zoom_btn", code_hash))
            .absolute()
            .top(px(8.))
            .right(px(8.))
            .w(px(28.))
            .h(px(28.))
            .rounded_md()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.control_selection_bg())
            .hover(|s| s.bg(theme.hover_bg()))
            .tooltip({
                let t = strings.mermaid_zoom_hint.to_string();
                move |_window, cx| cx.new(|_| CodeTooltipView { text: t.clone() }).into()
            })
            .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                cx.stop_propagation(); // 防冒泡到外层容器
                cb_btn(p_btn.clone(), code_hash, None, window, cx);
            })
            .child(
                svg()
                    .path("icons/maximize.svg")
                    .size(px(14.))
                    .text_color(theme.selection_text()),
            );

        container = container
            .cursor(CursorStyle::PointingHand)
            .hover(|s| s.border_color(theme.accent))
            .on_mouse_down(MouseButton::Left, move |_event, window, app| {
                app.stop_propagation();
                cb(p_click.clone(), code_hash, None, window, app);
            })
            .child(btn);
    }

    container
        .child(div().w(px(display.0)).h(px(display.1)).when_some(image, |el, image| {
            el.child(img(image).size_full())
        }))
        .into_any_element()
}

/// Opacity fade for diagram/image cards the first time they are laid out.
fn fade_in_card(card: AnyElement, reduce_motion: bool, id: String) -> AnyElement {
    crate::views::motion::animated(
        div().w_full().child(card),
        reduce_motion,
        id,
        220,
        |el, delta| el.opacity(delta),
    )
}

fn render_mermaid_diagram(
    code: &str,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    _base_font_size: f32,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
    media_width: f32,
) -> AnyElement {
    mermaid::Mermaid { code: Arc::from(code), theme: theme.clone(), strings,
        open: on_open_lightbox.cloned(), width: media_width }.into_any_element()
}

struct CodeTooltipView {
    text: String,
}

impl Render for CodeTooltipView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(rgba(0x18181bf2))
            .border_1()
            .border_color(rgba(0x3f3f4640))
            .text_size(px(11.))
            .text_color(rgb(0xf4f4f5))
            .font_family(crate::platform::ui_font())
            .shadow_md()
            .child(self.text.clone())
    }
}

fn render_list_element(
    ordered: bool,
    items: &[zmark_core::ast::ListItem],
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    base_font_size: f32,
    depth: usize,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<&OnCopyCallback>,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
    on_image_recovery: Option<&OnImageRecoveryCallback>,
    on_toggle_task: Option<&OnToggleTaskCallback>,
    selected_char_range: Option<std::ops::Range<usize>>,
    block_ix: usize,
    regions: Arc<RegionsRegistry>,
    syntax_cache: Arc<std::sync::Mutex<zmark_core::syntax::SyntaxCache>>,
    images: &crate::local_images::SharedImages,
    code_blocks: &[zmark_core::ast::ParsedCodeBlock],
    flat: &zmark_core::ast::FlattenResult,
    path: &str,
    media_width: f32,
    reduce_motion: bool,
) -> AnyElement {
    let mut item_elements = Vec::with_capacity(items.len());

    for (idx, item) in items.iter().enumerate() {
        let bullet_element = if let Some(checked) = item.checked {
            let toggle = on_toggle_task.zip(item.marker.clone());
            div()
                .id(SharedString::from(format!("task_box_{block_ix}_{path}_{idx}")))
                .when_some(toggle, |el, (callback, marker)| {
                    let callback = callback.clone();
                    el.cursor_pointer()
                        // Keep the press from starting a preview text selection.
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            callback(marker.clone(), checked, window, cx);
                        })
                })
                .mt_1()
                .w(px(14.))
                .h(px(14.))
                .rounded_sm()
                .border_1()
                .border_color(theme.syntax_bullet)
                .bg(if checked { theme.syntax_bullet } else { theme.bg })
                .flex()
                .items_center()
                .justify_center()
                .child(if checked {
                    svg()
                        .path("icons/check.svg")
                        .size(px(9.))
                        .text_color(rgb(0xffffff))
                        .into_any_element()
                } else {
                    div().into_any_element()
                })
                .into_any_element()
        } else if ordered {
            div()
                .flex_shrink_0()
                .w(px(20.))
                .text_right()
                .text_size(px(base_font_size))
                .font_family(crate::platform::mono_font())
                .text_color(theme.syntax_bullet)
                .child(format!("{}.", idx + 1))
                .into_any_element()
        } else {
            let bullet = match depth % 3 {
                0 => "–",
                1 => "◦",
                _ => "▪",
            };
            div()
                .flex_shrink_0()
                .w(px(12.))
                .text_center()
                .text_size(px(base_font_size))
                .text_color(theme.syntax_bullet)
                .child(bullet)
                .into_any_element()
        };

        let mut first_child_el = None;
        let mut subsequent_child_els = Vec::new();

        for (ci, child) in item.body.iter().enumerate() {
            let child_path = format!("{}.{}.{}", path, idx, ci);
            let seg = flat.segments.iter().find(|s| s.child_path == child_path);
            let flat_char_offset = seg.map(|s| s.char_range.start).unwrap_or(0);
            let child_range = seg.and_then(|s| {
                intersect_selection(selected_char_range.as_ref(), s.char_range.start, s.char_range.len())
            });

            if ci == 0 {
                if let zmark_core::ast::BlockElement::Paragraph(content) = child {
                    let content_text = content.plain_text();
                    first_child_el = Some(
                        with_text_probe(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(px(base_font_size))
                                .line_height(relative(crate::platform::preview_list_line_height()))
                                .text_color(theme.text_primary)
                                .child(render_styled_inlines(content, theme, base_font_size, child_range, images, on_image_recovery, on_open_lightbox, reduce_motion)),
                            TextRegionInfo {
                                block_ix,
                                text: content_text,
                                font_name: crate::platform::ui_font(),
                                font_size: base_font_size,
                                line_height_factor: crate::platform::preview_list_line_height(),
                                padding_left: 0.0,
                                padding_top: 0.0,
                                padding_right: 0.0,
                                flat_char_offset,
                                content_key: None,
                                fence_open_line: None,
                            },
                            regions.clone(),
                        )
                        .into_any_element(),
                    );
                } else {
                    let rendered = render_block_element(
                        child,
                        theme,
                        strings,
                        base_font_size,
                        copied_code_hash,
                        on_copy,
                        on_open_lightbox,
                        on_image_recovery,
                        on_toggle_task,
                        selected_char_range.clone(),
                        block_ix,
                        regions.clone(),
                        syntax_cache.clone(),
                        images,
                        code_blocks,
                        flat,
                        &child_path,
                    media_width - 28.0,
                    reduce_motion,
                    );
                    first_child_el = Some(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .child(rendered)
                            .into_any_element(),
                    );
                }
            } else {
                let rendered = if let zmark_core::ast::BlockElement::List { ordered: sub_ordered, items: sub_items } = child {
                    render_list_element(
                        *sub_ordered,
                        sub_items,
                        theme,
                        strings,
                        base_font_size,
                        depth + 1,
                        copied_code_hash,
                        on_copy,
                        on_open_lightbox,
                        on_image_recovery,
                        on_toggle_task,
                        selected_char_range.clone(),
                        block_ix,
                        regions.clone(),
                        syntax_cache.clone(),
                        images,
                        code_blocks,
                        flat,
                        &child_path,
                    media_width - 28.0,
                    reduce_motion,
                    )
                } else {
                    render_block_element(
                        child,
                        theme,
                        strings,
                        base_font_size,
                        copied_code_hash,
                        on_copy,
                        on_open_lightbox,
                        on_image_recovery,
                        on_toggle_task,
                        selected_char_range.clone(),
                        block_ix,
                        regions.clone(),
                        syntax_cache.clone(),
                        images,
                        code_blocks,
                        flat,
                        &child_path,
                    media_width - 28.0,
                    reduce_motion,
                    )
                };
                subsequent_child_els.push(
                    div()
                        .pl_5()
                        .pt_1()
                        .child(rendered),
                );
            }
        }

        let first_row = div()
            .w_full()
            .min_w(px(0.))
            .flex()
            .items_start()
            .gap_2()
            .child(bullet_element)
            .children(first_child_el);

        item_elements.push(
            div()
                .w_full()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .child(first_row)
                .children(subsequent_child_els),
        );
    }

    div()
        .w_full()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .gap_1p5()
        .children(item_elements)
        .into_any_element()
}

/// Determines whether a code block should soft-wrap (`whitespace_normal`)
/// or retain horizontal scroll (`overflow_x_scroll().whitespace_nowrap()`).
///
/// Under ADR 0036 §3 specification:
/// - `Some(l)` trimmed and lowercased: non-empty and `!= "text"` returns `true` (soft-wrap).
/// - `None`, `""`, `"text"` (case and whitespace-insensitive) return `false` (retain horizontal scroll for ASCII trees).
/// - Note: Mermaid diagrams do not enter this function as they are rendered
///   as interactive diagram widgets with pan/zoom lightbox capabilities.
/// Upper bound for a single inline image's displayed height, in points.
pub const MAX_IMAGE_DISPLAY_HEIGHT: f32 = 400.0;
/// Height reserved for an image that is still decoding.
const LOADING_IMAGE_HEIGHT: f32 = 120.0;

/// Aspect ratio and maximum display width of an image of `width`×`height` pixels.
/// The displayed height is capped by shrinking the width instead of
/// letterboxing: a tall image must not own a whole viewport (it made the
/// preview blocks huge and left big empty gaps).
fn image_display_box(width: u32, height: u32) -> (f32, f32) {
    let ratio = width as f32 / height.max(1) as f32;
    (ratio, (MAX_IMAGE_DISPLAY_HEIGHT * ratio).min(width as f32))
}

pub fn should_wrap_code_block(lang: Option<&str>) -> bool {
    match lang {
        None => false,
        Some(l) => {
            let s = l.trim().to_lowercase();
            !s.is_empty() && s != "text"
        }
    }
}

fn render_code_block(
    lang: Option<&str>,
    code: &std::sync::Arc<str>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    base_font_size: f32,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<&OnCopyCallback>,
    selected_char_range: Option<std::ops::Range<usize>>,
    block_ix: usize,
    base_char_offset: usize,
    regions: Arc<RegionsRegistry>,
    syntax_cache: &Arc<std::sync::Mutex<zmark_core::syntax::SyntaxCache>>,
    occurrence: Option<&zmark_core::ast::ParsedCodeBlock>,
    reduce_motion: bool,
) -> AnyElement {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.as_ref().hash(&mut hasher);
    let hash = hasher.finish();
    let is_copied = copy_feedback_active(copied_code_hash, hash, std::time::Instant::now());

    let on_copy_cb = on_copy.cloned();
    let code_to_copy = code.clone();
    let code_text = if code.is_empty() { " ".to_string() } else { code.to_string() };

    let canonical_lang = zmark_core::syntax::resolve_canonical_language(lang);
    let key = zmark_core::syntax::CodeKey::new(canonical_lang, code.clone());
    let content_key = Some(key.clone());

    let should_wrap = should_wrap_code_block(lang);

    let group_name: SharedString = format!("code_block_{}", hash).into();

    let spans = occurrence.filter(|b| b.is_valid).and_then(|_| syntax_cache.lock().unwrap().get(&key))
        .unwrap_or_else(|| {
            Arc::from([zmark_core::syntax::SyntaxSpan {
                range: 0..code.len(),
                role: zmark_core::syntax::SyntaxRole::Plain,
            }])
        });
    let syntax_highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = spans
        .iter()
        .filter(|s| s.range.start < s.range.end && s.range.end <= code_text.len())
        .map(|s| {
            (
                s.range.clone(),
                HighlightStyle {
                    color: Some(theme.syntax_color_for_role(s.role)),
                    ..Default::default()
                },
            )
        })
        .collect();

    let highlights = if let Some(char_range) = selected_char_range {
        let byte_range = char_range_to_byte_range(&code_text, char_range);
        if byte_range.start < byte_range.end {
            apply_system_selection_to_highlights(&code_text, &syntax_highlights, byte_range, theme.selection_bg(), theme.text_selection_foreground())
        } else {
            syntax_highlights
        }
    } else {
        syntax_highlights
    };

    #[cfg(feature = "test-observation")]
    obs::log_code_highlights(&code_text, &highlights);

    let styled_text = if highlights.is_empty() {
        StyledText::new(code_text)
    } else {
        StyledText::new(code_text).with_highlights(highlights)
    };

    let code_font_size = base_font_size;
    // Short fence language shown in the block's lower-right corner, e.g. "rust".
    let lang_label: Option<String> = lang
        .and_then(|l| l.split(|c: char| c.is_whitespace() || c == ',').next())
        .map(|l| l.trim().to_lowercase())
        .filter(|l| !l.is_empty() && l != "text");
    let mut content = div()
        .id(SharedString::from(format!("code_content_scroll_{}", hash)))
        .w_full()
        .min_w(px(0.))
        .p_3()
        .text_size(px(code_font_size))
        .font_family(crate::platform::mono_font())
        .line_height(relative(crate::platform::preview_line_height()))
        .text_color(theme.syntax_code)
        .child(styled_text);

    if should_wrap {
        content = content.whitespace_normal();
    } else {
        content = content.overflow_x_scroll().restrict_scroll_to_axis().whitespace_nowrap();
    }

    let content = with_text_probe_stateful(
        content,
        TextRegionInfo {
            block_ix,
            text: code.to_string(),
            font_name: crate::platform::mono_font(),
            font_size: code_font_size,
            line_height_factor: crate::platform::preview_line_height(),
            padding_left: 12.0,
            padding_top: 12.0,
            padding_right: 12.0,
            flat_char_offset: base_char_offset,
            content_key,
            fence_open_line: occurrence.map(|b| b.fence_open_line),
        },
        regions,
    );

    let copy_btn = div()
        .id(SharedString::from(format!("copy_code_btn_{}", hash)))
        .absolute()
        .top(px(8.))
        .right(px(8.))
        .w(px(26.))
        .h(px(26.))
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .opacity(if is_copied { 1.0 } else { 0.0 })
        .group_hover(group_name.clone(), |s| s.opacity(1.0))
        .bg(theme.control_selection_bg())
        .hover(|s| s.bg(theme.hover_bg()))
        .tooltip({
            let is_copied = is_copied;
            move |_window, cx| {
                cx.new(|_| CodeTooltipView {
                    text: if is_copied {
                        strings.copied.to_string()
                    } else {
                        strings.copy_code.to_string()
                    },
                })
                .into()
            }
        })
        .on_click(move |_, window, cx| {
            if let Some(ref cb) = on_copy_cb {
                cb(code_to_copy.to_string(), hash, window, cx);
            }
        })
        .child({
            let icon = svg()
                .path(if is_copied {
                    "icons/check.svg"
                } else {
                    "icons/copy.svg"
                })
                .size(px(13.))
                .text_color(if is_copied {
                    theme.accent
                } else {
                    theme.selection_text()
                });
            if is_copied {
                let occurrence_line = occurrence
                    .map(|block| block.fence_open_line)
                    .unwrap_or(block_ix);
                let copy_instant = copied_code_hash
                    .filter(|(copied_hash, _)| *copied_hash == hash)
                    .map(|(_, instant)| format!("{instant:?}"))
                    .unwrap_or_default();
                crate::views::motion::animated(
                    div().child(icon),
                    reduce_motion,
                    format!("copy_check_{hash}_{occurrence_line}_{copy_instant}"),
                    120,
                    |el, delta| el.opacity(delta),
                )
            } else {
                icon.into_any_element()
            }
        });

    div()
        .id(SharedString::from(format!("code_block_{}", hash)))
        .group(group_name)
        .relative()
        .w_full()
        .min_w(px(0.))
        .rounded_md()
        .bg(theme.code_bg)
        .child(content)
        .child(copy_btn)
        .when_some(lang_label, |el, label| {
            el.child(
                div()
                    .absolute()
                    .bottom(px(6.))
                    .right(px(10.))
                    .text_size(px(11.))
                    .font_family(crate::platform::mono_font())
                    .text_color(theme.text_muted.opacity(0.6))
                    .child(label),
            )
        })
        .into_any_element()
}

fn copy_feedback_active(
    copied: Option<(u64, std::time::Instant)>,
    hash: u64,
    now: std::time::Instant,
) -> bool {
    copied.is_some_and(|(copied_hash, instant)| {
        copied_hash == hash && now.duration_since(instant) < std::time::Duration::from_millis(1200)
    })
}

#[cfg(test)]
mod copy_feedback_tests {
    use super::copy_feedback_active;
    use std::time::{Duration, Instant};

    #[test]
    fn feedback_matches_the_block_and_expires_at_the_boundary() {
        let start = Instant::now();
        assert!(copy_feedback_active(Some((7, start)), 7, start + Duration::from_millis(1199)));
        assert!(!copy_feedback_active(Some((7, start)), 7, start + Duration::from_millis(1200)));
        assert!(!copy_feedback_active(Some((7, start)), 8, start));
        assert!(!copy_feedback_active(None, 7, start));
    }
}

#[cfg(test)]
mod typography_tests {
    use super::{compute_table_column_layout, heading_spacing};
    use zmark_core::ast::{InlineGroup, InlineItem};

    #[test]
    fn heading_spacing_follows_the_reading_scale() {
        assert_eq!(heading_spacing(1), (32.0, 12.0));
        assert_eq!(heading_spacing(2), (24.0, 8.0));
        assert_eq!(heading_spacing(3), (18.0, 6.0));
        assert_eq!(heading_spacing(4), (12.0, 4.0));
        assert_eq!(heading_spacing(6), (12.0, 4.0));
    }

    #[test]
    fn table_column_layout_weights_wide_descriptions_above_narrow_markers() {
        let cell = |s: &str| InlineGroup(vec![InlineItem::Text(s.to_string())]);
        let headers = vec![cell("单词"), cell("词性"), cell("音标"), cell("含义")];
        let rows = vec![vec![
            cell("Response Body"),
            cell("-"),
            cell("/rɪˈspɒns ˈbɒdi/"),
            cell("响应体（Spring MVC 中 @ResponseBody 注解，用于标识控制器方法的返回值）"),
        ]];
        let (layout, total_min) = compute_table_column_layout(&headers, &rows);
        assert_eq!(layout.len(), 4);
        assert!(layout[3].1 > layout[1].1 * 4.0, "meaning column should receive much wider share");
        assert!(layout[1].0 < layout[3].0, "narrow column min width should stay compact");
        assert!(total_min < 350.0, "4-column table should fit split pane without unnecessary x-scroll");
    }
}
