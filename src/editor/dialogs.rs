//! Confirmation dialog system for the JAE editor.
//!
//! This module provides a trait-based confirmation dialog system that allows
//! for multi-step user confirmations with various response types.

use std::fs;
use std::path::PathBuf;
use tui_textarea::TextArea;

use super::types::{ConfirmationStep, ResponseResult, ResponseType};
use super::Editor;

/// Trait for actions requiring user confirmation.
/// Implement this to create new confirmable dialogs.
///
/// The `handle_response` method can execute actions at any step,
/// and returns what to do next. This allows for complex flows
/// where intermediate steps trigger operations.
pub trait ConfirmationDialog {
    /// Returns all confirmation steps in order
    fn steps(&self) -> Vec<ConfirmationStep>;

    /// Handle user response at given step.
    /// Can execute actions and modify editor state.
    /// Returns what the dialog should do next.
    fn handle_response(
        &mut self,
        step_index: usize,
        response: &str,
        editor: &mut Editor,
    ) -> ResponseResult;

    /// Called when dialog completes successfully (last step + Continue, or Finish)
    fn on_complete(&self, _editor: &mut Editor) -> Result<(), String> {
        Ok(())
    }

    /// Called when user cancels - optional cleanup
    fn on_cancel(&self, _editor: &mut Editor) {}
}

/// Quit confirmation dialog - shown when quitting with unsaved changes
pub struct QuitConfirmation;

impl ConfirmationDialog for QuitConfirmation {
    fn steps(&self) -> Vec<ConfirmationStep> {
        vec![
            ConfirmationStep {
                prompt: "Buffer has unsaved changes. Quit anyway?".to_string(),
                response_type: ResponseType::Binary,
            },
            ConfirmationStep {
                prompt: "Save before quitting?".to_string(),
                response_type: ResponseType::Choice(vec![
                    ('y', "save & quit".to_string()),
                    ('n', "quit without saving".to_string()),
                    ('c', "cancel".to_string()),
                ]),
            },
        ]
    }

    fn handle_response(
        &mut self,
        step_index: usize,
        response: &str,
        editor: &mut Editor,
    ) -> ResponseResult {
        match step_index {
            0 => {
                // "Quit anyway?" step
                match response {
                    "y" => ResponseResult::Continue, // Go to save prompt
                    "n" => ResponseResult::Cancel,   // Don't quit
                    _ => ResponseResult::Stay,
                }
            }
            1 => {
                // "Save before quitting?" step
                match response {
                    "y" => {
                        // Save and quit
                        if let Err(_e) = editor.save_file() {
                            // Save failed (likely no filename), don't quit yet
                            ResponseResult::Cancel
                        } else {
                            // Save succeeded, mark for quit
                            editor.pending_quit = true;
                            ResponseResult::Finish
                        }
                    }
                    "n" => {
                        // Quit without saving
                        editor.pending_quit = true;
                        ResponseResult::Finish
                    }
                    "c" => ResponseResult::Cancel,
                    _ => ResponseResult::Stay,
                }
            }
            _ => ResponseResult::Stay,
        }
    }

    fn on_complete(&self, editor: &mut Editor) -> Result<(), String> {
        editor.pending_quit = true;
        Ok(())
    }
}

/// Delete file confirmation dialog
pub struct DeleteFileConfirmation {
    pub path: PathBuf,
}

impl ConfirmationDialog for DeleteFileConfirmation {
    fn steps(&self) -> Vec<ConfirmationStep> {
        vec![
            ConfirmationStep {
                prompt: format!("Delete file '{}'?", self.path.display()),
                response_type: ResponseType::Binary,
            },
            ConfirmationStep {
                prompt: format!("Permanently delete '{}'?", self.path.display()),
                response_type: ResponseType::Binary,
            },
        ]
    }

    fn handle_response(
        &mut self,
        _step_index: usize,
        response: &str,
        _editor: &mut Editor,
    ) -> ResponseResult {
        match response {
            "y" => ResponseResult::Continue,
            "n" => ResponseResult::Cancel,
            _ => ResponseResult::Stay,
        }
    }

    fn on_complete(&self, editor: &mut Editor) -> Result<(), String> {
        fs::remove_file(&self.path)
            .map_err(|e| format!("Failed to delete file: {}", e))?;

        // Clear buffer if this was the current file
        if editor.current_file.as_ref() == Some(&self.path) {
            editor.current_file = None;
            editor.modified = false;
            editor.textarea = TextArea::default();
            editor.update_textarea_colors();
            editor.textarea.set_cursor_line_style(ratatui::style::Style::default());
        }

        Ok(())
    }
}
