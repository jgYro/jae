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
        let mut textarea = TextArea::default();
        // Set cursor to be a visible block with inverted colors
        textarea.set_cursor_style(
            ratatui::style::Style::default()
                .add_modifier(ratatui::style::Modifier::REVERSED)
        );
        // Remove underline from the current line
        textarea.set_cursor_line_style(ratatui::style::Style::default());
        // Set selection style to be visible (highlighted with reversed colors)
        textarea.set_selection_style(
            ratatui::style::Style::default()
                .add_modifier(ratatui::style::Modifier::REVERSED)
        );

        Self {
            textarea,
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
        // Don't reset last_was_kill when setting mark
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
        // Don't reset last_was_kill on movement - only on text changes
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
        // Don't reset last_was_kill on movement
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
            // Don't reset last_was_kill on movement
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
        // Don't reset last_was_kill on movement
    }

    pub fn reset_kill_sequence(&mut self) {
        self.last_was_kill = false;
    }

    pub fn is_at_last_line(&self) -> bool {
        let (row, _) = self.textarea.cursor();
        let lines = self.textarea.lines();
        row == lines.len() - 1 || lines.is_empty()
    }

    pub fn kill_to_end_of_line(&mut self) {
        let (row, col) = self.textarea.cursor();

        // Collect needed information before mutating
        let (killed_text, should_move_to_end) = {
            let lines = self.textarea.lines();

            if row >= lines.len() {
                return;
            }

            let line = &lines[row];

            if col < line.len() {
                // Kill from cursor to end of line
                (line[col..].to_string(), true)
            } else if row + 1 < lines.len() {
                // If at end of line, kill the newline (join with next line)
                ("\n".to_string(), false)
            } else {
                return;
            }
        };

        if !killed_text.is_empty() {
            // Add to kill ring
            if self.last_was_kill {
                self.kill_ring.append_to_last(killed_text.clone());
            } else {
                self.kill_ring.push(killed_text.clone());
            }

            // Select the text to kill
            self.textarea.start_selection();
            if should_move_to_end {
                // Move to end of line
                self.textarea.move_cursor(CursorMove::End);
            } else {
                // Move to beginning of next line (to select the newline)
                self.textarea.move_cursor(CursorMove::Down);
                self.textarea.move_cursor(CursorMove::Head);
            }

            // Cut the selected text
            self.textarea.cut();
            self.mark_active = false;
            self.last_was_kill = true;
        }
    }

    pub fn kill_to_beginning_of_line(&mut self) {
        let (row, col) = self.textarea.cursor();

        // Collect needed information before mutating
        let killed_text = {
            let lines = self.textarea.lines();

            if row >= lines.len() || col == 0 {
                return;
            }

            let line = &lines[row];
            line[0..col].to_string()
        };

        if !killed_text.is_empty() {
            let text_len = killed_text.len();
            // Add to kill ring (C-u doesn't append to previous kills)
            self.kill_ring.push(killed_text);

            // Move to beginning of line
            self.textarea.move_cursor(CursorMove::Head);

            // Start selection and move forward to select the text
            self.textarea.start_selection();
            for _ in 0..text_len {
                self.textarea.move_cursor(CursorMove::Forward);
            }

            // Cut the selected text
            self.textarea.cut();
            self.mark_active = false;
            self.last_was_kill = false; // C-u doesn't continue kill sequence
        }
    }
}