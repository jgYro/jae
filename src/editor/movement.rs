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
        // Adjust scroll only if cursor went off-screen
        self.ensure_cursor_visible();
        if logging::log_movement() || logging::log_selection() {
            log::debug!(
                "move_cursor after: cursor={:?}, selection_range={:?}",
                self.textarea.cursor(),
                self.textarea.selection_range()
            );
        }
    }

    /// Ensure cursor is visible, adjusting scroll_offset minimally if needed
    fn ensure_cursor_visible(&mut self) {
        let (cursor_row, _) = self.textarea.cursor();
        let viewport_height = self.viewport_height as usize;

        // If cursor is above visible area, scroll up
        if cursor_row < self.scroll_offset {
            self.scroll_offset = cursor_row;
        }
        // If cursor is below visible area, scroll down
        else if cursor_row >= self.scroll_offset + viewport_height {
            self.scroll_offset = cursor_row - viewport_height + 1;
        }
        // Otherwise, cursor is visible, don't change scroll

        // Any non-recenter action clears the consecutive recenter flag
        self.last_was_recenter = false;
    }

    /// Center the cursor in the viewport
    fn center_cursor(&mut self) {
        let (cursor_row, _) = self.textarea.cursor();
        let viewport_height = self.viewport_height as usize;
        let half_height = viewport_height / 2;
        self.scroll_offset = cursor_row.saturating_sub(half_height);
    }

    pub fn move_word_forward(&mut self) {
        // Use tui-textarea's built-in word movement which properly handles selections
        self.textarea.move_cursor(CursorMove::WordForward);
        self.ensure_cursor_visible();
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
        self.ensure_cursor_visible();
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

    /// Move cursor up by half page then center (like Vim C-u + zz)
    pub fn page_up(&mut self) {
        let half_page = (self.viewport_height as usize) / 2;
        let (row, col) = self.textarea.cursor();
        let new_row = row.saturating_sub(half_page);
        self.textarea.move_cursor(CursorMove::Jump(new_row as u16, col as u16));
        // Center the cursor after moving
        self.center_cursor();
        self.last_was_recenter = false;
    }

    /// Move cursor down by half page then center (like Vim C-d + zz)
    pub fn page_down(&mut self) {
        let half_page = (self.viewport_height as usize) / 2;
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();
        let max_row = lines.len().saturating_sub(1);
        let new_row = (row + half_page).min(max_row);
        self.textarea.move_cursor(CursorMove::Jump(new_row as u16, col as u16));
        // Center the cursor after moving
        self.center_cursor();
        self.last_was_recenter = false;
    }

    /// Recenter the view - always centers first, cycles only on consecutive presses
    /// Cycle: center -> top -> bottom -> center (with blank space allowed)
    pub fn recenter(&mut self) {
        use super::core::RecenterState;
        let (cursor_row, _) = self.textarea.cursor();
        let viewport_height = self.viewport_height as usize;

        // If last command wasn't recenter, always start with center
        // If it was recenter, cycle to next state
        if self.last_was_recenter {
            self.recenter_state = match self.recenter_state {
                RecenterState::Normal => RecenterState::Center,
                RecenterState::Center => RecenterState::Top,
                RecenterState::Top => RecenterState::Bottom,
                RecenterState::Bottom => RecenterState::Center,
            };
        } else {
            self.recenter_state = RecenterState::Center;
        }

        // Set the scroll offset to position cursor at the desired location
        self.scroll_offset = match self.recenter_state {
            RecenterState::Normal => {
                // Should not happen
                self.scroll_offset
            }
            RecenterState::Center => {
                // Put cursor in center of viewport
                let half_height = viewport_height / 2;
                cursor_row.saturating_sub(half_height)
            }
            RecenterState::Top => {
                // Put cursor at top of viewport (blank space below allowed)
                cursor_row
            }
            RecenterState::Bottom => {
                // Put cursor at bottom of viewport (blank space above allowed)
                cursor_row.saturating_sub(viewport_height.saturating_sub(1))
            }
        };

        // Mark that we just did a recenter
        self.last_was_recenter = true;
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

    /// Delete character forward (C-d)
    ///
    /// Returns true if text was deleted, false if at end of document.
    /// Handles undo state and modification tracking internally.
    pub fn delete_char_forward(&mut self) -> bool {
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();

        // Check if there's anything to delete
        let at_end = row >= lines.len()
            || (row == lines.len() - 1 && col >= lines[row].chars().count());

        if at_end {
            return false;
        }

        self.save_undo_state();
        self.textarea.delete_next_char();
        self.mark_modified();
        true
    }

    /// Delete character backward (C-h)
    ///
    /// Returns true if text was deleted, false if at start of document.
    /// Handles undo state and modification tracking internally.
    pub fn delete_char_backward(&mut self) -> bool {
        let (row, col) = self.textarea.cursor();

        // Check if there's anything to delete
        let at_start = row == 0 && col == 0;

        if at_start {
            return false;
        }

        self.save_undo_state();
        self.textarea.delete_char();
        self.mark_modified();
        true
    }
}
