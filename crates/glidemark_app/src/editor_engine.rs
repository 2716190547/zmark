use std::sync::Arc;

const MAX_UNDO_STEPS: usize = 100;
const MAX_UNDO_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorEngine {
    pub lines: Arc<Vec<String>>,
    pub cursor_line: usize,
    pub cursor_col: usize,
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
            lines: Arc::new(lines),
            cursor_line: last_line_idx,
            cursor_col: last_col,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            undo_bytes: 0,
            redo_bytes: 0,
        }
    }

    pub fn reset_text(&mut self, text: &str) {
        self.lines = Arc::new(split_lines(text));
        self.cursor_line = self.cursor_line.min(self.lines.len() - 1);
        self.clamp_cursor();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.undo_bytes = 0;
        self.redo_bytes = 0;
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
    }

    pub fn insert_newline(&mut self) {
        self.push_snapshot();
        self.internal_newline();
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
        } else if self.cursor_line > 0 {
            self.push_snapshot();
            let lines = Arc::make_mut(&mut self.lines);
            let current = lines.remove(self.cursor_line);
            let prev_line = &mut lines[self.cursor_line - 1];
            let prev_len = prev_line.chars().count();
            prev_line.push_str(&current);
            self.cursor_line -= 1;
            self.cursor_col = prev_len;
        }
    }

    pub fn delete_forward(&mut self) {
        let line_len = self.lines[self.cursor_line].chars().count();
        if self.cursor_col < line_len {
            self.push_snapshot();
            let line = &mut Arc::make_mut(&mut self.lines)[self.cursor_line];
            let mut chars: Vec<char> = line.chars().collect();
            chars.remove(self.cursor_col);
            *line = chars.into_iter().collect();
        } else if self.cursor_line + 1 < self.lines.len() {
            self.push_snapshot();
            let lines = Arc::make_mut(&mut self.lines);
            let next = lines.remove(self.cursor_line + 1);
            lines[self.cursor_line].push_str(&next);
        }
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

    pub fn move_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.clamp_cursor();
        }
    }

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
            self.cursor_line = self.cursor_line.min(self.lines.len() - 1);
            self.clamp_cursor();
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
            self.cursor_line = self.cursor_line.min(self.lines.len() - 1);
            self.clamp_cursor();
            true
        } else {
            false
        }
    }
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
}
