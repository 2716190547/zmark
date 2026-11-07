use std::sync::Arc;

const MAX_UNDO_STEPS: usize = 100;
const MAX_UNDO_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualLine {
    pub physical_line_idx: usize,
    pub subline_idx: usize,
    pub char_start: usize,
    pub char_len: usize,
    pub is_first: bool,
}

pub fn char_unit_width(ch: char) -> usize {
    if ch == '\t' {
        return 4;
    }
    let u = ch as u32;
    // ASCII printable
    if (0x20..=0x7E).contains(&u) {
        return 1;
    }
    // Zero-width or control characters
    if u < 0x20 || (0x7F..=0x9F).contains(&u) || u == 0x200B || u == 0xFEFF {
        return 0;
    }
    // CJK Unified Ideographs, Hangul, Hiragana, Katakana, Fullwidth Forms, CJK symbols & punctuation, Emojis
    if (0x1100..=0x115F).contains(&u)
        || (0x2E80..=0xA4CF).contains(&u)
        || (0xAC00..=0xD7A3).contains(&u)
        || (0xF900..=0xFAFF).contains(&u)
        || (0xFE10..=0xFE19).contains(&u)
        || (0xFE30..=0xFE6F).contains(&u)
        || (0xFF01..=0xFF60).contains(&u)
        || (0xFFE0..=0xFFE6).contains(&u)
        || (0x1F300..=0x1F9FF).contains(&u)
        || (0x20000..=0x2FA1F).contains(&u)
    {
        return 2;
    }
    1
}

pub fn wrap_line_to_visual_measured<F>(
    physical_line_idx: usize,
    line_str: &str,
    max_width: f32,
    mut char_measure: F,
) -> Vec<VisualLine>
where
    F: FnMut(usize, char) -> f32,
{
    let max_width = max_width.max(1.0);
    let chars: Vec<char> = line_str.chars().collect();
    if chars.is_empty() {
        return vec![VisualLine {
            physical_line_idx,
            subline_idx: 0,
            char_start: 0,
            char_len: 0,
            is_first: true,
        }];
    }

    let mut visual_lines = Vec::new();
    let mut subline_idx = 0;
    let mut char_start = 0;
    let mut current_idx = 0;
    let mut current_width = 0.0;
    let mut last_break_char = None;

    while current_idx < chars.len() {
        let ch = chars[current_idx];
        let w = char_measure(current_idx, ch);

        if current_width + w > max_width && current_width > 0.0 {
            let break_at = if let Some(b) = last_break_char {
                if b > char_start && b <= current_idx {
                    b
                } else {
                    current_idx
                }
            } else {
                current_idx
            };

            visual_lines.push(VisualLine {
                physical_line_idx,
                subline_idx,
                char_start,
                char_len: break_at - char_start,
                is_first: subline_idx == 0,
            });

            subline_idx += 1;
            char_start = break_at;
            current_idx = break_at;
            current_width = 0.0;
            last_break_char = None;
            continue;
        }

        current_width += w;
        current_idx += 1;

        if ch == ' '
            || ch == '\t'
            || char_unit_width(ch) == 2
            || ch == '-'
            || ch == '/'
            || ch == '_'
            || ch == ','
            || ch == '.'
            || ch == ';'
        {
            last_break_char = Some(current_idx);
        }
    }

    if char_start < chars.len() || visual_lines.is_empty() {
        visual_lines.push(VisualLine {
            physical_line_idx,
            subline_idx,
            char_start,
            char_len: chars.len() - char_start,
            is_first: subline_idx == 0,
        });
    }

    visual_lines
}

#[allow(dead_code)]
pub fn wrap_line_to_visual(
    physical_line_idx: usize,
    line_str: &str,
    max_cols: usize,
) -> Vec<VisualLine> {
    wrap_line_to_visual_measured(physical_line_idx, line_str, max_cols.max(1) as f32, |_, ch| {
        char_unit_width(ch) as f32
    })
}

