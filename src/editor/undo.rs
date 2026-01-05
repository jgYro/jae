//! Custom undo/redo system for JAE editor.
//!
//! This module provides a simple linear undo/redo system that stores
//! snapshots of the editor content and cursor position.

use ratatui::style::{Color, Modifier, Style};
use tui_textarea::{CursorMove, TextArea};

/// A snapshot of the editor state at a point in time
#[derive(Debug, Clone)]
pub struct EditorSnapshot {
    /// The text content as lines
    pub lines: Vec<String>,
    /// Cursor position (row, col)
    pub cursor: (usize, usize),
}

/// Manages undo/redo history with a linear stack
pub struct UndoManager {
    /// Stack of past states (for undo)
    undo_stack: Vec<EditorSnapshot>,
    /// Stack of future states (for redo)
    redo_stack: Vec<EditorSnapshot>,
    /// Maximum number of undo states to keep
    max_history: usize,
}

impl UndoManager {
    /// Create a new undo manager with default history size
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_history: 100,
        }
    }

    /// Save the current state before an edit operation
    pub fn save_state(&mut self, snapshot: EditorSnapshot) {
        // Clear redo stack when new edit is made
        self.redo_stack.clear();

        // Add to undo stack
        self.undo_stack.push(snapshot);

        // Trim if exceeding max history
        if self.undo_stack.len() > self.max_history {
            self.undo_stack.remove(0);
        }
    }

    /// Undo: restore previous state, returns the state to restore to
    pub fn undo(&mut self, current: EditorSnapshot) -> Option<EditorSnapshot> {
        if let Some(previous) = self.undo_stack.pop() {
            // Save current state to redo stack
            self.redo_stack.push(current);
            Some(previous)
        } else {
            None
        }
    }

    /// Redo: restore next state, returns the state to restore to
    pub fn redo(&mut self, current: EditorSnapshot) -> Option<EditorSnapshot> {
        if let Some(next) = self.redo_stack.pop() {
            // Save current state to undo stack
            self.undo_stack.push(current);
            Some(next)
        } else {
            None
        }
    }

    /// Check if undo is available
    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available
    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear all history (e.g., when opening a new file)
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for UndoManager {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Editor Undo/Redo Methods ====================

use super::Editor;

impl Editor {
    /// Create a snapshot of the current editor state
    fn create_snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            lines: self.textarea.lines().iter().map(|s| s.to_string()).collect(),
            cursor: self.textarea.cursor(),
        }
    }

    /// Restore editor state from a snapshot
    fn restore_snapshot(&mut self, snapshot: EditorSnapshot) {
        // Recreate textarea with the snapshot's content
        let mut new_textarea = if snapshot.lines.is_empty() {
            TextArea::default()
        } else {
            TextArea::new(snapshot.lines)
        };

        // Copy over style settings
        new_textarea.set_cursor_style(
            Style::default()
                .bg(self.settings.cursor_color)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        new_textarea.set_cursor_line_style(Style::default());
        new_textarea.set_selection_style(
            Style::default()
                .bg(self.settings.selection_color)
                .fg(Color::White),
        );

        self.textarea = new_textarea;

        // Restore cursor position
        let (target_row, target_col) = snapshot.cursor;
        // Move to beginning first
        self.textarea.move_cursor(CursorMove::Top);
        self.textarea.move_cursor(CursorMove::Head);
        // Move to target row
        for _ in 0..target_row {
            self.textarea.move_cursor(CursorMove::Down);
        }
        // Move to target column
        for _ in 0..target_col {
            self.textarea.move_cursor(CursorMove::Forward);
        }
    }

    /// Save current state to undo history (call before making changes)
    pub fn save_undo_state(&mut self) {
        let snapshot = self.create_snapshot();
        self.undo_manager.save_state(snapshot);
    }

    /// Undo the last edit
    pub fn undo(&mut self) -> bool {
        let current = self.create_snapshot();
        if let Some(previous) = self.undo_manager.undo(current) {
            self.restore_snapshot(previous);
            true
        } else {
            false
        }
    }

    /// Redo the last undo
    pub fn redo(&mut self) -> bool {
        let current = self.create_snapshot();
        if let Some(next) = self.undo_manager.redo(current) {
            self.restore_snapshot(next);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(text: &str, cursor: (usize, usize)) -> EditorSnapshot {
        EditorSnapshot {
            lines: text.lines().map(String::from).collect(),
            cursor,
        }
    }

    #[test]
    fn test_undo_redo_basic() {
        let mut manager = UndoManager::new();

        // Save initial state
        let state1 = make_snapshot("hello", (0, 5));
        manager.save_state(state1.clone());

        // Save after edit
        let state2 = make_snapshot("hello world", (0, 11));
        manager.save_state(state2.clone());

        // Current state
        let current = make_snapshot("hello world!", (0, 12));

        // Undo should restore state2
        let restored = manager.undo(current.clone()).unwrap();
        assert_eq!(restored.lines, vec!["hello world"]);

        // Undo again should restore state1
        let current2 = make_snapshot("hello world", (0, 11));
        let restored2 = manager.undo(current2).unwrap();
        assert_eq!(restored2.lines, vec!["hello"]);

        // Redo should restore state2
        let current3 = make_snapshot("hello", (0, 5));
        let restored3 = manager.redo(current3).unwrap();
        assert_eq!(restored3.lines, vec!["hello world"]);
    }

    #[test]
    fn test_redo_cleared_on_new_edit() {
        let mut manager = UndoManager::new();

        manager.save_state(make_snapshot("a", (0, 1)));
        manager.save_state(make_snapshot("ab", (0, 2)));

        let current = make_snapshot("abc", (0, 3));
        manager.undo(current);

        assert!(manager.can_redo());

        // New edit should clear redo
        manager.save_state(make_snapshot("abx", (0, 3)));
        assert!(!manager.can_redo());
    }
}
