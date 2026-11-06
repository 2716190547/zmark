use gpui::*;
use glidemark_core::ast::{BlockElement, ParsedDocument};
use std::sync::Arc;

#[cfg(feature = "test-observation")]
use super::obs;

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
    pub fence_open_line: Option<usize>,
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
    pub fence_open_line: Option<usize>,
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
                fence_open_line: info.fence_open_line,
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
                fence_open_line: info.fence_open_line,
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
            crate::platform::UI_FONT,
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