pub fn wrap_line_to_visual_with_tab_width(
    physical_line_idx: usize,
    line_str: &str,
    max_cols: usize,
    tab_width: usize,
) -> Vec<VisualLine> {
    wrap_line_to_visual_measured(physical_line_idx, line_str, max_cols.max(1) as f32, |_, ch| {
        if ch == '\t' {
            tab_width as f32
        } else {
            char_unit_width(ch) as f32
        }
    })
}

pub fn physical_to_visual(
    phys_line: usize,
    phys_col: usize,
    visual_lines: &[VisualLine],
) -> (usize, usize) {
    let sublines: Vec<(usize, &VisualLine)> = visual_lines
        .iter()
        .enumerate()
        .filter(|(_, vl)| vl.physical_line_idx == phys_line)
        .collect();

    if sublines.is_empty() {
        return (0, 0);
    }

    for (idx, &(vis_idx, vl)) in sublines.iter().enumerate() {
        let is_last = idx + 1 == sublines.len();
        if phys_col >= vl.char_start && (phys_col < vl.char_start + vl.char_len || is_last) {
            let rel_col = phys_col.saturating_sub(vl.char_start);
            return (vis_idx, rel_col);
        }
    }

    let (last_vis_idx, last_vl) = sublines.last().unwrap();
    (*last_vis_idx, last_vl.char_len)
}

