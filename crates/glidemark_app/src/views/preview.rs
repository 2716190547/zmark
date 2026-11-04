use gpui::prelude::*;
use gpui::*;
use glidemark_core::ast::{BlockElement, InlineGroup, InlineItem, ParsedDocument};
use glidemark_core::i18n::I18nStrings;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::theme::ThemeColors;

pub type OnCopyCallback = Arc<dyn Fn(String, u64, &mut Window, &mut App) + Send + Sync>;
pub type OnOpenLightboxCallback = Arc<dyn Fn(std::path::PathBuf, u64, &mut Window, &mut App) + Send + Sync>;
pub type OnPreviewSelectionCallback = Arc<dyn Fn(Option<PreviewSelection>, bool, &mut Window, &mut App) + Send + Sync>;
pub type OnPreviewAutoscrollCallback = Arc<dyn Fn(f32, &mut Window, &mut App) + Send + Sync>;

#[derive(Clone, Default)]
pub struct PreviewCallbacks {
    pub on_copy: Option<OnCopyCallback>,
    pub on_open_lightbox: Option<OnOpenLightboxCallback>,
    pub on_selection_change: Option<OnPreviewSelectionCallback>,
    pub on_autoscroll: Option<OnPreviewAutoscrollCallback>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct PreviewCharPosition {
    pub block_ix: usize,
    pub char_offset: usize,
}

impl PreviewCharPosition {
    pub fn new(block_ix: usize, char_offset: usize) -> Self {
        Self { block_ix, char_offset }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PreviewSelection {
    pub anchor: PreviewCharPosition,
    pub head: PreviewCharPosition,
}

pub type PreviewSelectionRange = PreviewSelection;

impl PreviewSelection {
    pub fn new(anchor: PreviewCharPosition, head: PreviewCharPosition) -> Self {
        Self { anchor, head }
    }

    pub fn normalized(&self) -> (PreviewCharPosition, PreviewCharPosition) {
        if self.head < self.anchor {
            (self.head, self.anchor)
        } else {
            (self.anchor, self.head)
        }
    }

    pub fn char_range_for_block(&self, block_ix: usize, block_len: usize) -> Option<std::ops::Range<usize>> {
        let (start, end) = self.normalized();
        if block_ix < start.block_ix || block_ix > end.block_ix {
            return None;
        }
        let (s_char, e_char) = if start.block_ix == end.block_ix {
            (start.char_offset.min(block_len), end.char_offset.min(block_len))
        } else if block_ix == start.block_ix {
            (start.char_offset.min(block_len), block_len)
        } else if block_ix == end.block_ix {
            (0, end.char_offset.min(block_len))
        } else {
            (0, block_len)
        };
        if s_char >= e_char {
            None
        } else {
            Some(s_char..e_char)
        }
    }
}

pub fn serialize_preview_selection(parsed: &ParsedDocument, range: PreviewSelection) -> String {
    let (start, end) = range.normalized();
    parsed.extract_plain_text_char_range(
        start.block_ix,
        start.char_offset,
        end.block_ix,
        end.char_offset,
    )
}

pub fn char_range_to_byte_range(s: &str, char_range: std::ops::Range<usize>) -> std::ops::Range<usize> {
    if char_range.start >= char_range.end {
        return 0..0;
    }
    let mut byte_start = None;
    let mut byte_end = None;
    let mut current_char = 0;

    for (b_idx, _) in s.char_indices() {
        if current_char == char_range.start {
            byte_start = Some(b_idx);
        }
        if current_char == char_range.end {
            byte_end = Some(b_idx);
            break;
        }
        current_char += 1;
    }

    let start = byte_start.unwrap_or_else(|| if current_char <= char_range.start { s.len() } else { 0 });
    let end = byte_end.unwrap_or(s.len());
    let start = start.min(end);
    start..end
}

pub fn apply_selection_to_highlights(
    text: &str,
    existing_highlights: &[(std::ops::Range<usize>, HighlightStyle)],
    selection_byte_range: std::ops::Range<usize>,
    selection_bg: Hsla,
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    if selection_byte_range.start >= selection_byte_range.end || selection_byte_range.start >= text.len() {
        return existing_highlights.to_vec();
    }

    let sel_start = selection_byte_range.start.min(text.len());
    let sel_end = selection_byte_range.end.min(text.len());

    let mut split_points = Vec::with_capacity(existing_highlights.len() * 2 + 4);
    split_points.push(0);
    split_points.push(text.len());
    split_points.push(sel_start);
    split_points.push(sel_end);

    for (range, _) in existing_highlights {
        split_points.push(range.start.min(text.len()));
        split_points.push(range.end.min(text.len()));
    }

    split_points.sort_unstable();
    split_points.dedup();

    let mut result: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();

    for i in 0..split_points.len().saturating_sub(1) {
        let p_start = split_points[i];
        let p_end = split_points[i + 1];
        if p_start >= p_end {
            continue;
        }

        let base_style = existing_highlights
            .iter()
            .find(|(r, _)| r.start <= p_start && p_end <= r.end)
            .map(|(_, s)| s);

        let is_selected = sel_start <= p_start && p_end <= sel_end;

        let merged_style = match (base_style, is_selected) {
            (None, false) => None,
            (None, true) => Some(HighlightStyle {
                background_color: Some(selection_bg),
                ..Default::default()
            }),
            (Some(s), false) => Some(s.clone()),
            (Some(s), true) => {
                let mut combined = s.clone();
                combined.background_color = Some(selection_bg);
                Some(combined)
            }
        };

        if let Some(style) = merged_style {
            if let Some((prev_range, prev_style)) = result.last_mut() {
                if prev_range.end == p_start && prev_style == &style {
                    prev_range.end = p_end;
                    continue;
                }
            }
            result.push((p_start..p_end, style));
        }
    }

    result
}

pub fn detect_edge_autoscroll(
    mouse_y: f32,
    container_top: f32,
    container_height: f32,
) -> Option<f32> {
    const EDGE_ZONE: f32 = 40.0;
    const MAX_SPEED: f32 = 4.0;
    if mouse_y < container_top + EDGE_ZONE {
        let factor = ((container_top + EDGE_ZONE - mouse_y) / EDGE_ZONE).clamp(0.1, 1.0);
        Some(-factor * MAX_SPEED)
    } else if mouse_y > container_top + container_height - EDGE_ZONE {
        let factor = ((mouse_y - (container_top + container_height - EDGE_ZONE)) / EDGE_ZONE).clamp(0.1, 1.0);
        Some(factor * MAX_SPEED)
    } else {
        None
    }
}

#[derive(Clone, Debug)]
pub struct TextLayoutRegion {
    pub block_ix: usize,
    pub bounds: Bounds<Pixels>,
    pub text: String,
    pub font_name: &'static str,
    pub font_size: f32,
    pub line_height_factor: f32,
    pub padding_left: f32,
    pub padding_top: f32,
    pub padding_right: f32,
    pub flat_char_offset: usize,
    pub content_key: Option<glidemark_core::syntax::CodeKey>,
}

#[derive(Clone, Debug)]
pub struct TextRegionInfo {
    pub block_ix: usize,
    pub text: String,
    pub font_name: &'static str,
    pub font_size: f32,
    pub line_height_factor: f32,
    pub padding_left: f32,
    pub padding_top: f32,
    pub padding_right: f32,
    pub flat_char_offset: usize,
    pub content_key: Option<glidemark_core::syntax::CodeKey>,
}

pub type RegionsRegistry = std::sync::Mutex<Vec<TextLayoutRegion>>;

pub(crate) fn with_text_probe(el: gpui::Div, info: TextRegionInfo, regions: Arc<RegionsRegistry>) -> gpui::Div {
    use gpui::prelude::*;
    #[cfg(feature = "test-observation")]
    let key = info.text.clone();
    let probe = gpui::canvas(
        move |bounds, _window, _cx| {
            #[cfg(feature = "test-observation")]
            obs::log_bounds(&key, bounds);

            let mut reg = regions.lock().unwrap();
            reg.retain(|r| !(r.block_ix == info.block_ix && r.flat_char_offset == info.flat_char_offset));
            reg.push(TextLayoutRegion {
                block_ix: info.block_ix,
                bounds,
                text: info.text.clone(),
                font_name: info.font_name,
                font_size: info.font_size,
                line_height_factor: info.line_height_factor,
                padding_left: info.padding_left,
                padding_top: info.padding_top,
                padding_right: info.padding_right,
                flat_char_offset: info.flat_char_offset,
                content_key: info.content_key.clone(),
            });
        },
        |_bounds, _paint, _window, _cx| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();

    el.relative().child(probe)
}

pub(crate) fn with_text_probe_stateful(el: gpui::Stateful<gpui::Div>, info: TextRegionInfo, regions: Arc<RegionsRegistry>) -> gpui::Stateful<gpui::Div> {
    use gpui::prelude::*;
    #[cfg(feature = "test-observation")]
    let key = info.text.clone();
    let probe = gpui::canvas(
        move |bounds, _window, _cx| {
            #[cfg(feature = "test-observation")]
            obs::log_bounds(&key, bounds);

            let mut reg = regions.lock().unwrap();
            reg.retain(|r| !(r.block_ix == info.block_ix && r.flat_char_offset == info.flat_char_offset));
            reg.push(TextLayoutRegion {
                block_ix: info.block_ix,
                bounds,
                text: info.text.clone(),
                font_name: info.font_name,
                font_size: info.font_size,
                line_height_factor: info.line_height_factor,
                padding_left: info.padding_left,
                padding_top: info.padding_top,
                padding_right: info.padding_right,
                flat_char_offset: info.flat_char_offset,
                content_key: info.content_key.clone(),
            });
        },
        |_bounds, _paint, _window, _cx| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();

    el.relative().child(probe)
}

pub fn calculate_single_line_char_offset_with_font(
    text_system: &gpui::WindowTextSystem,
    line: &str,
    x_pos: f32,
    font_name: &'static str,
    font_size: f32,
) -> usize {
    if line.is_empty() || x_pos <= 0.0 {
        return 0;
    }
    let font = gpui::font(font_name);
    let font_size_px = gpui::px(font_size);
    let run = gpui::TextRun {
        len: line.len(),
        font,
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let shaped = text_system.shape_line(
        gpui::SharedString::from(line.to_string()),
        font_size_px,
        &[run],
        None,
    );
    let target_x = gpui::px(x_pos);
    if target_x >= shaped.width {
        return line.chars().count();
    }

    let byte_indices: Vec<usize> = line.char_indices().map(|(i, _)| i).collect();
    let char_count = byte_indices.len();
    for (k, &b_idx) in byte_indices.iter().enumerate() {
        let gx = shaped.x_for_index(b_idx);
        let next_gx = if k + 1 < char_count {
            shaped.x_for_index(byte_indices[k + 1])
        } else {
            shaped.width
        };
        let mid_x = (gx + next_gx) / 2.0;
        if target_x < mid_x {
            return k;
        }
    }
    char_count
}

#[allow(dead_code)]
pub fn calculate_single_line_char_offset(
    text_system: &gpui::WindowTextSystem,
    line: &str,
    x_pos: f32,
    font_size: f32,
) -> usize {
    calculate_single_line_char_offset_with_font(text_system, line, x_pos, "ui-sans-serif, system-ui", font_size)
}

pub fn calculate_text_click_char_offset(
    text_system: &gpui::WindowTextSystem,
    text: &str,
    x_pos: f32,
    y_pos: f32,
    font_name: &'static str,
    font_size: f32,
    line_height_factor: f32,
    width_limit: Option<f32>,
) -> usize {
    if text.is_empty() {
        return 0;
    }
    if y_pos <= 0.0 && x_pos <= 0.0 {
        return 0;
    }

    let font = gpui::font(font_name);
    let font_size_px = gpui::px(font_size);
    let total_chars = text.chars().count();

    let mut visual_lines: Vec<(String, usize)> = Vec::new();
    let mut current_char_offset = 0;
    let phys_lines: Vec<&str> = text.split('\n').collect();

    for (p_idx, phys_line) in phys_lines.iter().enumerate() {
        let phys_char_count = phys_line.chars().count();
        if phys_line.is_empty() {
            visual_lines.push((String::new(), current_char_offset));
        } else {
            let mut wrapped = false;
            if let Some(max_w) = width_limit {
                if max_w > 0.0 {
                    let run = gpui::TextRun {
                        len: phys_line.len(),
                        font: font.clone(),
                        color: gpui::black(),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let shaped = text_system.shape_line(
                        gpui::SharedString::from(phys_line.to_string()),
                        font_size_px,
                        &[run],
                        None,
                    );
                    if f32::from(shaped.width) > max_w {
                        let chars: Vec<char> = phys_line.chars().collect();
                        let char_count = chars.len();
                        let mut byte_offsets = Vec::with_capacity(char_count + 1);
                        for (b_idx, _) in phys_line.char_indices() {
                            byte_offsets.push(b_idx);
                        }
                        byte_offsets.push(phys_line.len());

                        let mut char_widths = Vec::with_capacity(char_count);
                        for k in 0..char_count {
                            let start_x = f32::from(shaped.x_for_index(byte_offsets[k]));
                            let end_x = if k + 1 == char_count {
                                f32::from(shaped.width)
                            } else {
                                f32::from(shaped.x_for_index(byte_offsets[k + 1]))
                            };
                            char_widths.push((end_x - start_x).max(0.0));
                        }

                        let vls = crate::editor_engine::wrap_line_to_visual_measured(
                            p_idx,
                            phys_line,
                            max_w,
                            |idx, _| char_widths.get(idx).copied().unwrap_or(font_size * 0.6),
                        );

                        if !vls.is_empty() {
                            for vl in vls {
                                let vl_str: String = chars[vl.char_start..vl.char_start + vl.char_len].iter().collect();
                                visual_lines.push((vl_str, current_char_offset + vl.char_start));
                            }
                            wrapped = true;
                        }
                    }
                }
            }

            if !wrapped {
                visual_lines.push((phys_line.to_string(), current_char_offset));
            }
        }

        current_char_offset += phys_char_count + 1;
    }

    if visual_lines.is_empty() {
        return 0;
    }

    let line_height = font_size * line_height_factor;
    let line_idx = if y_pos <= 0.0 {
        0
    } else {
        ((y_pos / line_height).floor() as usize).min(visual_lines.len() - 1)
    };

    let (ref vl_str, vl_offset) = visual_lines[line_idx];
    let line_offset = calculate_single_line_char_offset_with_font(text_system, vl_str, x_pos, font_name, font_size);
    (vl_offset + line_offset).min(total_chars)
}

#[allow(dead_code)]
pub fn calculate_block_click_char_offset(
    text_system: &gpui::WindowTextSystem,
    block_text: &str,
    x_pos: f32,
    y_pos: f32,
    base_font_size: f32,
    width_limit: Option<f32>,
) -> usize {
    calculate_text_click_char_offset(
        text_system,
        block_text,
        x_pos,
        y_pos,
        "ui-sans-serif, system-ui",
        base_font_size,
        1.75,
        width_limit,
    )
}

fn compute_offset_in_region(
    text_system: &gpui::WindowTextSystem,
    region: &TextLayoutRegion,
    click_x: f32,
    click_y: f32,
    total_block_chars: usize,
) -> usize {
    let rx = f32::from(region.bounds.left()) + region.padding_left;
    let ry = f32::from(region.bounds.top()) + region.padding_top;
    let local_x = (click_x - rx).max(0.0);
    let local_y = (click_y - ry).max(0.0);
    let rw = (f32::from(region.bounds.size.width) - region.padding_left - region.padding_right).max(10.0);

    let offset_in_region = calculate_text_click_char_offset(
        text_system,
        &region.text,
        local_x,
        local_y,
        region.font_name,
        region.font_size,
        region.line_height_factor,
        Some(rw),
    );

    (region.flat_char_offset + offset_in_region).min(total_block_chars)
}

pub fn hit_test_char_offset_in_block(
    text_system: &gpui::WindowTextSystem,
    regions: &[TextLayoutRegion],
    block_ix: usize,
    block_text: &str,
    click_pos: Point<Pixels>,
    base_font_size: f32,
    block_bounds: Bounds<Pixels>,
) -> usize {
    let matching_regions: Vec<TextLayoutRegion> = regions
        .iter()
        .filter(|r| r.block_ix == block_ix)
        .cloned()
        .collect();

    let click_x = f32::from(click_pos.x);
    let click_y = f32::from(click_pos.y);

    if matching_regions.is_empty() {
        let local_x = (click_pos.x - block_bounds.left()).max(px(0.));
        let local_y = (click_pos.y - block_bounds.top()).max(px(0.));
        return calculate_text_click_char_offset(
            text_system,
            block_text,
            f32::from(local_x),
            f32::from(local_y),
            "ui-sans-serif, system-ui",
            base_font_size,
            1.75,
            Some(f32::from(block_bounds.size.width)),
        );
    }

    if let Some(r) = matching_regions.iter().find(|r| {
        let l = f32::from(r.bounds.left());
        let t = f32::from(r.bounds.top());
        let w = f32::from(r.bounds.size.width);
        let h = f32::from(r.bounds.size.height);
        click_x >= l && click_x <= l + w && click_y >= t && click_y <= t + h
    }) {
        return compute_offset_in_region(text_system, r, click_x, click_y, block_text.chars().count());
    }

    let y_matching: Vec<&TextLayoutRegion> = matching_regions
        .iter()
        .filter(|r| {
            let t = f32::from(r.bounds.top());
            let h = f32::from(r.bounds.size.height);
            click_y >= t && click_y <= t + h
        })
        .collect();

    if !y_matching.is_empty() {
        let best = y_matching
            .into_iter()
            .min_by(|a, b| {
                let a_center = f32::from(a.bounds.left()) + f32::from(a.bounds.size.width) / 2.0;
                let b_center = f32::from(b.bounds.left()) + f32::from(b.bounds.size.width) / 2.0;
                let da = (a_center - click_x).abs();
                let db = (b_center - click_x).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        return compute_offset_in_region(text_system, best, click_x, click_y, block_text.chars().count());
    }

    let best = matching_regions
        .iter()
        .min_by(|a, b| {
            let a_mid = f32::from(a.bounds.top()) + f32::from(a.bounds.size.height) / 2.0;
            let b_mid = f32::from(b.bounds.top()) + f32::from(b.bounds.size.height) / 2.0;
            let da = (a_mid - click_y).abs();
            let db = (b_mid - click_y).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();

    compute_offset_in_region(text_system, best, click_x, click_y, block_text.chars().count())
}

pub fn hit_test_point_across_all_regions(
    regions: &[TextLayoutRegion],
    text_system: Option<&gpui::WindowTextSystem>,
    mouse_pos: Point<Pixels>,
    blocks: &[BlockElement],
) -> Option<PreviewCharPosition> {
    if regions.is_empty() {
        return None;
    }
    let click_x = f32::from(mouse_pos.x);
    let click_y = f32::from(mouse_pos.y);

    let matching_region = regions
        .iter()
        .filter(|r| {
            let t = f32::from(r.bounds.top());
            let h = f32::from(r.bounds.size.height);
            click_y >= t && click_y <= t + h
        })
        .min_by(|a, b| {
            let a_mid = f32::from(a.bounds.left()) + f32::from(a.bounds.size.width) / 2.0;
            let b_mid = f32::from(b.bounds.left()) + f32::from(b.bounds.size.width) / 2.0;
            let da = (a_mid - click_x).abs();
            let db = (b_mid - click_x).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .or_else(|| {
            regions.iter().min_by(|a, b| {
                let a_mid = f32::from(a.bounds.top()) + f32::from(a.bounds.size.height) / 2.0;
                let b_mid = f32::from(b.bounds.top()) + f32::from(b.bounds.size.height) / 2.0;
                let da = (a_mid - click_y).abs();
                let db = (b_mid - click_y).abs();
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
        })?
        .clone();

    let block_len = blocks
        .get(matching_region.block_ix)
        .map(|b| b.to_plain_text().chars().count())
        .unwrap_or(0);

    let offset = if let Some(ts) = text_system {
        compute_offset_in_region(ts, &matching_region, click_x, click_y, block_len)
    } else {
        let rx = f32::from(matching_region.bounds.left()) + matching_region.padding_left;
        let ry = f32::from(matching_region.bounds.top()) + matching_region.padding_top;
        let local_x = (click_x - rx).max(0.0);
        let local_y = (click_y - ry).max(0.0);
        let line_height = matching_region.font_size * matching_region.line_height_factor;
        let line_idx = (local_y / line_height).floor() as usize;
        let char_width = matching_region.font_size * 0.55;
        let char_idx = (local_x / char_width).round() as usize;
        let phys_lines: Vec<&str> = matching_region.text.split('\n').collect();
        let target_line = line_idx.min(phys_lines.len().saturating_sub(1));
        let mut off = 0;
        for i in 0..target_line {
            off += phys_lines[i].chars().count() + 1;
        }
        let line_chars = phys_lines.get(target_line).map(|l| l.chars().count()).unwrap_or(0);
        off += char_idx.min(line_chars);
        let raw = (matching_region.flat_char_offset + off).min(block_len);
        if raw >= block_len && block_len > 0 {
            block_len - 1
        } else {
            raw
        }
    };

    Some(PreviewCharPosition::new(matching_region.block_ix, offset))
}

pub fn render_preview_pane_with_callbacks(
    parsed: Arc<ParsedDocument>,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    max_column_width: f32,
    font_size: f32,
    list_state: &ListState,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    callbacks: PreviewCallbacks,
    selection: Option<PreviewSelection>,
    is_selecting: bool,
    regions: Arc<RegionsRegistry>,
    syntax_cache: Arc<std::sync::Mutex<glidemark_core::syntax::SyntaxCache>>,
    failures: &[crate::views::workspace::VisibleSyntaxFailure],
) -> impl IntoElement {
    let theme = theme.clone();
    let item_theme = theme.clone();
    let item_parsed = parsed.clone();
    let item_callbacks = callbacks.clone();
    let item_regions = regions.clone();
    let item_syntax_cache = syntax_cache.clone();
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
    }

    pane = pane.child(
        list(list_state.clone(), move |ix, _window, _cx| {
            let Some(block) = item_parsed.blocks.get(ix) else {
                return div().into_any_element();
            };

            let flat = glidemark_core::ast::flatten_block(block, "0");
            let block_text = flat.text.clone();
            let block_char_len = block_text.chars().count();
            let selected_char_range = selection.and_then(|s| s.char_range_for_block(ix, block_char_len));

            let block_bounds_cell = Arc::new(std::cell::Cell::new(Bounds::default()));
            let bounds_for_canvas = block_bounds_cell.clone();

            let mut block_container = div()
                .relative()
                .w_full()
                .max_w(px(max_column_width))
                .mx_auto()
                .child(
                    canvas(
                        move |bounds, _window, _cx| {
                            bounds_for_canvas.set(bounds);
                        },
                        |_bounds, _, _window, _cx| {},
                    )
                    .absolute()
                    .size_full(),
                )
                .child(render_block_element(
                    block,
                    &item_theme,
                    strings,
                    font_size,
                    copied_code_hash,
                    item_callbacks.on_copy.as_ref(),
                    item_callbacks.on_open_lightbox.as_ref(),
                    selected_char_range,
                    ix,
                    item_regions.clone(),
                    item_syntax_cache.clone(),
                    &flat,
                    "0",
                ));

            if let Some(ref on_selection_change) = item_callbacks.on_selection_change {
                let on_sel_down = on_selection_change.clone();
                let on_sel_move = on_selection_change.clone();
                let on_autoscroll = item_callbacks.on_autoscroll.clone();
                let text_for_down = block_text.clone();
                let text_for_move = block_text.clone();
                let bounds_cell_down = block_bounds_cell.clone();
                let bounds_cell_move = block_bounds_cell.clone();
                let regions_down = item_regions.clone();
                let regions_move = item_regions.clone();

                block_container = block_container
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
                        let pos = PreviewCharPosition::new(ix, offset);
                        on_sel_down(Some(PreviewSelection::new(pos, pos)), true, window, cx);
                    })
                    .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                        if is_selecting {
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

            div()
                .w_full()
                .px_8()
                .pb_4()
                .child(block_container)
                .into_any_element()
        })
        .flex_1()
        .w_full()
        .min_w(px(0.))
        .min_h(px(0.))
        .pt_6()
        .pb_6(),
    );

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
                .right_3()
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
                .font_family("ui-sans-serif, system-ui")
                .text_color(theme.text_muted)
                .id("preview_syntax_failure_badge")
                .tooltip(move |_window, cx| {
                    let t = tooltip_text.clone();
                    cx.new(|_| CodeTooltipView { text: t }).into()
                })
                .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, window, cx| {
                    if let Some(ref sel_cb) = on_sel_down {
                        let text_system = window.text_system();
                        let reg = regions_down.lock().unwrap();
                        for (ix, block) in parsed_down.blocks.iter().enumerate() {
                            let flat = glidemark_core::ast::flatten_block(block, "0");
                            let offset = hit_test_char_offset_in_block(
                                text_system,
                                &reg,
                                ix,
                                &flat.text,
                                event.position,
                                font_size,
                                Bounds::default(),
                            );
                            let pos = PreviewCharPosition::new(ix, offset);
                            sel_cb(Some(PreviewSelection::new(pos, pos)), true, window, cx);
                            break;
                        }
                    }
                })
                .on_mouse_move(move |event: &MouseMoveEvent, window, cx| {
                    if is_selecting {
                        if let Some(ref sel_cb) = on_sel_move {
                            if let Some(sel) = selection {
                                let text_system = window.text_system();
                                let reg = regions_move.lock().unwrap();
                                for (ix, block) in parsed_move.blocks.iter().enumerate() {
                                    let flat = glidemark_core::ast::flatten_block(block, "0");
                                    let offset = hit_test_char_offset_in_block(
                                        text_system,
                                        &reg,
                                        ix,
                                        &flat.text,
                                        event.position,
                                        font_size,
                                        Bounds::default(),
                                    );
                                    let head = PreviewCharPosition::new(ix, offset);
                                    sel_cb(Some(PreviewSelection::new(sel.anchor, head)), true, window, cx);
                                    break;
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

fn render_block_element(
    block: &BlockElement,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    base_font_size: f32,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<&OnCopyCallback>,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
    selected_char_range: Option<std::ops::Range<usize>>,
    block_ix: usize,
    regions: Arc<RegionsRegistry>,
    syntax_cache: Arc<std::sync::Mutex<glidemark_core::syntax::SyntaxCache>>,
    flat: &glidemark_core::ast::FlattenResult,
    path: &str,
) -> AnyElement {
    match block {
        BlockElement::Heading { level, content } => {
            let (size_mult, border_bottom) = match level {
                1 => (1.8, true),
                2 => (1.45, true),
                3 => (1.25, false),
                _ => (1.1, false),
            };

            let font_size = base_font_size * size_mult;
            let mut el = div()
                .w_full()
                .min_w(px(0.))
                .text_size(px(font_size))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.text_primary)
                .pt_3()
                .pb_1();

            if border_bottom {
                el = el.border_b_1().border_color(theme.border);
            }

            let seg = flat.segments.iter().find(|s| s.child_path == path);
            let flat_char_offset = seg.map(|s| s.char_range.start).unwrap_or(0);
            let el_range = seg.and_then(|s| {
                selected_char_range.as_ref().and_then(|range| {
                    if range.start < s.char_range.end && range.end > s.char_range.start {
                        let len = s.char_range.len();
                        let start = range.start.saturating_sub(s.char_range.start).min(len);
                        let end = range.end.saturating_sub(s.char_range.start).min(len);
                        if start < end { Some(start..end) } else { None }
                    } else {
                        None
                    }
                })
            });

            el = el.child(render_styled_inlines(content, theme, font_size, el_range));

            with_text_probe(
                el,
                TextRegionInfo {
                    block_ix,
                    text: content.plain_text(),
                    font_name: "ui-sans-serif, system-ui",
                    font_size,
                    line_height_factor: 1.75,
                    padding_left: 0.0,
                    padding_top: 12.0,
                    padding_right: 0.0,
                    flat_char_offset,
                    content_key: None,
                },
                regions,
            )
            .into_any_element()
        }
        BlockElement::Paragraph(content) => {
            let seg = flat.segments.iter().find(|s| s.child_path == path);
            let flat_char_offset = seg.map(|s| s.char_range.start).unwrap_or(0);
            let el_range = seg.and_then(|s| {
                selected_char_range.as_ref().and_then(|range| {
                    if range.start < s.char_range.end && range.end > s.char_range.start {
                        let len = s.char_range.len();
                        let start = range.start.saturating_sub(s.char_range.start).min(len);
                        let end = range.end.saturating_sub(s.char_range.start).min(len);
                        if start < end { Some(start..end) } else { None }
                    } else {
                        None
                    }
                })
            });

            let el = div()
                .w_full()
                .min_w(px(0.))
                .text_size(px(base_font_size))
                .line_height(relative(1.75))
                .text_color(theme.text_primary)
                .child(render_styled_inlines(content, theme, base_font_size, el_range));

            with_text_probe(
                el,
                TextRegionInfo {
                    block_ix,
                    text: content.plain_text(),
                    font_name: "ui-sans-serif, system-ui",
                    font_size: base_font_size,
                    line_height_factor: 1.75,
                    padding_left: 0.0,
                    padding_top: 0.0,
                    padding_right: 0.0,
                    flat_char_offset,
                    content_key: None,
                },
                regions,
            )
            .into_any_element()
        }
        BlockElement::FencedCode { lang, code } => {
            if lang.as_deref() == Some("mermaid") {
                render_mermaid_diagram(code, theme, strings, base_font_size, on_open_lightbox)
            } else {
                let seg = flat.segments.iter().find(|s| s.child_path == path);
                let flat_char_offset = seg.map(|s| s.char_range.start).unwrap_or(0);
                let el_range = seg.and_then(|s| {
                    selected_char_range.as_ref().and_then(|range| {
                        if range.start < s.char_range.end && range.end > s.char_range.start {
                            let len = s.char_range.len();
                            let start = range.start.saturating_sub(s.char_range.start).min(len);
                            let end = range.end.saturating_sub(s.char_range.start).min(len);
                            if start < end { Some(start..end) } else { None }
                        } else {
                            None
                        }
                    })
                });

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
                    selected_char_range.clone(),
                    block_ix,
                    regions.clone(),
                    syntax_cache.clone(),
                    flat,
                    &child_path,
                ));
            }

            div()
                .w_full()
                .min_w(px(0.))
                .border_l_4()
                .border_color(theme.accent)
                .bg(theme.surface)
                .p_3()
                .rounded_r_md()
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
            selected_char_range,
            block_ix,
            regions,
            syntax_cache.clone(),
            flat,
            path,
        ),
        BlockElement::Table { headers, rows } => {
            let mut cur_offset = 0;
            let mut header_cells = Vec::new();
            for h in headers {
                let text = h.plain_text();
                let len = text.chars().count();
                let cell_start = cur_offset;
                let cell_end = cur_offset + len;
                let cell_range = selected_char_range.as_ref().and_then(|range| {
                    if range.start < cell_end && range.end > cell_start {
                        let s = range.start.saturating_sub(cell_start).min(len);
                        let e = range.end.saturating_sub(cell_start).min(len);
                        if s < e { Some(s..e) } else { None }
                    } else {
                        None
                    }
                });
                cur_offset += len + 1;
                header_cells.push(
                    with_text_probe(
                        div()
                            .flex_1()
                            .min_w(px(120.))
                            .p_3()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(base_font_size * 0.95))
                            .text_color(theme.text_primary)
                            .child(render_styled_inlines(h, theme, base_font_size * 0.95, cell_range)),
                        TextRegionInfo {
                            block_ix,
                            text: text.clone(),
                            font_name: "ui-sans-serif, system-ui",
                            font_size: base_font_size * 0.95,
                            line_height_factor: 1.75,
                            padding_left: 12.0,
                            padding_top: 12.0,
                            padding_right: 12.0,
                            flat_char_offset: cell_start,
                            content_key: None,
                        },
                        regions.clone(),
                    )
                );
            }

            let mut row_cells = Vec::new();
            for row in rows {
                let mut cells = Vec::new();
                for cell in row {
                    let text = cell.plain_text();
                    let len = text.chars().count();
                    let cell_start = cur_offset;
                    let cell_end = cur_offset + len;
                    let cell_range = selected_char_range.as_ref().and_then(|range| {
                        if range.start < cell_end && range.end > cell_start {
                            let s = range.start.saturating_sub(cell_start).min(len);
                            let e = range.end.saturating_sub(cell_start).min(len);
                            if s < e { Some(s..e) } else { None }
                        } else {
                            None
                        }
                    });
                    cur_offset += len + 1;
                    cells.push(
                        with_text_probe(
                            div()
                                .flex_1()
                                .min_w(px(120.))
                                .p_3()
                                .text_size(px(base_font_size * 0.95))
                                .text_color(theme.text_primary)
                                .child(render_styled_inlines(cell, theme, base_font_size * 0.95, cell_range)),
                            TextRegionInfo {
                                block_ix,
                                text: text.clone(),
                                font_name: "ui-sans-serif, system-ui",
                                font_size: base_font_size * 0.95,
                                line_height_factor: 1.75,
                                padding_left: 12.0,
                                padding_top: 12.0,
                                padding_right: 12.0,
                                flat_char_offset: cell_start,
                                content_key: None,
                            },
                            regions.clone(),
                        )
                    );
                }
                row_cells.push(
                    div()
                        .border_b_1()
                        .border_color(theme.border)
                        .flex()
                        .children(cells),
                );
            }

            div()
                .id("preview_table_scroll")
                .w_full()
                .min_w(px(0.))
                .overflow_x_scroll()
                .my_2()
                .child(
                    div()
                        .w_full()
                        .min_w(px(520.))
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .bg(theme.bg)
                                .border_b_1()
                                .border_color(theme.border)
                                .flex()
                                .children(header_cells),
                        )
                        .children(row_cells),
                )
                .into_any_element()
        }
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
) -> AnyElement {
    let has_image = group.0.iter().any(|item| matches!(item, InlineItem::Image { .. }));
    if has_image {
        return div()
            .w_full()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .gap_2()
            .children(group.0.iter().map(|item| match item {
                InlineItem::Image { alt, url, .. } => div()
                    .w_full()
                    .min_w(px(0.))
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.bg)
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .child(
                        svg()
                            .path("icons/image.svg")
                            .size(px(18.))
                            .text_color(theme.text_muted),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_sm()
                            .text_color(theme.text_muted)
                            .child(if alt.is_empty() { url.clone() } else { format!("{} ({})", alt, url) }),
                    )
                    .into_any_element(),
                other => {
                    let single = InlineGroup(vec![other.clone()]);
                    render_styled_text_only(&single, theme, font_size, selected_char_range.clone()).into_any_element()
                }
            }))
            .into_any_element();
    }

    render_styled_text_only(group, theme, font_size, selected_char_range).into_any_element()
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
                        color: Some(theme.accent),
                        underline: Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(theme.accent),
                            wavy: false,
                        }),
                        ..Default::default()
                    }));
                }
            }
            InlineItem::Image { alt, .. } => {
                text.push_str(alt);
            }
        }
    }

    #[cfg(feature = "test-observation")]
    obs::log_highlight(&text, &selected_char_range);

    if let Some(char_range) = selected_char_range {
        let byte_range = char_range_to_byte_range(&text, char_range);
        highlights = apply_selection_to_highlights(&text, &highlights, byte_range, theme.selection_bg());
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

pub const MAX_DIAGRAM_CACHE: usize = 64;

pub struct DiagramCache {
    entries: std::collections::HashMap<(u64, bool), std::path::PathBuf>,
    order: std::collections::VecDeque<(u64, bool)>,
    max_size: usize,
}

impl DiagramCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
            max_size,
        }
    }

    pub fn get(&mut self, key: &(u64, bool)) -> Option<std::path::PathBuf> {
        let path = self.entries.get(key)?.clone();
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
            self.order.push_back(*key);
        }
        Some(path)
    }

    pub fn insert(&mut self, key: (u64, bool), path: std::path::PathBuf) {
        if self.entries.insert(key, path).is_none() {
            self.order.push_back(key);
            if self.order.len() > self.max_size {
                if let Some(evicted_key) = self.order.pop_front() {
                    if let Some(evicted_path) = self.entries.remove(&evicted_key) {
                        let _ = std::fs::remove_file(evicted_path);
                    }
                }
            }
        } else if let Some(pos) = self.order.iter().position(|k| k == &key) {
            self.order.remove(pos);
            self.order.push_back(key);
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

static DIAGRAM_CACHE: std::sync::LazyLock<std::sync::Mutex<DiagramCache>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(DiagramCache::new(MAX_DIAGRAM_CACHE)));

fn render_mermaid_container(
    path: std::path::PathBuf,
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
        .overflow_x_scroll();

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
            .bg(theme.selection_bg())
            .hover(|s| s.bg(theme.hover_bg()))
            .tooltip({
                let t = strings.mermaid_zoom_hint.to_string();
                move |_window, cx| cx.new(|_| CodeTooltipView { text: t.clone() }).into()
            })
            .on_mouse_down(MouseButton::Left, move |_event, window, cx| {
                cx.stop_propagation(); // 防冒泡到外层容器
                cb_btn(p_btn.clone(), code_hash, window, cx);
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
                cb(p_click.clone(), code_hash, window, app);
            })
            .child(btn);
    }

    container
        .child(
            img(path)
                .max_w_full()
        )
        .into_any_element()
}

fn render_mermaid_diagram(
    code: &str,
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    _base_font_size: f32,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
) -> AnyElement {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.hash(&mut hasher);
    let code_hash = hasher.finish();
    let is_dark = theme.is_dark;

    // Check in-memory memoized cache first (zero-cost on scroll)
    let cached_path = {
        let mut cache = DIAGRAM_CACHE.lock().unwrap();
        cache.get(&(code_hash, is_dark))
    };

    if let Some(path) = cached_path {
        if path.exists() {
            return render_mermaid_container(path, code_hash, theme, strings, on_open_lightbox);
        }
    }

    let result = glidemark_core::mermaid::render_to_svg_themed(code, is_dark);
    match result {
        Ok(svg_content) => {
            let cache_dir = std::env::temp_dir()
                .join("glidemark")
                .join("mermaid");
            let _ = std::fs::create_dir_all(&cache_dir);
            let filename = format!("{:016x}_{}.svg", code_hash, if is_dark { "dark" } else { "light" });
            let path = cache_dir.join(filename);
            let _ = std::fs::write(&path, svg_content);

            {
                let mut cache = DIAGRAM_CACHE.lock().unwrap();
                cache.insert((code_hash, is_dark), path.clone());
            }

            render_mermaid_container(path, code_hash, theme, strings, on_open_lightbox)
        }
        Err(err) => {
            div()
                .w_full()
                .min_w(px(0.))
                .my_3()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .bg(theme.bg)
                .p_3()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.accent)
                        .child("Mermaid Syntax Error"),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family("ui-monospace")
                        .text_color(theme.text_muted)
                        .child(err),
                )
                .into_any_element()
        }
    }
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
            .font_family("ui-sans-serif, system-ui")
            .shadow_md()
            .child(self.text.clone())
    }
}

fn render_list_element(
    ordered: bool,
    items: &[glidemark_core::ast::ListItem],
    theme: &ThemeColors,
    strings: &'static I18nStrings,
    base_font_size: f32,
    depth: usize,
    copied_code_hash: Option<(u64, std::time::Instant)>,
    on_copy: Option<&OnCopyCallback>,
    on_open_lightbox: Option<&OnOpenLightboxCallback>,
    selected_char_range: Option<std::ops::Range<usize>>,
    block_ix: usize,
    regions: Arc<RegionsRegistry>,
    syntax_cache: Arc<std::sync::Mutex<glidemark_core::syntax::SyntaxCache>>,
    flat: &glidemark_core::ast::FlattenResult,
    path: &str,
) -> AnyElement {
    let mut item_elements = Vec::with_capacity(items.len());

    for (idx, item) in items.iter().enumerate() {
        let bullet_element = if let Some(checked) = item.checked {
            div()
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
        } else if ordered {
            div()
                .flex_shrink_0()
                .w(px(20.))
                .text_right()
                .text_size(px(base_font_size))
                .font_family("ui-monospace")
                .text_color(theme.syntax_bullet)
                .child(format!("{}.", idx + 1))
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
        };

        let mut first_child_el = None;
        let mut subsequent_child_els = Vec::new();

        for (ci, child) in item.body.iter().enumerate() {
            let child_path = format!("{}.{}.{}", path, idx, ci);
            let seg = flat.segments.iter().find(|s| s.child_path == child_path);
            let flat_char_offset = seg.map(|s| s.char_range.start).unwrap_or(0);
            let child_range = seg.and_then(|s| {
                selected_char_range.as_ref().and_then(|range| {
                    if range.start < s.char_range.end && range.end > s.char_range.start {
                        let len = s.char_range.len();
                        let start = range.start.saturating_sub(s.char_range.start).min(len);
                        let end = range.end.saturating_sub(s.char_range.start).min(len);
                        if start < end { Some(start..end) } else { None }
                    } else {
                        None
                    }
                })
            });

            if ci == 0 {
                if let glidemark_core::ast::BlockElement::Paragraph(content) = child {
                    let content_text = content.plain_text();
                    first_child_el = Some(
                        with_text_probe(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(px(base_font_size))
                                .line_height(relative(1.6))
                                .text_color(theme.text_primary)
                                .child(render_styled_inlines(content, theme, base_font_size, child_range)),
                            TextRegionInfo {
                                block_ix,
                                text: content_text,
                                font_name: "ui-sans-serif, system-ui",
                                font_size: base_font_size,
                                line_height_factor: 1.6,
                                padding_left: 0.0,
                                padding_top: 0.0,
                                padding_right: 0.0,
                                flat_char_offset,
                                content_key: None,
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
                        selected_char_range.clone(),
                        block_ix,
                        regions.clone(),
                        syntax_cache.clone(),
                        flat,
                        &child_path,
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
                let rendered = if let glidemark_core::ast::BlockElement::List { ordered: sub_ordered, items: sub_items } = child {
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
                        selected_char_range.clone(),
                        block_ix,
                        regions.clone(),
                        syntax_cache.clone(),
                        flat,
                        &child_path,
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
                        selected_char_range.clone(),
                        block_ix,
                        regions.clone(),
                        syntax_cache.clone(),
                        flat,
                        &child_path,
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
    syntax_cache: &Arc<std::sync::Mutex<glidemark_core::syntax::SyntaxCache>>,
) -> AnyElement {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    code.as_ref().hash(&mut hasher);
    let hash = hasher.finish();
    let is_copied = copied_code_hash
        .map(|(h, t)| h == hash && t.elapsed() < std::time::Duration::from_millis(1200))
        .unwrap_or(false);

    let on_copy_cb = on_copy.cloned();
    let code_to_copy = code.to_string();
    let code_text = if code.is_empty() { " ".to_string() } else { code.to_string() };

    let canonical_lang = glidemark_core::syntax::resolve_canonical_language(lang);
    let key = glidemark_core::syntax::CodeKey::new(canonical_lang, code.clone());
    let content_key = Some(key.clone());

    let should_wrap = should_wrap_code_block(lang);

    let group_name: SharedString = format!("code_block_{}", hash).into();

    let spans = syntax_cache
        .lock()
        .unwrap()
        .get(&key)
        .unwrap_or_else(|| {
            Arc::from([glidemark_core::syntax::SyntaxSpan {
                range: 0..code.len(),
                role: glidemark_core::syntax::SyntaxRole::Plain,
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
            apply_selection_to_highlights(&code_text, &syntax_highlights, byte_range, theme.selection_bg())
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

    let code_font_size = base_font_size * 0.85;
    let mut content = div()
        .id(SharedString::from(format!("code_content_scroll_{}", hash)))
        .w_full()
        .min_w(px(0.))
        .p_3()
        .text_size(px(code_font_size))
        .font_family("ui-monospace")
        .line_height(relative(1.5))
        .text_color(theme.syntax_code)
        .child(styled_text);

    if should_wrap {
        content = content.whitespace_normal();
    } else {
        content = content.overflow_x_scroll().whitespace_nowrap();
    }

    let content = with_text_probe_stateful(
        content,
        TextRegionInfo {
            block_ix,
            text: code.to_string(),
            font_name: "ui-monospace",
            font_size: code_font_size,
            line_height_factor: 1.5,
            padding_left: 12.0,
            padding_top: 12.0,
            padding_right: 12.0,
            flat_char_offset: base_char_offset,
            content_key,
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
        .bg(theme.selection_bg())
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
                cb(code_to_copy.clone(), hash, window, cx);
            }
        })
        .child(
            svg()
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
                }),
        );

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
        .into_any_element()
}
