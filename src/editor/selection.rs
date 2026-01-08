//! Selection and clipboard operations for the editor.

use super::{Editor, MarkState};
use crate::logging;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use std::cmp::min;
use tui_textarea::CursorMove;

impl Editor {
    // ==================== Mark/Selection Operations ====================

    /// Set or toggle the mark (C-SPC)
    pub fn set_mark(&mut self) {
        let cursor_pos = self.textarea.cursor();
        if logging::log_selection() {
            log::debug!(
                "set_mark: cursor={:?}, current mark={:?}, is_selecting={}",
                cursor_pos,
                self.mark,
                self.textarea.is_selecting()
            );
        }

        // Validate cursor position is within document bounds
        let lines = self.textarea.lines();
        if cursor_pos.0 >= lines.len() {
            return;
        }
        if cursor_pos.0 < lines.len() && cursor_pos.1 > lines[cursor_pos.0].chars().count() {
            return;
        }

        // Check if this is a double C-SPC (C-SPC C-SPC)
        if self.last_key == Some((KeyCode::Char(' '), KeyModifiers::CONTROL)) {
            // Second C-SPC: Set mark but deactivate region
            match self.mark {
                MarkState::None => {
                    self.mark = MarkState::Set {
                        row: cursor_pos.0,
                        col: cursor_pos.1,
                    };
                }
                MarkState::Active { row, col } => {
                    self.textarea.cancel_selection();
                    self.mark = MarkState::Set { row, col };
                }
                MarkState::Set { .. } => {
                    // Already set and inactive, do nothing
                }
            }
        } else {
            // First C-SPC: Set mark and activate region
            match self.mark {
                MarkState::None | MarkState::Set { .. } => {
                    // Setting new mark or reactivating
                    self.mark = MarkState::Active {
                        row: cursor_pos.0,
                        col: cursor_pos.1,
                    };
                    self.textarea.start_selection();
                }
                MarkState::Active { row, col } => {
                    // If already active, deactivate (toggle behavior)
                    self.textarea.cancel_selection();
                    self.mark = MarkState::Set { row, col };
                    // Clear last_key so next C-SPC starts a fresh selection
                    self.last_key = None;
                }
            }
        }
    }

    /// Cancel the active selection (C-g)
    pub fn cancel_mark(&mut self) {
        self.textarea.cancel_selection();
        // Keep mark position for navigation but deactivate
        match self.mark {
            MarkState::Active { row, col } => {
                self.mark = MarkState::Set { row, col };
            }
            _ => {}
        }
    }

    /// Exchange point and mark (C-x C-x)
    pub fn swap_cursor_mark(&mut self) {
        let mark_pos = match self.mark.position() {
            Some(pos) => pos,
            None => {
                // No mark to swap with - just set mark here
                self.set_mark();
                return;
            }
        };

        let current_cursor = self.textarea.cursor();

        // Validate both positions are still within document bounds
        let lines = self.textarea.lines();
        if mark_pos.0 >= lines.len() || current_cursor.0 >= lines.len() {
            self.mark = MarkState::None;
            self.textarea.cancel_selection();
            return;
        }

        // If they're the same, just activate region if needed
        if current_cursor == mark_pos {
            if !self.mark.is_active() {
                self.textarea.start_selection();
                self.mark = MarkState::Active {
                    row: mark_pos.0,
                    col: mark_pos.1,
                };
            }
            return;
        }

        // Save the current cursor position as the new mark
        let new_mark_pos = current_cursor;

        if self.mark.is_active() {
            // Selection is active - preserve it while swapping ends
            self.textarea.cancel_selection();

            // Move cursor to current position (new anchor)
            let jump_row = min(current_cursor.0, u16::MAX as usize) as u16;
            let jump_col = min(current_cursor.1, u16::MAX as usize) as u16;
            self.textarea.move_cursor(CursorMove::Jump(jump_row, jump_col));

            // Start selection from current cursor position
            self.textarea.start_selection();

            // Move cursor to the saved mark (where cursor will end up)
            let mark_row = min(mark_pos.0, u16::MAX as usize) as u16;
            let mark_col = min(mark_pos.1, u16::MAX as usize) as u16;
            self.textarea.move_cursor(CursorMove::Jump(mark_row, mark_col));

            self.mark = MarkState::Active {
                row: new_mark_pos.0,
                col: new_mark_pos.1,
            };
        } else {
            // No active selection, create one between cursor and mark
            let jump_row = min(current_cursor.0, u16::MAX as usize) as u16;
            let jump_col = min(current_cursor.1, u16::MAX as usize) as u16;
            self.textarea.move_cursor(CursorMove::Jump(jump_row, jump_col));

            self.textarea.start_selection();

            let mark_row = min(mark_pos.0, u16::MAX as usize) as u16;
            let mark_col = min(mark_pos.1, u16::MAX as usize) as u16;
            self.textarea.move_cursor(CursorMove::Jump(mark_row, mark_col));

            self.mark = MarkState::Active {
                row: new_mark_pos.0,
                col: new_mark_pos.1,
            };
        }
    }