pub fn visual_to_physical(
    vis_line_idx: usize,
    vis_col: usize,
    visual_lines: &[VisualLine],
) -> (usize, usize) {
    if visual_lines.is_empty() {
        return (0, 0);
    }
    let vl = if vis_line_idx < visual_lines.len() {
        &visual_lines[vis_line_idx]
    } else {
        visual_lines.last().unwrap()
    };

    let phys_line = vl.physical_line_idx;
    let phys_col = vl.char_start + vis_col.min(vl.char_len);
    (phys_line, phys_col)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorEngine {
    pub lines: Arc<Vec<String>>,
    /// Character count including line breaks, cached so the status pill does not
    /// rescan every line on every frame.
    pub char_count: usize,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub revision: usize,
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
    undo_bytes: usize,
    redo_bytes: usize,
}

impl EditorEngine {
    pub fn new(initial_text: &str) -> Self {
        let lines = split_lines(initial_text);
        let last_line_idx = lines.len() - 1;
        let last_col = lines[last_line_idx].chars().count();
        Self {
            char_count: count_chars(&lines),
            lines: Arc::new(lines),
            cursor_line: last_line_idx,
            cursor_col: last_col,
            revision: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            undo_bytes: 0,
            redo_bytes: 0,
        }
    }

    pub fn reset_text(&mut self, text: &str) {
        self.lines = Arc::new(split_lines(text));
        self.char_count = count_chars(&self.lines);
        self.cursor_line = self.cursor_line.min(self.lines.len() - 1);
        self.clamp_cursor();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.undo_bytes = 0;
        self.redo_bytes = 0;
        self.revision += 1;
    }

    pub fn to_text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_col)
    }

    pub fn set_cursor(&mut self, line: usize, col: usize) {
        self.cursor_line = line.min(self.lines.len() - 1);
        self.cursor_col = col;
        self.clamp_cursor();
    }

    fn clamp_cursor(&mut self) {
        let max_col = self.lines[self.cursor_line].chars().count();
        self.cursor_col = self.cursor_col.min(max_col);
    }

    pub fn push_snapshot(&mut self) {
        let snapshot = self.to_text();
        self.undo_bytes += snapshot.len();
        self.undo_stack.push(snapshot);
        trim_stack(&mut self.undo_stack, &mut self.undo_bytes);
        self.redo_stack.clear();
        self.redo_bytes = 0;
    }

    /// Replace one verified, single-line source range as one undoable edit.
    pub fn replace_source_range(&mut self, range: std::ops::Range<usize>, expected: &str, replacement: &str) -> bool {
        let source = self.to_text();
        if range.start > range.end || source.get(range.clone()) != Some(expected)
            || expected.contains('\n') || replacement.contains('\n')
        {
            return false;
        }
        let Some(new_len) = source.len().checked_sub(range.len()).and_then(|n| n.checked_add(replacement.len())) else { return false; };
        if source.len() > MAX_UNDO_BYTES || new_len > MAX_UNDO_BYTES { return false; }
        let prefix = &source[..range.start];
        let line = prefix.bytes().filter(|&b| b == b'\n').count();
        let start_col = prefix.rsplit('\n').next().unwrap_or("").chars().count();
        let old_end = start_col + expected.chars().count();
        let new_end = start_col + replacement.chars().count();
        let cursor_col = if self.cursor_line == line {
            if self.cursor_col < start_col { self.cursor_col }
            else if self.cursor_col < old_end { new_end }
            else { self.cursor_col - old_end + new_end }
        } else { self.cursor_col };
        let mut updated = source;
        updated.replace_range(range, replacement);
        self.push_snapshot();
        self.lines = Arc::new(split_lines(&updated));
        self.char_count = count_chars(&self.lines);
        self.cursor_col = cursor_col;
        self.clamp_cursor();
        self.revision += 1;
        true
    }

    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.push_snapshot();
        for ch in text.chars() {
            if ch == '\n' {
                self.internal_newline();
            } else {
                self.internal_insert_char(ch);
            }
        }
        self.char_count = count_chars(&self.lines);
        self.revision += 1;
    }

    pub fn insert_newline(&mut self) {
        self.push_snapshot();
        self.internal_newline();
        self.char_count = count_chars(&self.lines);
        self.revision += 1;
    }

    fn internal_insert_char(&mut self, ch: char) {
        let line = &mut Arc::make_mut(&mut self.lines)[self.cursor_line];
        let mut chars: Vec<char> = line.chars().collect();
        chars.insert(self.cursor_col, ch);
        *line = chars.into_iter().collect();
        self.cursor_col += 1;
    }

    fn internal_newline(&mut self) {
        let chars: Vec<char> = self.lines[self.cursor_line].chars().collect();
        let left: String = chars[..self.cursor_col].iter().collect();
        let right: String = chars[self.cursor_col..].iter().collect();

        let lines = Arc::make_mut(&mut self.lines);
        lines[self.cursor_line] = left;
        lines.insert(self.cursor_line + 1, right);
        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    pub fn delete_backward(&mut self) {
        if self.cursor_col > 0 {
            self.push_snapshot();
            let line = &mut Arc::make_mut(&mut self.lines)[self.cursor_line];
            let mut chars: Vec<char> = line.chars().collect();
            chars.remove(self.cursor_col - 1);
            *line = chars.into_iter().collect();
            self.cursor_col -= 1;
            self.revision += 1;
        } else if self.cursor_line > 0 {
            self.push_snapshot();
            let lines = Arc::make_mut(&mut self.lines);
            let current = lines.remove(self.cursor_line);
            let prev_line = &mut lines[self.cursor_line - 1];
            let prev_len = prev_line.chars().count();
            prev_line.push_str(&current);
            self.cursor_line -= 1;
            self.cursor_col = prev_len;
            self.revision += 1;
        }
        self.char_count = count_chars(&self.lines);
    }

    pub fn delete_forward(&mut self) {
        let line_len = self.lines[self.cursor_line].chars().count();
        if self.cursor_col < line_len {
            self.push_snapshot();
            let line = &mut Arc::make_mut(&mut self.lines)[self.cursor_line];
            let mut chars: Vec<char> = line.chars().collect();
            chars.remove(self.cursor_col);
            *line = chars.into_iter().collect();
            self.revision += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.push_snapshot();
            let lines = Arc::make_mut(&mut self.lines);
            let next = lines.remove(self.cursor_line + 1);
            lines[self.cursor_line].push_str(&next);
            self.revision += 1;
        }
        self.char_count = count_chars(&self.lines);
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        let line_len = self.lines[self.cursor_line].chars().count();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    #[allow(dead_code)]
    pub fn move_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.clamp_cursor();
        }
    }

    #[allow(dead_code)]
    pub fn move_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.clamp_cursor();
        }
    }

    pub fn move_to_line_start(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_to_line_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_line].chars().count();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.undo_stack.pop() {
            self.undo_bytes = self.undo_bytes.saturating_sub(prev.len());
            let current = self.to_text();
            self.redo_bytes += current.len();
            self.redo_stack.push(current);
            trim_stack(&mut self.redo_stack, &mut self.redo_bytes);
            self.lines = Arc::new(split_lines(&prev));
            self.char_count = count_chars(&self.lines);
            self.cursor_line = self.cursor_line.min(self.lines.len() - 1);
            self.clamp_cursor();
            self.revision += 1;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.redo_bytes = self.redo_bytes.saturating_sub(next.len());
            let current = self.to_text();
            self.undo_bytes += current.len();
            self.undo_stack.push(current);
            trim_stack(&mut self.undo_stack, &mut self.undo_bytes);
            self.lines = Arc::new(split_lines(&next));
            self.char_count = count_chars(&self.lines);
            self.cursor_line = self.cursor_line.min(self.lines.len() - 1);
            self.clamp_cursor();
            self.revision += 1;
            true
        } else {
            false
        }
    }

    pub fn move_up_visual_lines(&mut self, vis_lines: &[VisualLine]) {
        let (vis_idx, vis_col) = physical_to_visual(self.cursor_line, self.cursor_col, vis_lines);
        if vis_idx > 0 {
            let (new_phys_line, new_phys_col) =
                visual_to_physical(vis_idx - 1, vis_col, vis_lines);
            self.set_cursor(new_phys_line, new_phys_col);
        }
    }

    pub fn move_down_visual_lines(&mut self, vis_lines: &[VisualLine]) {
        let (vis_idx, vis_col) = physical_to_visual(self.cursor_line, self.cursor_col, vis_lines);
        if vis_idx + 1 < vis_lines.len() {
            let (new_phys_line, new_phys_col) =
                visual_to_physical(vis_idx + 1, vis_col, vis_lines);
            self.set_cursor(new_phys_line, new_phys_col);
        }
    }

}

