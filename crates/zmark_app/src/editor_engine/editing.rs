use super::*;
use std::ops::Range;

impl EditorEngine {
    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
        self.column_ranges.clear();
    }

    pub fn is_column_selection(&self) -> bool {
        !self.column_ranges.is_empty()
    }

    pub fn selection_reversed(&self) -> bool {
        self.selection_anchor
            .is_some_and(|anchor| anchor > self.cursor())
    }

    pub fn selection_ranges(&self) -> Vec<Range<usize>> {
        if self.is_column_selection() {
            self.column_ranges.clone()
        } else {
            self.selection()
                .map(|(start, end)| start..end)
                .into_iter()
                .collect()
        }
    }

    pub fn position_for_offset(&self, offset: usize) -> (usize, usize) {
        position_for_char_offset(&self.lines, offset.min(self.char_count))
    }

    pub fn set_selection_offsets(&mut self, start: usize, end: usize, reversed: bool) {
        self.clear_selection();
        let anchor = if reversed { end } else { start };
        let head = if reversed { start } else { end };
        self.selection_anchor = Some(self.position_for_offset(anchor));
        (self.cursor_line, self.cursor_col) = self.position_for_offset(head);
    }

    pub fn set_column_ranges(&mut self, mut ranges: Vec<Range<usize>>) {
        ranges.retain(|r| r.start <= r.end && r.end <= self.char_count);
        ranges.sort_by_key(|r| (r.start, r.end));
        ranges.dedup();
        let mut unique: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
        for range in ranges {
            if let Some(previous) = unique.last_mut() {
                if range.start < previous.end || (range.is_empty() && range.start == previous.end) {
                    previous.end = previous.end.max(range.end);
                    continue;
                }
            }
            unique.push(range);
        }
        let ranges = unique;
        self.selection_anchor = None;
        if let Some(last) = ranges.last() {
            (self.cursor_line, self.cursor_col) = self.position_for_offset(last.end);
        }
        self.column_ranges = ranges;
    }

    /// Ranges address one original buffer. Validate all edits before touching history or text.
    pub fn replace_ranges(
        &mut self,
        ranges: &[Range<usize>],
        replacements: &[String],
    ) -> Result<(), &'static str> {
        if ranges.is_empty() || ranges.len() != replacements.len() {
            return Err("Invalid edit ranges");
        }
        let mut previous_end = 0;
        for (index, range) in ranges.iter().enumerate() {
            if range.start > range.end
                || range.end > self.char_count
                || (index > 0 && range.start < previous_end)
            {
                return Err("Overlapping or invalid edit ranges");
            }
            previous_end = range.end;
        }
        let inserted_bytes = replacements
            .iter()
            .try_fold(0usize, |bytes, text| bytes.checked_add(text.len()));
        if inserted_bytes.is_none_or(|bytes| bytes > MAX_UNDO_BYTES) {
            return Err("Edit exceeds the input budget");
        }
        let source = self.to_text();
        let mut updated = String::new();
        let mut previous_byte = 0;
        let mut carets = Vec::with_capacity(ranges.len());
        let mut delta: isize = 0;
        let mut boundaries = source
            .char_indices()
            .map(|(byte, _)| byte)
            .chain(std::iter::once(source.len()))
            .enumerate()
            .peekable();
        for (range, replacement) in ranges.iter().zip(replacements) {
            while boundaries
                .peek()
                .is_some_and(|(index, _)| *index < range.start)
            {
                boundaries.next();
            }
            let start = boundaries
                .peek()
                .filter(|(index, _)| *index == range.start)
                .map(|(_, byte)| *byte)
                .ok_or("Invalid start")?;
            while boundaries
                .peek()
                .is_some_and(|(index, _)| *index < range.end)
            {
                boundaries.next();
            }
            let end = boundaries
                .peek()
                .filter(|(index, _)| *index == range.end)
                .map(|(_, byte)| *byte)
                .ok_or("Invalid end")?;
            let inserted = replacement.chars().count();
            updated.push_str(&source[previous_byte..start]);
            updated.push_str(replacement);
            carets.push((range.start as isize + delta) as usize + inserted);
            delta += inserted as isize - range.len() as isize;
            previous_byte = end;
        }
        updated.push_str(&source[previous_byte..]);
        let changed = updated != source;
        let column = self.is_column_selection();
        if changed {
            self.push_snapshot();
            self.lines = Arc::new(split_lines(&updated));
            self.char_count = count_chars(&self.lines);
        }
        self.clear_selection();
        if let Some(&last) = carets.last() {
            (self.cursor_line, self.cursor_col) = self.position_for_offset(last);
        }
        if column {
            self.set_column_ranges(carets.into_iter().map(|pos| pos..pos).collect());
        }
        if changed {
            self.revision += 1;
        }
        Ok(())
    }

    pub fn paste_text(&mut self, text: &str) -> Result<(), &'static str> {
        if !self.is_column_selection() {
            let offset = self.offset_for_position(self.cursor());
            let range = self
                .selection()
                .map_or(offset..offset, |(start, end)| start..end);
            return self.replace_ranges(&[range], &[text.to_owned()]);
        }
        let normalized = text.replace("\r\n", "\n");
        let parts: Vec<_> = normalized.split('\n').collect();
        let ranges = self.column_ranges.clone();
        if parts.len() != 1 && parts.len() != ranges.len() {
            return Err("Column paste line count does not match the selection");
        }
        let bytes = if parts.len() == 1 {
            text.len().checked_mul(ranges.len())
        } else {
            Some(text.len())
        };
        if bytes.is_none_or(|bytes| bytes > MAX_UNDO_BYTES) {
            return Err("Column paste exceeds the edit budget");
        }
        let replacements = if parts.len() == 1 {
            vec![parts[0].to_owned(); ranges.len()]
        } else {
            parts.into_iter().map(str::to_owned).collect()
        };
        self.replace_ranges(&ranges, &replacements)
    }

    pub fn move_by_word(&mut self, right: bool, extend: bool) {
        let text = self.to_text();
        let chars: Vec<_> = text.chars().collect();
        let mut offset = self.offset_for_position(self.cursor());
        let is_word = |ch: char| ch.is_alphanumeric() || ch == '_';
        if right {
            if offset < chars.len() && is_word(chars[offset]) {
                while offset < chars.len() && is_word(chars[offset]) {
                    offset += 1;
                }
            } else if offset < chars.len() && !chars[offset].is_whitespace() {
                offset += 1;
            }
            while offset < chars.len() && chars[offset].is_whitespace() {
                offset += 1;
            }
        } else {
            while offset > 0 && chars[offset - 1].is_whitespace() {
                offset -= 1;
            }
            if offset > 0 && is_word(chars[offset - 1]) {
                while offset > 0 && is_word(chars[offset - 1]) {
                    offset -= 1;
                }
            } else {
                offset = offset.saturating_sub(1);
            }
        }
        let (line, col) = self.position_for_offset(offset);
        if extend {
            self.select_to(line, col, true);
        } else {
            self.set_cursor(line, col);
        }
    }

    pub fn select_word(&mut self, line: usize, col: usize) {
        let chars: Vec<_> = self.lines[line].chars().collect();
        let at = col.min(chars.len().saturating_sub(1));
        let kind = |ch: char| {
            if ch.is_whitespace() {
                0
            } else if ch.is_alphanumeric() || ch == '_' {
                1
            } else {
                2
            }
        };
        let mut start = at;
        let mut end = (at + 1).min(chars.len());
        if let Some(&ch) = chars.get(at) {
            while start > 0 && kind(chars[start - 1]) == kind(ch) {
                start -= 1;
            }
            while end < chars.len() && kind(chars[end]) == kind(ch) {
                end += 1;
            }
        }
        self.set_cursor(line, start);
        self.select_to(line, end, false);
    }

    pub fn select_line(&mut self, line: usize) {
        self.set_cursor(line, 0);
        if line + 1 < self.lines.len() {
            self.select_to(line + 1, 0, false);
        } else {
            self.select_to(line, self.lines[line].chars().count(), false);
        }
    }

    fn selected_physical_lines(&self) -> Vec<usize> {
        let mut result = Vec::new();
        let mut ranges = self.selection_ranges();
        if ranges.is_empty() {
            let pos = self.offset_for_position(self.cursor());
            ranges.push(pos..pos);
        }
        for range in ranges {
            let (first, _) = self.position_for_offset(range.start);
            let (mut last, col) = self.position_for_offset(range.end);
            if last > first && col == 0 {
                last -= 1;
            }
            result.extend(first..=last);
        }
        result.sort_unstable();
        result.dedup();
        result
    }

    pub fn indent_lines(&mut self, tab_size: usize, outdent: bool) {
        let lines = self.selected_physical_lines();
        if self.selection().is_none() && !self.is_column_selection() && !outdent {
            self.insert_text(&" ".repeat(tab_size.clamp(1, 16)));
            return;
        }
        let selected = self.selection().map(|(start, end)| {
            (
                self.position_for_offset(start),
                self.position_for_offset(end),
            )
        });
        let columns: Vec<_> = self
            .column_ranges
            .iter()
            .map(|range| {
                (
                    self.position_for_offset(range.start),
                    self.position_for_offset(range.end),
                )
            })
            .collect();
        let reversed = self.selection_reversed();
        let mut ranges = Vec::new();
        let mut replacements = Vec::new();
        for line in lines.iter().copied() {
            let offset = self.offset_for_position((line, 0));
            let remove = if outdent {
                if self.lines[line].starts_with('\t') {
                    1
                } else {
                    self.lines[line]
                        .chars()
                        .take(tab_size.clamp(1, 16))
                        .take_while(|ch| *ch == ' ')
                        .count()
                }
            } else {
                0
            };
            ranges.push(offset..offset + remove);
            replacements.push(if outdent {
                String::new()
            } else {
                " ".repeat(tab_size.clamp(1, 16))
            });
        }
        if self.replace_ranges(&ranges, &replacements).is_err() {
            return;
        }
        let adjusted = |(line, col): (usize, usize)| {
            let col = if let Ok(index) = lines.binary_search(&line) {
                col.saturating_sub(ranges[index].len()) + replacements[index].chars().count()
            } else {
                col
            };
            self.offset_for_position((line, col.min(self.line_len(line))))
        };
        if let Some((start, end)) = selected {
            let (start, end) = (adjusted(start), adjusted(end));
            self.set_selection_offsets(start, end, reversed);
        } else if !columns.is_empty() {
            let columns = columns
                .into_iter()
                .map(|(start, end)| adjusted(start)..adjusted(end))
                .collect();
            self.set_column_ranges(columns);
        }
    }

    pub fn toggle_numbered_list(&mut self) {
        if self.is_column_selection() {
            return;
        }
        let lines = self.selected_physical_lines();
        let parsed = zmark_core::ast::parse_markdown(&self.to_text());
        let eligible: Vec<_> = lines
            .into_iter()
            .filter(|&index| {
                !self.lines[index].trim().is_empty()
                    && !parsed.code_blocks.iter().any(|block| {
                        index >= block.fence_open_line
                            && index <= block.fence_close_line.unwrap_or(self.lines.len() - 1)
                    })
            })
            .collect();
        if eligible.is_empty() {
            return;
        }
        let remove = eligible
            .iter()
            .all(|&line| ordered_marker(self.lines[line].trim_start()).is_some());
        let first = *eligible.first().unwrap();
        let last = *eligible.last().unwrap();
        let mut ranges = Vec::new();
        let mut replacements = Vec::new();
        for (number, line) in eligible.into_iter().enumerate() {
            let text = &self.lines[line];
            let indent = text
                .chars()
                .take_while(|ch| *ch == ' ' || *ch == '\t')
                .count();
            let offset = self.offset_for_position((line, indent));
            let marker = ordered_marker(text.trim_start()).map_or(0, |(_, len)| len);
            ranges.push(offset..offset + marker);
            replacements.push(if remove {
                String::new()
            } else {
                format!("{}. ", number + 1)
            });
        }
        let reversed = self.selection_reversed();
        if self.replace_ranges(&ranges, &replacements).is_err() {
            return;
        }
        self.set_selection_offsets(
            self.offset_for_position((first, 0)),
            self.offset_for_position((last, self.line_len(last))),
            reversed,
        );
    }
}

pub(super) fn ordered_marker(text: &str) -> Option<(usize, usize)> {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || !text[digits..].starts_with(". ") {
        return None;
    }
    Some((text[..digits].parse().ok()?, digits + 2))
}
