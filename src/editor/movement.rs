//! Cursor movement operations for the editor.

use super::Editor;
use crate::logging;
use tui_textarea::CursorMove;

impl Editor {
    // ==================== Cursor Movement ====================

    pub fn move_cursor(&mut self, movement: CursorMove) {
        if logging::log_movement() {
            log::debug!(
                "move_cursor({:?}): cursor={:?}, selection_range={:?}, is_selecting={}",
                movement,
                self.textarea.cursor(),
                self.textarea.selection_range(),
                self.textarea.is_selecting()
            );
        }
        self.textarea.move_cursor(movement);
        if logging::log_movement() || logging::log_selection() {
            log::debug!(
                "move_cursor after: cursor={:?}, selection_range={:?}",
                self.textarea.cursor(),
                self.textarea.selection_range()
            );
        }
    }

    pub fn move_word_forward(&mut self) {
        // Use tui-textarea's built-in word movement which properly handles selections
        self.textarea.move_cursor(CursorMove::WordForward);
    }

    pub fn move_word_backward(&mut self) {
        if logging::log_movement() {
            log::debug!(
                "move_word_backward: cursor={:?}, selection_range={:?}, is_selecting={}",
                self.textarea.cursor(),
                self.textarea.selection_range(),
                self.textarea.is_selecting()
            );
        }
        // Use tui-textarea's built-in word movement which properly handles selections
        self.textarea.move_cursor(CursorMove::WordBack);
        if logging::log_movement() || logging::log_selection() {
            log::debug!(
                "move_word_backward after: cursor={:?}, selection_range={:?}",
                self.textarea.cursor(),
                self.textarea.selection_range()
            );
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
