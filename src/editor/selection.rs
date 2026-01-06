//! Selection and clipboard operations for the editor.

use super::{Editor, MarkState};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use std::cmp::min;
use tui_textarea::CursorMove;

impl Editor {
    // ==================== Mark/Selection Operations ====================

    /// Set or toggle the mark (C-SPC)
    pub fn set_mark(&mut self) {
        let cursor_pos = self.textarea.cursor();

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
        if let MarkState::Active { row, col } = self.mark {
            self.mark = MarkState::Set { row, col };
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
        if let Some(text) = self.get_selected_text() {
            self.save_undo_state();
            self.clipboard.copy(&text);
            self.textarea.cut();
            self.mark = MarkState::None;
            self.mark_modified();
            true
        } else {
            false
        }
    }

    /// Copy selected text to system clipboard (M-w)
    ///
    /// Does not modify buffer, so no undo state or modification tracking needed.
    pub fn copy_region(&mut self) {
        if let Some(text) = self.get_selected_text() {
            self.clipboard.copy(&text);
            self.cancel_mark();
        }
    }

    /// Paste from system clipboard (C-y)
    ///
    /// Returns true if text was actually pasted, false if clipboard empty.
    /// Handles undo state and modification tracking internally.
    pub fn paste(&mut self) -> bool {
        if let Some(text) = self.clipboard.paste() {
            self.save_undo_state();
            self.textarea.insert_str(&text);
            self.mark_modified();
            true
        } else {
            false
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
}
