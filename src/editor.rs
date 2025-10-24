use crate::kill_ring::KillRing;
use tui_textarea::{CursorMove, TextArea};

pub struct Editor {
    pub textarea: TextArea<'static>,
    pub mark_active: bool,
    pub kill_ring: KillRing,
    pub last_was_kill: bool,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            textarea: TextArea::default(),
            mark_active: false,
            kill_ring: KillRing::new(),
            last_was_kill: false,
        }
    }

    pub fn set_mark(&mut self) {
        if !self.mark_active {
            self.textarea.start_selection();
            self.mark_active = true;
        } else {
            self.cancel_mark();
        }
        self.last_was_kill = false;
    }

    pub fn cancel_mark(&mut self) {
        self.textarea.cancel_selection();
        self.mark_active = false;
    }

    pub fn get_selected_text(&self) -> Option<String> {
        let (start, end) = self.textarea.selection_range()?;

        if start == end {
            return None;
        }

        let (start, end) = if start > end {
            (end, start)
        } else {
            (start, end)
        };

        let lines = self.textarea.lines();
        let mut selected = String::new();

        for row in start.0..=end.0 {
            if row >= lines.len() {
                break;
            }

            let line = &lines[row];
            let start_col = if row == start.0 { start.1 } else { 0 };
            let end_col = if row == end.0 {
                end.1.min(line.len())
            } else {
                line.len()
            };

            if start_col <= end_col && end_col <= line.len() {
                selected.push_str(&line[start_col..end_col]);
            }

            if row < end.0 {
                selected.push('\n');
            }
        }

        Some(selected)
    }

    pub fn kill_region(&mut self) {
        if let Some(text) = self.get_selected_text() {
            if self.last_was_kill {
                self.kill_ring.append_to_last(text);
            } else {
                self.kill_ring.push(text);
            }
            self.textarea.cut();
            self.mark_active = false;
            self.last_was_kill = true;
        }
    }

    pub fn copy_region(&mut self) {
        if let Some(text) = self.get_selected_text() {
            self.kill_ring.push(text);
            self.cancel_mark();
        }
        self.last_was_kill = false;
    }

    pub fn yank(&mut self) {
        if let Some(text) = self.kill_ring.yank() {
            self.textarea.insert_str(text);
        }
        self.last_was_kill = false;
    }

    pub fn move_cursor(&mut self, movement: CursorMove) {
        self.textarea.move_cursor(movement);
        self.last_was_kill = false;
    }

    pub fn move_word_forward(&mut self) {
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();

        if row >= lines.len() {
            return;
        }

        let line = &lines[row];
        let mut new_col = col;

        // Skip current word
        while new_col < line.len() && !line.chars().nth(new_col).unwrap_or(' ').is_whitespace() {
            new_col += 1;
        }

        // Skip whitespace
        while new_col < line.len() && line.chars().nth(new_col).unwrap_or(' ').is_whitespace() {
            new_col += 1;
        }

        if new_col > col {
            for _ in col..new_col {
                self.textarea.move_cursor(CursorMove::Forward);
            }
        } else if row + 1 < lines.len() {
            self.textarea.move_cursor(CursorMove::Down);
            self.textarea.move_cursor(CursorMove::Head);
        }
        self.last_was_kill = false;
    }

    pub fn move_word_backward(&mut self) {
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();

        if row >= lines.len() {
            return;
        }

        let line = &lines[row];

        if col == 0 {
            if row > 0 {
                self.textarea.move_cursor(CursorMove::Up);
                self.textarea.move_cursor(CursorMove::End);
            }
            self.last_was_kill = false;
            return;
        }

        let mut new_col = col - 1;

        // Skip whitespace
        while new_col > 0 && line.chars().nth(new_col).unwrap_or(' ').is_whitespace() {
            new_col -= 1;
        }

        // Skip word
        while new_col > 0 && !line.chars().nth(new_col - 1).unwrap_or(' ').is_whitespace() {
            new_col -= 1;
        }

        for _ in new_col..col {
            self.textarea.move_cursor(CursorMove::Back);
        }
        self.last_was_kill = false;
    }

    pub fn reset_kill_sequence(&mut self) {
        self.last_was_kill = false;
    }
}