fn count_chars(lines: &[String]) -> usize {
    lines.iter().map(|line| line.chars().count()).sum::<usize>()
        + lines.len().saturating_sub(1)
}

fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        vec![String::new()]
    } else {
        text.split('\n').map(|s| s.to_string()).collect()
    }
}

/// Keeps undo/redo bounded by both step count and total text bytes.
fn trim_stack(stack: &mut Vec<String>, bytes: &mut usize) {
    while stack.len() > MAX_UNDO_STEPS || *bytes > MAX_UNDO_BYTES {
        let removed = stack.remove(0);
        *bytes = bytes.saturating_sub(removed.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_stack_respects_byte_budget() {
        let mut editor = EditorEngine::new(&"x".repeat(1024 * 1024));
        for _ in 0..12 {
            editor.insert_text("a");
        }

        assert!(editor.undo_bytes <= MAX_UNDO_BYTES);
        assert!(editor.undo_stack.len() <= 8);
        assert!(editor.undo());
    }

    #[test]
    fn test_char_unit_width() {
        assert_eq!(char_unit_width('a'), 1);
        assert_eq!(char_unit_width('Z'), 1);
        assert_eq!(char_unit_width('1'), 1);
        assert_eq!(char_unit_width(' '), 1);
        assert_eq!(char_unit_width('\t'), 4);
        assert_eq!(char_unit_width('\0'), 0);
        assert_eq!(char_unit_width('\u{200B}'), 0);
        assert_eq!(char_unit_width('中'), 2);
        assert_eq!(char_unit_width('文'), 2);
        assert_eq!(char_unit_width('🎉'), 2);
        assert_eq!(char_unit_width('é'), 1);
    }

    #[test]
    fn test_wrap_line_to_visual() {
        let empty = wrap_line_to_visual(0, "", 10);
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0].char_len, 0);
        assert!(empty[0].is_first);

        let short = wrap_line_to_visual(1, "hello", 10);
        assert_eq!(short.len(), 1);
        assert_eq!(short[0].char_len, 5);
        assert_eq!(short[0].char_start, 0);

        let wrapped = wrap_line_to_visual(2, "hello world test wrap", 12);
        assert!(wrapped.len() > 1);
        assert!(wrapped[0].is_first);
        assert!(!wrapped[1].is_first);

        let cjk = wrap_line_to_visual(3, "你好世界测试换行", 8);
        assert!(cjk.len() > 1);
    }

    #[test]
    fn test_bidirectional_coordinate_mapping() {
        let text = "hello world quick brown fox jumps over";
        let vl = wrap_line_to_visual(0, text, 12);

        for col in 0..text.chars().count() {
            let (v_line, v_col) = physical_to_visual(0, col, &vl);
            let (p_line, p_col) = visual_to_physical(v_line, v_col, &vl);
            assert_eq!(p_line, 0);
            assert_eq!(p_col, col);
        }
    }

    #[test]
    fn char_count_tracks_edits_and_undo() {
        let mut editor = EditorEngine::new("a\nbc");
        assert_eq!(editor.char_count, 4); // 'a' + '\n' + 'b' + 'c'
        editor.insert_text("x");
        assert_eq!(editor.char_count, 5);
        assert!(editor.undo());
        assert_eq!(editor.char_count, 4);
        assert!(editor.redo());
        assert_eq!(editor.char_count, 5);
    }

    #[test]
    fn test_engine_navigation_with_wrapped_lines() {
        let mut editor = EditorEngine::new("first line\nhello world long line here wrapping\nthird line");
        let visual: Vec<VisualLine> = editor
            .lines
            .iter()
            .enumerate()
            .flat_map(|(idx, line)| wrap_line_to_visual_with_tab_width(idx, line, 12, 4))
            .collect();

        editor.set_cursor(1, 20);
        editor.move_up_visual_lines(&visual);
        let (cur_line, _) = editor.cursor();
        assert!(cur_line <= 1);

        editor.move_down_visual_lines(&visual);
    }

    #[test]
    fn test_wrap_line_to_visual_measured_and_tab_width() {
        let text = "a\tb\tc";
        // With tab_width = 4: 'a'(1) + '\t'(4) + 'b'(1) + '\t'(4) + 'c'(1) = 11 cols
        let wrapped_tab4 = wrap_line_to_visual_with_tab_width(0, text, 6, 4);
        assert!(wrapped_tab4.len() > 1);

        // With tab_width = 1 (matching 1 monospace font advance): 1 + 1 + 1 + 1 + 1 = 5 cols
        let wrapped_tab1 = wrap_line_to_visual_with_tab_width(0, text, 6, 1);
        assert_eq!(wrapped_tab1.len(), 1);
        assert_eq!(wrapped_tab1[0].char_len, 5);

        // Measured with arbitrary float pixel widths: 11 chars * 10.0 = 110.0px, max_width 50.0px => 3 lines
        let pixel_wrapped = wrap_line_to_visual_measured(0, "hello world", 50.0, |_, _| 10.0);
        assert_eq!(pixel_wrapped.len(), 3);
        assert_eq!(pixel_wrapped[0].char_len, 5); // "hello" breaks before space
    }

    #[test]
    fn test_move_visual_lines_direct() {
        let mut editor = EditorEngine::new("line one is very long and will wrap across several sublines\nshort line two");
        let measured_lines = wrap_line_to_visual(0, &editor.lines[0], 15);
        assert!(measured_lines.len() > 1);

        let mut all_vis = measured_lines;
        let line2_vis = wrap_line_to_visual(1, &editor.lines[1], 15);
        all_vis.extend(line2_vis);

        editor.set_cursor(0, 0);
        editor.move_down_visual_lines(&all_vis);
        assert_eq!(editor.cursor_line, 0);
        assert_eq!(editor.cursor_col, all_vis[1].char_start);

        editor.move_up_visual_lines(&all_vis);
        assert_eq!(editor.cursor_line, 0);
        assert_eq!(editor.cursor_col, 0);
    }
}
