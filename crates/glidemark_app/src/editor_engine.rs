#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorEngine {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
}

impl EditorEngine {
    pub fn new(initial_text: &str) -> Self {
        let lines: Vec<String> = if initial_text.is_empty() {
            vec![String::new()]
        } else {
            initial_text.split('\n').map(|s| s.to_string()).collect()
        };
        let last_line_idx = lines.len() - 1;
        let last_col = lines[last_line_idx].chars().count();
        Self {
            lines,
            cursor_line: last_line_idx,
            cursor_col: last_col,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn reset_text(&mut self, text: &str) {
        self.lines = if text.is_empty() {
            vec![String::new()]
        } else {
            text.split('\n').map(|s| s.to_string()).collect()
        };
        self.cursor_line = self.cursor_line.min(self.lines.len() - 1);
        self.clamp_cursor();
        self.undo_stack.clear();
        self.redo_stack.clear();
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
        self.undo_stack.push(self.to_text());
        self.redo_stack.clear();
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
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
        let line = &mut self.lines[self.cursor_line];
        let mut chars: Vec<char> = line.chars().collect();
        chars.insert(self.cursor_col, ch);
        *line = chars.into_iter().collect();
        self.cursor_col += 1;
    }

    fn internal_newline(&mut self) {
        let line = &self.lines[self.cursor_line];
        let chars: Vec<char> = line.chars().collect();
        let left: String = chars[..self.cursor_col].iter().collect();
        let right: String = chars[self.cursor_col..].iter().collect();

        self.lines[self.cursor_line] = left;
        self.lines.insert(self.cursor_line + 1, right);
        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    pub fn delete_backward(&mut self) {
        if self.cursor_col > 0 {
            self.push_snapshot();
            let line = &mut self.lines[self.cursor_line];
            let mut chars: Vec<char> = line.chars().collect();
            chars.remove(self.cursor_col - 1);
            *line = chars.into_iter().collect();
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            self.push_snapshot();
            let current = self.lines.remove(self.cursor_line);
            let prev_line = &mut self.lines[self.cursor_line - 1];
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
            let line = &mut self.lines[self.cursor_line];
            let mut chars: Vec<char> = line.chars().collect();
            chars.remove(self.cursor_col);
            *line = chars.into_iter().collect();
        } else if self.cursor_line + 1 < self.lines.len() {
            self.push_snapshot();
            let next = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next);
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
            self.redo_stack.push(self.to_text());
            self.lines = prev.split('\n').map(|s| s.to_string()).collect();
            self.cursor_line = self.cursor_line.min(self.lines.len() - 1);
            self.clamp_cursor();
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.to_text());
            self.lines = next.split('\n').map(|s| s.to_string()).collect();
            self.cursor_line = self.cursor_line.min(self.lines.len() - 1);
            self.clamp_cursor();
            true
        } else {
            false
        }
    }
}
