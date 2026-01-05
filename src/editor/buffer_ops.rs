//! Buffer operation traits and helpers for JAE editor.
//!
//! This module provides the `BufferEdit` trait which ensures consistent handling
//! of buffer modifications. All operations that modify the buffer should use this
//! pattern to prevent bugs where the buffer is marked as modified when nothing changed.
//!
//! # The Problem This Solves
//!
//! Without this pattern, it's easy to write buggy code like:
//! ```ignore
//! editor.save_undo_state();  // Always called
//! editor.cut_region();       // Might do nothing if no selection!
//! editor.mark_modified();    // Always called - BUG!
//! ```
//!
//! # The Solution
//!
//! Operations that implement `BufferEdit` are self-contained:
//! 1. Check if the operation will actually modify anything
//! 2. If not, return `false` immediately (no side effects)
//! 3. If yes, save undo state BEFORE modifying
//! 4. Perform the modification
//! 5. Mark buffer as modified
//! 6. Return `true`
//!
//! # Example Implementation
//!
//! ```ignore
//! impl Editor {
//!     pub fn my_operation(&mut self) -> bool {
//!         // 1. Check preconditions
//!         if !self.can_do_operation() {
//!             return false;
//!         }
//!
//!         // 2. Save undo state BEFORE any modification
//!         self.save_undo_state();
//!
//!         // 3. Perform the modification
//!         self.do_the_thing();
//!
//!         // 4. Mark as modified
//!         self.mark_modified();
//!
//!         // 5. Return success
//!         true
//!     }
//! }
//! ```
//!
//! # Usage in Keybindings
//!
//! With this pattern, keybindings become simple:
//! ```ignore
//! (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
//!     editor.cut_region();  // Handles everything internally
//! }
//! ```

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

/// Helper to determine if a key event represents actual text input
/// (as opposed to a command/control sequence).
///
/// Text input keys are:
/// - Character keys with no modifiers (regular typing)
/// - Character keys with only Shift (uppercase letters, symbols)
/// - Backspace, Delete, Enter, Tab (with no modifiers)
///
/// NOT text input:
/// - Control+letter (commands like C-w, C-k)
/// - Alt+letter (commands like M-f, M-b)
/// - Function keys, arrow keys, etc.
///
/// # Example
///
/// ```ignore
/// if is_text_input_key(key.code, key.modifiers) {
///     editor.save_undo_state();
///     editor.textarea.input(input);
///     editor.mark_modified();
/// }
/// // else: it's a command key, handle separately or ignore
/// ```
pub fn is_text_input_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    match code {
        KeyCode::Char(_) => {
            // Only NONE or SHIFT modifiers mean actual character input
            // Control+char and Alt+char are commands, not text input
            modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT
        }
        // These special keys always modify the buffer (when not modified)
        KeyCode::Backspace | KeyCode::Delete | KeyCode::Enter | KeyCode::Tab => {
            // Only count as text input with no modifiers
            // M-Backspace, C-Backspace etc. are commands handled separately
            modifiers == KeyModifiers::NONE
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regular_typing_is_text_input() {
        // Regular letters
        assert!(is_text_input_key(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(is_text_input_key(KeyCode::Char('z'), KeyModifiers::NONE));

        // Shift+letter (uppercase)
        assert!(is_text_input_key(KeyCode::Char('A'), KeyModifiers::SHIFT));

        // Numbers and symbols
        assert!(is_text_input_key(KeyCode::Char('1'), KeyModifiers::NONE));
        assert!(is_text_input_key(KeyCode::Char('!'), KeyModifiers::SHIFT));
    }

    #[test]
    fn test_control_keys_not_text_input() {
        // Control+letter is a command, not text input
        assert!(!is_text_input_key(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert!(!is_text_input_key(KeyCode::Char('k'), KeyModifiers::CONTROL));
        assert!(!is_text_input_key(KeyCode::Char('t'), KeyModifiers::CONTROL));
    }

    #[test]
    fn test_alt_keys_not_text_input() {
        // Alt+letter is a command, not text input
        assert!(!is_text_input_key(KeyCode::Char('f'), KeyModifiers::ALT));
        assert!(!is_text_input_key(KeyCode::Char('b'), KeyModifiers::ALT));
    }

    #[test]
    fn test_special_keys() {
        // Plain special keys are text input
        assert!(is_text_input_key(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(is_text_input_key(KeyCode::Delete, KeyModifiers::NONE));
        assert!(is_text_input_key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(is_text_input_key(KeyCode::Tab, KeyModifiers::NONE));

        // Modified special keys are commands
        assert!(!is_text_input_key(KeyCode::Backspace, KeyModifiers::ALT));
        assert!(!is_text_input_key(KeyCode::Backspace, KeyModifiers::CONTROL));
    }

    #[test]
    fn test_arrow_keys_not_text_input() {
        assert!(!is_text_input_key(KeyCode::Up, KeyModifiers::NONE));
        assert!(!is_text_input_key(KeyCode::Down, KeyModifiers::NONE));
        assert!(!is_text_input_key(KeyCode::Left, KeyModifiers::NONE));
        assert!(!is_text_input_key(KeyCode::Right, KeyModifiers::NONE));
    }
}
