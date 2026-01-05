//! Cursor movement operations for the editor.

use super::Editor;
use tui_textarea::CursorMove;

impl Editor {
    // ==================== Cursor Movement ====================

    pub fn move_cursor(&mut self, movement: CursorMove) {
        self.textarea.move_cursor(movement);
    }

    pub fn move_word_forward(&mut self) {
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();

        if row >= lines.len() {
            return;
        }

        let line = &lines[row];
        let chars: Vec<char> = line.chars().collect();
        let char_count = chars.len();
        let mut new_col = col;

        // Skip current word (including punctuation as word boundaries)
        while new_col < char_count {
            let ch = chars[new_col];
            if ch.is_whitespace() || (!ch.is_alphanumeric() && ch != '_') {
                break;
            }
            new_col += 1;
        }

        // Skip whitespace and punctuation
        while new_col < char_count && !chars[new_col].is_alphanumeric() {
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
    }

    pub fn move_word_backward(&mut self) {
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();

        if row >= lines.len() {
            return;
        }

        let line = &lines[row];
        let chars: Vec<char> = line.chars().collect();

        if col == 0 {
            if row > 0 {
                self.textarea.move_cursor(CursorMove::Up);
                self.textarea.move_cursor(CursorMove::End);
            }
            return;
        }

        let mut new_col = col - 1;

        // Skip whitespace and punctuation
        while new_col > 0 && !chars[new_col].is_alphanumeric() {
            new_col -= 1;
        }

        // Skip word (including underscores as part of word)
        while new_col > 0 {
            let ch = chars[new_col - 1];
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            new_col -= 1;
        }

        for _ in new_col..col {
            self.textarea.move_cursor(CursorMove::Back);
        }
    }

    pub fn is_at_last_line(&self) -> bool {
        let (row, _) = self.textarea.cursor();
        let lines = self.textarea.lines();
        row == lines.len() - 1 || lines.is_empty()
    }

    // ==================== Word Delete Operations ====================
    //
    // These follow the BufferEdit pattern - see buffer_ops.rs for details.

    /// Delete word forward (M-d)
    ///
    /// Returns true if text was deleted, false if at end of document.
    /// Handles undo state and modification tracking internally.
    pub fn delete_word_forward(&mut self) -> bool {
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();

        // Check if there's anything to delete
        let at_end = row >= lines.len()
            || (row == lines.len() - 1 && col >= lines[row].chars().count());

        if at_end {
            return false;
        }

        self.save_undo_state();
        self.textarea.delete_next_word();
        self.mark_modified();
        true
    }

    /// Delete word backward (M-Backspace)
    ///
    /// Returns true if text was deleted, false if at start of document.
    /// Handles undo state and modification tracking internally.
    pub fn delete_word_backward(&mut self) -> bool {
        let (row, col) = self.textarea.cursor();

        // Check if there's anything to delete
        let at_start = row == 0 && col == 0;

        if at_start {
            return false;
        }

        self.save_undo_state();
        self.textarea.delete_word();
        self.mark_modified();
        true
    }
}