    /// Get the currently selected text
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
                end.1.min(line.chars().count())
            } else {
                line.chars().count()
            };

            // Convert character indices to byte indices for proper UTF-8 slicing
            let mut start_byte = 0;
            let mut end_byte = line.len();

            for (char_pos, (byte_idx, _)) in line.char_indices().enumerate() {
                if char_pos == start_col {
                    start_byte = byte_idx;
                }
                if char_pos == end_col {
                    end_byte = byte_idx;
                    break;
                }
            }

            if start_byte <= end_byte && end_byte <= line.len() {
                selected.push_str(&line[start_byte..end_byte]);
            }

            if row < end.0 {
                selected.push('\n');
            }
        }

        Some(selected)
    }

    // ==================== Clipboard Operations ====================
    //
    // All buffer-modifying operations follow the BufferEdit pattern:
    // 1. Check if operation will actually modify anything
    // 2. If not, return false immediately (no side effects)
    // 3. If yes, save undo state BEFORE modifying
    // 4. Perform the modification
    // 5. Mark buffer as modified
    // 6. Return true
    //
    // See buffer_ops.rs for full documentation.

    /// Cut selected text to system clipboard (C-w)
    ///
    /// Returns true if text was actually cut, false if no selection.
    /// Handles undo state and modification tracking internally.
    pub fn cut_region(&mut self) -> bool {
        match self.get_selected_text() {
            Some(text) => {
                self.save_undo_state();
                self.clipboard.copy(&text);
                self.textarea.cut();
                self.mark = MarkState::None;
                self.mark_modified();
                true
            }
            None => false,
        }
    }

    /// Copy selected text to system clipboard (M-w)
    ///
    /// Does not modify buffer, so no undo state or modification tracking needed.
    pub fn copy_region(&mut self) {
        match self.get_selected_text() {
            Some(text) => {
                self.clipboard.copy(&text);
                self.cancel_mark();
            }
            None => {}
        }
    }

    /// Paste from system clipboard (C-y)
    ///
    /// Returns true if text was actually pasted, false if clipboard empty.
    /// Handles undo state and modification tracking internally.
    pub fn paste(&mut self) -> bool {
        match self.clipboard.paste() {
            Some(text) => {
                self.save_undo_state();
                self.textarea.insert_str(&text);
                self.mark_modified();
                true
            }
            None => false,
        }
    }

    /// Cut from cursor to end of line to clipboard (C-k)
    ///
    /// Returns true if text was actually cut, false if nothing to cut.
    /// Handles undo state and modification tracking internally.
    pub fn cut_to_end_of_line(&mut self) -> bool {
        let (row, col) = self.textarea.cursor();

        let (cut_text, should_move_to_end) = {
            let lines = self.textarea.lines();

            if row >= lines.len() {
                return false;
            }

            let line = &lines[row];

            if col < line.chars().count() {
                (Self::safe_string_slice(line, col, line.chars().count()), true)
            } else if row + 1 < lines.len() {
                ("\n".to_string(), false)
            } else {
                return false;
            }
        };

        if !cut_text.is_empty() {
            self.save_undo_state();
            self.clipboard.copy(&cut_text);

            self.textarea.start_selection();
            if should_move_to_end {
                self.textarea.move_cursor(CursorMove::End);
            } else {
                self.textarea.move_cursor(CursorMove::Down);
                self.textarea.move_cursor(CursorMove::Head);
            }

            self.textarea.cut();
            self.mark = MarkState::None;
            self.mark_modified();
            true
        } else {
            false
        }
    }

    /// Cut from beginning of line to cursor to clipboard (C-u)
    ///
    /// Returns true if text was actually cut, false if nothing to cut.
    /// Handles undo state and modification tracking internally.
    pub fn cut_to_beginning_of_line(&mut self) -> bool {
        let (row, col) = self.textarea.cursor();

        let cut_text = {
            let lines = self.textarea.lines();

            if row >= lines.len() || col == 0 {
                return false;
            }

            let line = &lines[row];
            Self::safe_string_slice(line, 0, col)
        };

        if !cut_text.is_empty() {
            self.save_undo_state();
            self.clipboard.copy(&cut_text);

            self.textarea.move_cursor(CursorMove::Head);
            self.textarea.start_selection();
            for _ in 0..col {
                self.textarea.move_cursor(CursorMove::Forward);
            }

            self.textarea.cut();
            self.mark = MarkState::None;
            self.mark_modified();
            true
        } else {
            false
        }
    }

    // ==================== Syntax-Aware Selection ====================

    /// Expand selection to parent syntax node (Alt-o, like Helix)
    ///
    /// If no selection is active, selects the smallest node at cursor.
    /// If selection is active, expands to the parent node.
    /// Pushes current selection to history for shrink operation.
    pub fn expand_selection(&mut self) {
        let syntax_state = match &self.syntax_state {
            Some(s) => s,
            None => return, // No syntax state, nothing to do
        };

        let source = self.textarea.lines().join("\n");

        // Get current selection or cursor position
        let (start_byte, end_byte) = match self.textarea.selection_range() {
            Some(((start_row, start_col), (end_row, end_col))) => {
                // Convert row/col to byte offsets
                let start = self.row_col_to_byte(&source, start_row, start_col);
                let end = self.row_col_to_byte(&source, end_row, end_col);
                match start <= end {
                    true => (start, end),
                    false => (end, start),
                }
            }
            None => {
                // No selection, use cursor position
                let (row, col) = self.textarea.cursor();
                let byte_pos = self.row_col_to_byte(&source, row, col);
                (byte_pos, byte_pos)
            }
        };

        // Get parent node range
        let new_range = match syntax_state.get_parent_node_range(start_byte, end_byte) {
            Some(range) => range,
            None => return,
        };

        // Only expand if we're actually getting a larger range
        match new_range.0 < start_byte || new_range.1 > end_byte {
            true => {
                // Save current selection to history for shrink
                self.selection_history.push((start_byte, end_byte));

                // Convert byte range back to row/col and set selection
                self.set_selection_from_bytes(&source, new_range.0, new_range.1);
            }
            false => {}
        }
    }

    /// Shrink selection to previous or child syntax node (Alt-i, like Helix)
    ///
    /// If there's selection history, pops the previous selection.
    /// Otherwise, tries to shrink to the first child node.
    pub fn shrink_selection(&mut self) {
        let source = self.textarea.lines().join("\n");

        // First, try to restore from history
        match self.selection_history.pop() {
            Some((start_byte, end_byte)) => {
                self.set_selection_from_bytes(&source, start_byte, end_byte);
                return;
            }
            None => {}
        }

        // No history, try to shrink to child node
        let syntax_state = match &self.syntax_state {
            Some(s) => s,
            None => return,
        };

        // Get current selection
        let (start_byte, end_byte) = match self.textarea.selection_range() {
            Some(((start_row, start_col), (end_row, end_col))) => {
                let start = self.row_col_to_byte(&source, start_row, start_col);
                let end = self.row_col_to_byte(&source, end_row, end_col);
                match start <= end {
                    true => (start, end),
                    false => (end, start),
                }
            }
            None => return, // No selection to shrink
        };

        // Get child node range
        match syntax_state.get_child_node_range(start_byte, end_byte) {
            Some((new_start, new_end)) => {
                // Only shrink if we're actually getting a smaller range
                match new_start > start_byte || new_end < end_byte {
                    true => {
                        self.set_selection_from_bytes(&source, new_start, new_end);
                    }
                    false => {}
                }
            }
            None => {}
        }
    }

    /// Clear selection history (called when selection is manually changed)
    pub fn clear_selection_history(&mut self) {
        self.selection_history.clear();
    }

    // ==================== Helper Methods ====================

    /// Convert row/col to byte offset in source
    fn row_col_to_byte(&self, source: &str, row: usize, col: usize) -> usize {
        let mut byte_offset = 0;
        for (line_idx, line) in source.lines().enumerate() {
            match line_idx == row {
                true => {
                    // Count bytes up to col
                    for (char_idx, ch) in line.chars().enumerate() {
                        match char_idx >= col {
                            true => break,
                            false => byte_offset += ch.len_utf8(),
                        }
                    }
                    return byte_offset;
                }
                false => {
                    byte_offset += line.len() + 1; // +1 for newline
                }
            }
        }
        byte_offset
    }

    /// Convert byte offset to row/col
    fn byte_to_row_col(&self, source: &str, byte_offset: usize) -> (usize, usize) {
        let mut current_byte = 0;
        for (line_idx, line) in source.lines().enumerate() {
            let line_end = current_byte + line.len();
            match byte_offset <= line_end {
                true => {
                    // This is the line - find column
                    let offset_in_line = byte_offset - current_byte;
                    let mut col = 0;
                    let mut byte_count = 0;
                    for ch in line.chars() {
                        match byte_count >= offset_in_line {
                            true => break,
                            false => {
                                byte_count += ch.len_utf8();
                                col += 1;
                            }
                        }
                    }
                    return (line_idx, col);
                }
                false => {
                    current_byte = line_end + 1; // +1 for newline
                }
            }
        }
        // Past end of document
        let line_count = source.lines().count();
        match line_count {
            0 => (0, 0),
            n => (n - 1, source.lines().last().map(|l| l.chars().count()).unwrap_or(0)),
        }
    }

    /// Set selection from byte range
    fn set_selection_from_bytes(&mut self, source: &str, start_byte: usize, end_byte: usize) {
        let (start_row, start_col) = self.byte_to_row_col(source, start_byte);
        let (end_row, end_col) = self.byte_to_row_col(source, end_byte);

        // Move cursor to start, start selection, move to end
        let start_row_u16 = min(start_row, u16::MAX as usize) as u16;
        let start_col_u16 = min(start_col, u16::MAX as usize) as u16;
        let end_row_u16 = min(end_row, u16::MAX as usize) as u16;
        let end_col_u16 = min(end_col, u16::MAX as usize) as u16;

        self.textarea.move_cursor(CursorMove::Jump(start_row_u16, start_col_u16));
        self.textarea.start_selection();
        self.textarea.move_cursor(CursorMove::Jump(end_row_u16, end_col_u16));

        self.mark = MarkState::Active {
            row: start_row,
            col: start_col,
        };
    }
}
