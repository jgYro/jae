use crate::commands::{CommandRegistry, KeyPrefix};
use crate::kill_ring::KillRing;
use tui_textarea::{CursorMove, TextArea};
use std::cmp::min;
use std::path::PathBuf;
use std::fs;
use std::io::{self, Read, Write};

#[derive(Clone)]
pub enum MenuAction {
    // Text transformation actions
    Uppercase,
    Lowercase,
    Capitalize,
    Reverse,
    Base64Encode,
    Base64Decode,
    UrlEncode,
    UrlDecode,
    // Insert actions
    InsertDate,
    InsertTime,
    InsertDateTime,
    InsertLorem,
    InsertBullets,
    InsertNumbers,
    InsertTodo,
    // Text analysis
    CountWords,
    CountChars,
    CountLines,
}

#[derive(Clone)]
pub enum MenuItem {
    Action(MenuAction, String), // (action, label)
    Category(String, Vec<MenuItem>), // (category name, items)
}

impl MenuItem {
    pub fn label(&self) -> &str {
        match self {
            MenuItem::Action(_, label) => label,
            MenuItem::Category(name, _) => name,
        }
    }

    pub fn is_category(&self) -> bool {
        matches!(self, MenuItem::Category(_, _))
    }
}

pub struct MenuState {
    pub items: Vec<MenuItem>,
    pub selected: usize,
    pub path: Vec<String>, // Breadcrumb trail
}

impl MenuState {
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self {
            items,
            selected: 0,
            path: Vec::new(),
        }
    }

    pub fn enter_category(&mut self) -> bool {
        if let Some(MenuItem::Category(name, items)) = self.items.get(self.selected) {
            self.path.push(name.clone());
            self.items = items.clone();
            self.selected = 0;
            true
        } else {
            false
        }
    }

    pub fn go_back(&mut self, root_items: &[MenuItem]) -> bool {
        if !self.path.is_empty() {
            self.path.pop();

            // Navigate back through the tree
            let mut current = root_items.to_vec();
            for segment in &self.path {
                for item in &current {
                    if let MenuItem::Category(name, items) = item {
                        if name == segment {
                            current = items.clone();
                            break;
                        }
                    }
                }
            }

            self.items = current;
            self.selected = 0;
            true
        } else {
            false
        }
    }
}

/// Callback for minibuffer actions
#[derive(Clone)]
pub enum MinibufferCallback {
    OpenFile,
    SaveFileAs,
    DeleteFile,
}

/// Defines what kind of response a confirmation step expects
#[derive(Clone)]
pub enum ResponseType {
    /// Binary yes/no - user presses 'y' or 'n'
    Binary,
    /// Multiple choice - user presses key associated with option: Vec<(key, label)>
    Choice(Vec<(char, String)>),
    /// Text input - user types response and presses Enter
    TextInput { placeholder: String },
}

/// A single step in a confirmation dialog
#[derive(Clone)]
pub struct ConfirmationStep {
    pub prompt: String,
    pub response_type: ResponseType,
}

/// Result of handling a user response - controls dialog flow
pub enum ResponseResult {
    /// Advance to next step (or finish if last step)
    Continue,
    /// Go back to previous step
    Back,
    /// Jump to a specific step by index
    GoTo(usize),
    /// Stay on current step (e.g., invalid input)
    Stay,
    /// Cancel the entire operation
    Cancel,
    /// Finish immediately (skip any remaining steps)
    Finish,
}

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
                            // The save_file function will open a minibuffer for filename
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
                    "c" => ResponseResult::Cancel, // Go back, don't quit
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

pub enum FloatingMode {
    TextEdit,
    Menu {
        state: MenuState,
        root_items: Vec<MenuItem>,
        preview: Option<String>,
        metadata: Option<String>,
    },
    Settings {
        items: Vec<SettingItem>,
        selected: usize,
    },
    /// Minibuffer for text input with completions (like Emacs minibuffer)
    Minibuffer {
        prompt: String,
        input: String,
        cursor_pos: usize,
        completions: Vec<String>,
        selected_completion: Option<usize>,
        callback: MinibufferCallback,
    },
    /// Confirmation dialog - wraps any ConfirmationDialog implementation
    Confirm {
        dialog: Box<dyn ConfirmationDialog>,
        steps: Vec<ConfirmationStep>,
        current_index: usize,
        text_input: String, // For TextInput response type
    },
    /// Command palette (M-x) for executing commands by name
    CommandPalette {
        input: String,
        cursor_pos: usize,
        filtered_commands: Vec<CommandInfo>,
        selected: usize,
    },
}

/// Command info for display in command palette
pub struct CommandInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub keybinding: Option<String>,
}

#[derive(Clone)]
pub struct SettingItem {
    pub name: String,
    pub value: SettingValue,
    pub description: String,
}

#[derive(Clone)]
pub enum SettingValue {
    Bool(bool),
    Number(u16),
    Choice { current: usize, options: Vec<String> },
}

pub struct FloatingWindow {
    pub textarea: TextArea<'static>,
    pub visible: bool,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub mode: FloatingMode,
}

pub struct Settings {
    pub show_metadata: bool,
    pub floating_window_width: u16,
    pub floating_window_height: u16,
    pub show_preview: bool,
    pub cursor_color: ratatui::style::Color,
    pub selection_color: ratatui::style::Color,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_metadata: true,
            floating_window_width: 60,
            floating_window_height: 20,
            show_preview: true,
            cursor_color: ratatui::style::Color::Red,
            selection_color: ratatui::style::Color::Magenta,
        }
    }
}

/// Item shown in which-key display
pub struct WhichKeyItem {
    pub key_display: String,
    pub command_name: &'static str,
}

/// Status bar state for which-key display and prefix tracking
pub struct StatusBarState {
    pub expanded: bool,
    pub active_prefix: Option<Box<dyn KeyPrefix>>,
    pub which_key_items: Vec<WhichKeyItem>,
    pub command_registry: CommandRegistry,
}

impl StatusBarState {
    pub fn new() -> Self {
        Self {
            expanded: false,
            active_prefix: None,
            which_key_items: Vec::new(),
            command_registry: CommandRegistry::new(),
        }
    }

    /// Activate a prefix and populate which-key items
    pub fn activate_prefix(&mut self, prefix: Box<dyn KeyPrefix>) {
        let bindings = prefix.bindings();
        self.which_key_items = bindings
            .into_iter()
            .map(|b| WhichKeyItem {
                key_display: b.key.display(),
                command_name: b.command,
            })
            .collect();
        self.active_prefix = Some(prefix);
        self.expanded = true;
    }

    /// Clear the active prefix and collapse the status bar
    pub fn clear_prefix(&mut self) {
        self.active_prefix = None;
        self.which_key_items.clear();
        self.expanded = false;
    }

    /// Get the display name of the active prefix
    pub fn prefix_display_name(&self) -> Option<&'static str> {
        self.active_prefix.as_ref().map(|p| p.display_name())
    }
}

pub struct Editor {
    pub textarea: TextArea<'static>,
    pub mark_active: bool,
    pub mark_set: bool,  // Mark exists but may not be active (for C-SPC C-SPC)
    pub mark_position: Option<(usize, usize)>,  // Store mark position separately
    pub kill_ring: KillRing,
    pub last_was_kill: bool,
    pub floating_window: Option<FloatingWindow>,
    pub focus_floating: bool,
    pub settings: Settings,
    pub status_bar: StatusBarState,
    pub last_key: Option<(ratatui::crossterm::event::KeyCode, ratatui::crossterm::event::KeyModifiers)>,
    // File state
    pub current_file: Option<PathBuf>,
    pub modified: bool,
    // Quit state - set by QuitConfirmation dialog
    pub pending_quit: bool,
}

impl Editor {
    fn char_index_to_byte_index(s: &str, char_idx: usize) -> usize {
        s.char_indices()
            .nth(char_idx)
            .map(|(byte_idx, _)| byte_idx)
            .unwrap_or(s.len())
    }

    fn safe_string_slice(s: &str, start_char: usize, end_char: usize) -> String {
        let start_byte = Self::char_index_to_byte_index(s, start_char);
        let end_byte = if end_char >= s.chars().count() {
            s.len()
        } else {
            Self::char_index_to_byte_index(s, end_char)
        };

        if start_byte <= end_byte && end_byte <= s.len() {
            s[start_byte..end_byte].to_string()
        } else {
            String::new()
        }
    }

    fn get_color_index(&self, color: ratatui::style::Color) -> usize {
        match color {
            ratatui::style::Color::Red => 0,
            ratatui::style::Color::Green => 1,
            ratatui::style::Color::Yellow => 2,
            ratatui::style::Color::Blue => 3,
            ratatui::style::Color::Magenta => 4,
            ratatui::style::Color::Cyan => 5,
            ratatui::style::Color::White => 6,
            ratatui::style::Color::LightBlue => 6, // Map LightBlue to last index for selection
            _ => 0,
        }
    }

    pub fn index_to_color(&self, index: usize, for_selection: bool) -> ratatui::style::Color {
        if for_selection {
            match index {
                0 => ratatui::style::Color::Red,
                1 => ratatui::style::Color::Green,
                2 => ratatui::style::Color::Yellow,
                3 => ratatui::style::Color::Blue,
                4 => ratatui::style::Color::Magenta,
                5 => ratatui::style::Color::Cyan,
                6 => ratatui::style::Color::LightBlue,
                _ => ratatui::style::Color::Magenta,
            }
        } else {
            match index {
                0 => ratatui::style::Color::Red,
                1 => ratatui::style::Color::Green,
                2 => ratatui::style::Color::Yellow,
                3 => ratatui::style::Color::Blue,
                4 => ratatui::style::Color::Magenta,
                5 => ratatui::style::Color::Cyan,
                6 => ratatui::style::Color::White,
                _ => ratatui::style::Color::Red,
            }
        }
    }

    pub fn update_textarea_colors(&mut self) {
        self.textarea.set_cursor_style(
            ratatui::style::Style::default()
                .bg(self.settings.cursor_color)
                .fg(ratatui::style::Color::White)
                .add_modifier(ratatui::style::Modifier::BOLD)
        );
        self.textarea.set_selection_style(
            ratatui::style::Style::default()
                .bg(self.settings.selection_color)
                .fg(ratatui::style::Color::White)
        );
    }

    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        let settings = Settings::default();

        // Set cursor to be a visible block with red background
        textarea.set_cursor_style(
            ratatui::style::Style::default()
                .bg(settings.cursor_color)
                .fg(ratatui::style::Color::White)
                .add_modifier(ratatui::style::Modifier::BOLD)
        );
        // Remove underline from the current line
        textarea.set_cursor_line_style(ratatui::style::Style::default());
        // Set selection style to be magenta/purple background (distinct from cursor)
        textarea.set_selection_style(
            ratatui::style::Style::default()
                .bg(settings.selection_color)
                .fg(ratatui::style::Color::White)
        );

        Self {
            textarea,
            mark_active: false,
            mark_set: false,
            mark_position: None,
            kill_ring: KillRing::new(),
            last_was_kill: false,
            floating_window: None,
            focus_floating: false,
            settings,
            status_bar: StatusBarState::new(),
            last_key: None,
            current_file: None,
            modified: false,
            pending_quit: false,
        }
    }

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
        if self.last_key == Some((ratatui::crossterm::event::KeyCode::Char(' '), ratatui::crossterm::event::KeyModifiers::CONTROL)) {
            // Second C-SPC: Set mark but deactivate region
            if !self.mark_set {
                self.mark_position = Some(cursor_pos);
                self.mark_set = true;
            }

            if self.mark_active {
                self.textarea.cancel_selection();
                self.mark_active = false;
            }
        } else {
            // First C-SPC: Set mark and activate region
            if !self.mark_active {
                // Setting new mark
                self.mark_position = Some(cursor_pos);
                self.mark_set = true;
                self.textarea.start_selection();
                self.mark_active = true;
            } else {
                // If already active, deactivate (toggle behavior)
                self.textarea.cancel_selection();
                self.mark_active = false;
                // Keep mark position for later use
            }
        }
        // Don't reset last_was_kill when setting mark
    }

    pub fn cancel_mark(&mut self) {
        self.textarea.cancel_selection();
        self.mark_active = false;
        // Note: We keep mark_set and mark_position for navigation
    }

    pub fn swap_cursor_mark(&mut self) {
        // C-x C-x exchanges point and mark

        if !self.mark_set || self.mark_position.is_none() {
            // No mark to swap with - just set mark here
            self.set_mark();
            return;
        }

        let current_cursor = self.textarea.cursor();
        let saved_mark = self.mark_position.unwrap();

        // Validate both positions are still within document bounds
        let lines = self.textarea.lines();
        if saved_mark.0 >= lines.len() || current_cursor.0 >= lines.len() {
            // Mark or cursor is out of bounds, reset mark
            self.mark_position = None;
            self.mark_set = false;
            self.mark_active = false;
            self.textarea.cancel_selection();
            return;
        }

        // If they're the same, just activate region if needed
        if current_cursor == saved_mark {
            if !self.mark_active {
                self.textarea.start_selection();
                self.mark_active = true;
            }
            return;
        }

        // Save the current cursor position as the new mark
        self.mark_position = Some(current_cursor);

        if self.mark_active {
            // Selection is active - we need to preserve it while swapping ends
            // The selection should remain between the same two points,
            // but the cursor should move to the other end

            // Cancel the current selection
            self.textarea.cancel_selection();

            // Move cursor to where we want the new anchor (current cursor position)
            // Use saturating cast to prevent overflow for large files
            let jump_row = min(current_cursor.0, u16::MAX as usize) as u16;
            let jump_col = min(current_cursor.1, u16::MAX as usize) as u16;
            self.textarea.move_cursor(CursorMove::Jump(jump_row, jump_col));

            // Start selection from current cursor position
            self.textarea.start_selection();

            // Move cursor to the saved mark (where cursor will end up)
            let mark_row = min(saved_mark.0, u16::MAX as usize) as u16;
            let mark_col = min(saved_mark.1, u16::MAX as usize) as u16;
            self.textarea.move_cursor(CursorMove::Jump(mark_row, mark_col));

            self.mark_active = true;
        } else {
            // No active selection, create one between cursor and mark
            // Move to current cursor position first (will be the new anchor)
            // Use saturating cast to prevent overflow for large files
            let jump_row = min(current_cursor.0, u16::MAX as usize) as u16;
            let jump_col = min(current_cursor.1, u16::MAX as usize) as u16;
            self.textarea.move_cursor(CursorMove::Jump(jump_row, jump_col));

            // Start selection from here
            self.textarea.start_selection();
            self.mark_active = true;

            // Move to saved mark (where cursor ends up)
            let mark_row = min(saved_mark.0, u16::MAX as usize) as u16;
            let mark_col = min(saved_mark.1, u16::MAX as usize) as u16;
            self.textarea.move_cursor(CursorMove::Jump(mark_row, mark_col));
        }
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
                end.1.min(line.chars().count())
            } else {
                line.chars().count()
            };

            // Convert character indices to byte indices for proper UTF-8 slicing
            let mut char_pos = 0;
            let mut start_byte = 0;
            let mut end_byte = line.len();

            for (byte_idx, _) in line.char_indices() {
                if char_pos == start_col {
                    start_byte = byte_idx;
                }
                if char_pos == end_col {
                    end_byte = byte_idx;
                    break;
                }
                char_pos += 1;
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
        // Don't reset last_was_kill on movement
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
            // Don't reset last_was_kill on movement
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

            if col < line.chars().count() {
                // Kill from cursor to end of line - use safe slicing
                (Self::safe_string_slice(line, col, line.chars().count()), true)
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
            Self::safe_string_slice(line, 0, col)
        };

        if !killed_text.is_empty() {
            // Add to kill ring (C-u doesn't append to previous kills)
            self.kill_ring.push(killed_text);

            // Move to beginning of line
            self.textarea.move_cursor(CursorMove::Head);

            // Start selection and move forward to select the text
            // Use `col` directly - it's already the character count from cursor position
            self.textarea.start_selection();
            for _ in 0..col {
                self.textarea.move_cursor(CursorMove::Forward);
            }

            // Cut the selected text
            self.textarea.cut();
            self.mark_active = false;
            self.last_was_kill = false; // C-u doesn't continue kill sequence
        }
    }

    pub fn open_settings_menu(&mut self) {
        // Close any existing floating window
        self.floating_window = None;
        self.focus_floating = false;

        // Create settings items
        let settings_items = vec![
            SettingItem {
                name: "Show Metadata".to_string(),
                value: SettingValue::Bool(self.settings.show_metadata),
                description: "Display metadata for analysis actions".to_string(),
            },
            SettingItem {
                name: "Show Preview".to_string(),
                value: SettingValue::Bool(self.settings.show_preview),
                description: "Show preview of transformations".to_string(),
            },
            SettingItem {
                name: "Window Width".to_string(),
                value: SettingValue::Number(self.settings.floating_window_width),
                description: "Width of floating windows".to_string(),
            },
            SettingItem {
                name: "Window Height".to_string(),
                value: SettingValue::Number(self.settings.floating_window_height),
                description: "Height of floating windows".to_string(),
            },
            SettingItem {
                name: "Cursor Color".to_string(),
                value: SettingValue::Choice {
                    current: self.get_color_index(self.settings.cursor_color),
                    options: vec![
                        "Red".to_string(),
                        "Green".to_string(),
                        "Yellow".to_string(),
                        "Blue".to_string(),
                        "Magenta".to_string(),
                        "Cyan".to_string(),
                        "White".to_string(),
                    ],
                },
                description: "Color of the cursor".to_string(),
            },
            SettingItem {
                name: "Selection Color".to_string(),
                value: SettingValue::Choice {
                    current: self.get_color_index(self.settings.selection_color),
                    options: vec![
                        "Red".to_string(),
                        "Green".to_string(),
                        "Yellow".to_string(),
                        "Blue".to_string(),
                        "Magenta".to_string(),
                        "Cyan".to_string(),
                        "LightBlue".to_string(),
                    ],
                },
                description: "Color of selected text".to_string(),
            },
        ];

        let mode = FloatingMode::Settings {
            items: settings_items,
            selected: 0,
        };

        // Position settings window
        let x = 10;
        let y = 5;

        let mut floating_textarea = TextArea::default();
        floating_textarea.set_cursor_style(
            ratatui::style::Style::default()
                .add_modifier(ratatui::style::Modifier::REVERSED)
        );

        self.floating_window = Some(FloatingWindow {
            textarea: floating_textarea,
            visible: true,
            x,
            y,
            width: 50,
            height: 15,
            mode,
        });
        self.focus_floating = true;
    }

    pub fn update_menu_preview(&mut self) {
        // First, get the necessary data without borrowing self mutably
        let (action_opt, selected_text_opt) = if let Some(ref fw) = self.floating_window {
            if let FloatingMode::Menu { state, .. } = &fw.mode {
                if let Some(MenuItem::Action(action, _)) = state.items.get(state.selected) {
                    (Some(action.clone()), self.get_selected_text())
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Now update preview and metadata with the collected data
        if let (Some(action), Some(selected_text)) = (action_opt, selected_text_opt) {
            let preview_text = self.generate_preview(&action, &selected_text);
            let metadata_text = if self.settings.show_metadata {
                self.generate_metadata(&action, &selected_text)
            } else {
                None
            };

            // Finally, update the floating window
            if let Some(ref mut fw) = self.floating_window {
                if let FloatingMode::Menu { preview, metadata, .. } = &mut fw.mode {
                    *preview = preview_text;
                    *metadata = metadata_text;
                }
            }
        } else {
            // Clear preview and metadata if no action selected
            if let Some(ref mut fw) = self.floating_window {
                if let FloatingMode::Menu { preview, metadata, .. } = &mut fw.mode {
                    *preview = None;
                    *metadata = None;
                }
            }
        }
    }

    fn generate_preview(&self, action: &MenuAction, text: &str) -> Option<String> {
        if !self.settings.show_preview {
            return None;
        }

        // Limit preview length (use chars for UTF-8 safety)
        let preview_text = if text.chars().count() > 50 {
            format!("{}...", text.chars().take(50).collect::<String>())
        } else {
            text.to_string()
        };

        match action {
            MenuAction::Uppercase => Some(preview_text.to_uppercase()),
            MenuAction::Lowercase => Some(preview_text.to_lowercase()),
            MenuAction::Capitalize => {
                Some(preview_text
                    .split_whitespace()
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => first.to_uppercase().chain(chars.as_str().to_lowercase().chars()).collect(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" "))
            }
            MenuAction::Reverse => Some(preview_text.chars().rev().collect()),
            MenuAction::Base64Encode => {
                use base64::{Engine as _, engine::general_purpose};
                Some(general_purpose::STANDARD.encode(preview_text.as_bytes()))
            }
            _ => None,
        }
    }

    fn generate_metadata(&self, action: &MenuAction, text: &str) -> Option<String> {
        match action {
            MenuAction::CountWords => {
                let count = text.split_whitespace().count();
                Some(format!("Words: {}", count))
            }
            MenuAction::CountChars => {
                let count = text.chars().count();
                let bytes = text.len();
                Some(format!("Chars: {} | Bytes: {}", count, bytes))
            }
            MenuAction::CountLines => {
                let count = text.lines().count();
                Some(format!("Lines: {}", count))
            }
            _ => None,
        }
    }

    pub fn toggle_floating_window(&mut self) {
        if self.floating_window.is_some() {
            // Close floating window
            self.floating_window = None;
            self.focus_floating = false;
        } else {
            // Preserve selection state before creating floating window
            let selection_range = self.textarea.selection_range();
            let was_mark_active = self.mark_active;

            // Create floating window near cursor
            let (cursor_row, cursor_col) = self.textarea.cursor();

            // Calculate position (offset from cursor)
            let x = (cursor_col as u16).saturating_add(5).min(80);
            let y = (cursor_row as u16).saturating_add(2).min(20);

            let mut floating_textarea = TextArea::default();

            // Apply similar styling as main textarea
            floating_textarea.set_cursor_style(
                ratatui::style::Style::default()
                    .add_modifier(ratatui::style::Modifier::REVERSED)
            );
            floating_textarea.set_cursor_line_style(ratatui::style::Style::default());
            floating_textarea.set_selection_style(
                ratatui::style::Style::default()
                    .add_modifier(ratatui::style::Modifier::REVERSED)
            );

            // Determine the menu based on whether text is selected
            let root_items = if was_mark_active && selection_range.is_some() {
                // Text transformation menu for selected text
                vec![
                    MenuItem::Category("Transform Case".to_string(), vec![
                        MenuItem::Action(MenuAction::Uppercase, "UPPERCASE".to_string()),
                        MenuItem::Action(MenuAction::Lowercase, "lowercase".to_string()),
                        MenuItem::Action(MenuAction::Capitalize, "Capitalize Words".to_string()),
                    ]),
                    MenuItem::Category("Encoding/Decoding".to_string(), vec![
                        MenuItem::Action(MenuAction::Base64Encode, "Base64 Encode".to_string()),
                        MenuItem::Action(MenuAction::Base64Decode, "Base64 Decode".to_string()),
                        MenuItem::Action(MenuAction::UrlEncode, "URL Encode".to_string()),
                        MenuItem::Action(MenuAction::UrlDecode, "URL Decode".to_string()),
                    ]),
                    MenuItem::Action(MenuAction::Reverse, "Reverse Text".to_string()),
                    MenuItem::Category("Text Analysis".to_string(), vec![
                        MenuItem::Action(MenuAction::CountWords, "Count Words".to_string()),
                        MenuItem::Action(MenuAction::CountChars, "Count Characters".to_string()),
                        MenuItem::Action(MenuAction::CountLines, "Count Lines".to_string()),
                    ]),
                ]
            } else {
                // Insert menu when no text is selected
                vec![
                    MenuItem::Category("Insert Date/Time".to_string(), vec![
                        MenuItem::Action(MenuAction::InsertDate, "Insert Date".to_string()),
                        MenuItem::Action(MenuAction::InsertTime, "Insert Time".to_string()),
                        MenuItem::Action(MenuAction::InsertDateTime, "Insert Date & Time".to_string()),
                    ]),
                    MenuItem::Category("Insert Templates".to_string(), vec![
                        MenuItem::Action(MenuAction::InsertLorem, "Lorem Ipsum".to_string()),
                        MenuItem::Action(MenuAction::InsertBullets, "Bullet List".to_string()),
                        MenuItem::Action(MenuAction::InsertNumbers, "Numbered List".to_string()),
                        MenuItem::Action(MenuAction::InsertTodo, "TODO List".to_string()),
                    ]),
                ]
            };

            let mode = FloatingMode::Menu {
                state: MenuState::new(root_items.clone()),
                root_items,
                preview: None,
                metadata: None,
            };

            self.floating_window = Some(FloatingWindow {
                textarea: floating_textarea,
                visible: true,
                x,
                y,
                width: self.settings.floating_window_width,
                height: self.settings.floating_window_height,
                mode,
            });
            self.focus_floating = true;

            // Generate initial preview if applicable
            self.update_menu_preview();

            // Maintain the mark_active state
            // The selection in main textarea should remain intact
        }
    }

    pub fn get_active_textarea(&mut self) -> &mut TextArea<'static> {
        if self.focus_floating {
            if let Some(ref mut fw) = self.floating_window {
                &mut fw.textarea
            } else {
                &mut self.textarea
            }
        } else {
            &mut self.textarea
        }
    }

    pub fn apply_menu_option(&mut self, action: MenuAction) {
        match action {
            MenuAction::Uppercase => {
                if let Some(text) = self.get_selected_text() {
                    let transformed = text.to_uppercase();
                    self.replace_selection(transformed);
                }
            }
            MenuAction::Lowercase => {
                if let Some(text) = self.get_selected_text() {
                    let transformed = text.to_lowercase();
                    self.replace_selection(transformed);
                }
            }
            MenuAction::Capitalize => {
                if let Some(text) = self.get_selected_text() {
                    let transformed = text
                        .split_whitespace()
                        .map(|word| {
                            let mut chars = word.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(first) => first.to_uppercase().chain(chars.as_str().to_lowercase().chars()).collect(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    self.replace_selection(transformed);
                }
            }
            MenuAction::Reverse => {
                if let Some(text) = self.get_selected_text() {
                    let transformed = text.chars().rev().collect();
                    self.replace_selection(transformed);
                }
            }
            MenuAction::InsertDate => {
                use std::time::SystemTime;
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap();
                // Simple date format (you could use chrono crate for better formatting)
                let date = format!("2024-{:02}-{:02}",
                    (now.as_secs() / 86400 / 30) % 12 + 1,
                    (now.as_secs() / 86400) % 30 + 1
                );
                self.textarea.insert_str(&date);
            }
            MenuAction::InsertLorem => {
                let lorem = "Lorem ipsum dolor sit amet, consectetur adipiscing elit.";
                self.textarea.insert_str(lorem);
            }
            MenuAction::InsertBullets => {
                let bullets = "• Item 1\n• Item 2\n• Item 3";
                self.textarea.insert_str(bullets);
            }
            MenuAction::Base64Encode => {
                if let Some(text) = self.get_selected_text() {
                    use base64::{Engine as _, engine::general_purpose};
                    let encoded = general_purpose::STANDARD.encode(text.as_bytes());
                    self.replace_selection(encoded);
                }
            }
            MenuAction::Base64Decode => {
                if let Some(text) = self.get_selected_text() {
                    use base64::{Engine as _, engine::general_purpose};
                    if let Ok(decoded_bytes) = general_purpose::STANDARD.decode(text.as_bytes()) {
                        if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                            self.replace_selection(decoded_str);
                        }
                    }
                }
            }
            MenuAction::UrlEncode => {
                if let Some(text) = self.get_selected_text() {
                    let encoded = urlencoding::encode(&text).to_string();
                    self.replace_selection(encoded);
                }
            }
            MenuAction::UrlDecode => {
                if let Some(text) = self.get_selected_text() {
                    if let Ok(decoded) = urlencoding::decode(&text) {
                        self.replace_selection(decoded.to_string());
                    }
                }
            }
            MenuAction::CountWords => {
                if let Some(text) = self.get_selected_text() {
                    let count = text.split_whitespace().count();
                    let msg = format!("Word count: {}", count);
                    // For now, just replace selection with count
                    // Later we might want to show this in a status message
                    self.textarea.insert_str(&msg);
                }
            }
            MenuAction::CountChars => {
                if let Some(text) = self.get_selected_text() {
                    let count = text.chars().count();
                    let msg = format!("Character count: {}", count);
                    self.textarea.insert_str(&msg);
                }
            }
            MenuAction::CountLines => {
                if let Some(text) = self.get_selected_text() {
                    let count = text.lines().count();
                    let msg = format!("Line count: {}", count);
                    self.textarea.insert_str(&msg);
                }
            }
            MenuAction::InsertTime => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap();
                // Simple time format - could enhance with chrono crate later
                let time = format!("{:02}:{:02}", (now.as_secs() / 3600) % 24, (now.as_secs() / 60) % 60);
                self.textarea.insert_str(&time);
            }
            MenuAction::InsertDateTime => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap();
                // Simple date/time format - could enhance with chrono crate later
                let datetime = format!("{}", now.as_secs());
                self.textarea.insert_str(&datetime);
            }
            MenuAction::InsertNumbers => {
                let numbers = "1.\n2.\n3.\n4.\n5.";
                self.textarea.insert_str(numbers);
            }
            MenuAction::InsertTodo => {
                let todos = "☐ Task 1\n☐ Task 2\n☐ Task 3";
                self.textarea.insert_str(todos);
            }
        }

        // Cancel mark/selection after applying action
        if self.mark_active {
            self.cancel_mark();
        }

        // Close floating window after applying
        self.floating_window = None;
        self.focus_floating = false;
    }

    fn replace_selection(&mut self, new_text: String) {
        // Delete selected text
        self.textarea.cut();
        // Insert new text
        self.textarea.insert_str(&new_text);
        // Cancel mark
        self.cancel_mark();
    }

    // ==================== File Operations ====================

    /// Expand ~ to home directory and resolve path
    pub fn expand_path(path_str: &str) -> PathBuf {
        if path_str.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                if path_str == "~" {
                    return home;
                } else if path_str.starts_with("~/") {
                    return home.join(&path_str[2..]);
                }
            }
        }
        PathBuf::from(path_str)
    }

    /// Get filesystem completions for a partial path
    pub fn get_path_completions(partial: &str) -> Vec<String> {
        let expanded = Self::expand_path(partial);

        // Determine parent directory and prefix
        let (dir, prefix) = if partial.ends_with('/') || partial.ends_with(std::path::MAIN_SEPARATOR) {
            (expanded.clone(), String::new())
        } else {
            let parent = expanded.parent().unwrap_or(&expanded);
            let file_name = expanded.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            (parent.to_path_buf(), file_name.to_string())
        };

        let mut completions = Vec::new();

        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    // Case-insensitive prefix match
                    if name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                        let mut completion = if partial.starts_with('~') && !partial.starts_with("~/") {
                            // Handle bare ~ case
                            format!("~/{}", name)
                        } else if partial.ends_with('/') || partial.ends_with(std::path::MAIN_SEPARATOR) {
                            format!("{}{}", partial, name)
                        } else {
                            // Replace the last component
                            let parent_str = if partial.contains('/') || partial.contains(std::path::MAIN_SEPARATOR) {
                                let sep_pos = partial.rfind(|c| c == '/' || c == std::path::MAIN_SEPARATOR).unwrap();
                                &partial[..=sep_pos]
                            } else {
                                ""
                            };
                            format!("{}{}", parent_str, name)
                        };

                        // Add trailing slash for directories
                        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            completion.push('/');
                        }

                        completions.push(completion);
                    }
                }
            }
        }

        completions.sort();
        completions
    }

    /// Open minibuffer for file selection (C-x C-f)
    pub fn open_file_prompt(&mut self) {
        // Default to current directory or current file's directory
        let initial_path = if let Some(ref current) = self.current_file {
            current.parent()
                .map(|p| format!("{}/", p.display()))
                .unwrap_or_else(|| "./".to_string())
        } else {
            std::env::current_dir()
                .map(|p| format!("{}/", p.display()))
                .unwrap_or_else(|_| "~/".to_string())
        };

        let completions = Self::get_path_completions(&initial_path);

        self.floating_window = Some(FloatingWindow {
            textarea: TextArea::default(),
            visible: true,
            x: 0,
            y: 0,  // Will be positioned at bottom by UI
            width: 80,
            height: 1,
            mode: FloatingMode::Minibuffer {
                prompt: "Find file: ".to_string(),
                input: initial_path.clone(),
                cursor_pos: initial_path.len(),
                completions,
                selected_completion: None,
                callback: MinibufferCallback::OpenFile,
            },
        });
        self.focus_floating = true;
    }

    /// Open file from path, load into textarea
    pub fn open_file(&mut self, path: &std::path::Path) -> io::Result<()> {
        let mut file = fs::File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        // Create new textarea with file contents
        let lines: Vec<&str> = contents.lines().collect();
        self.textarea = if lines.is_empty() {
            TextArea::default()
        } else {
            TextArea::new(lines.iter().map(|s| s.to_string()).collect())
        };

        // Reapply styling
        self.update_textarea_colors();
        self.textarea.set_cursor_line_style(ratatui::style::Style::default());

        // Update file state
        self.current_file = Some(path.to_path_buf());
        self.modified = false;

        // Reset editor state
        self.mark_active = false;
        self.mark_set = false;
        self.mark_position = None;

        Ok(())
    }

    /// Save current buffer to current_file (or prompt if none)
    pub fn save_file(&mut self) -> io::Result<()> {
        if let Some(ref path) = self.current_file.clone() {
            self.save_file_to(path)
        } else {
            // No current file, need to prompt
            self.save_file_as_prompt();
            Ok(())
        }
    }

    /// Open minibuffer for save-as path (C-x C-w)
    pub fn save_file_as_prompt(&mut self) {
        let initial_path = if let Some(ref current) = self.current_file {
            current.display().to_string()
        } else {
            std::env::current_dir()
                .map(|p| format!("{}/", p.display()))
                .unwrap_or_else(|_| "~/".to_string())
        };

        let completions = Self::get_path_completions(&initial_path);

        self.floating_window = Some(FloatingWindow {
            textarea: TextArea::default(),
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Minibuffer {
                prompt: "Save as: ".to_string(),
                input: initial_path.clone(),
                cursor_pos: initial_path.len(),
                completions,
                selected_completion: None,
                callback: MinibufferCallback::SaveFileAs,
            },
        });
        self.focus_floating = true;
    }

    /// Save to specific path
    pub fn save_file_to(&mut self, path: &std::path::Path) -> io::Result<()> {
        let contents = self.textarea.lines().join("\n");
        let mut file = fs::File::create(path)?;
        file.write_all(contents.as_bytes())?;

        self.current_file = Some(path.to_path_buf());
        self.modified = false;

        Ok(())
    }

    /// Start delete file confirmation chain (C-x k)
    pub fn delete_file_prompt(&mut self) {
        // Default to current file if one is open
        let initial_path = self.current_file
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|p| format!("{}/", p.display()))
                    .unwrap_or_else(|_| "~/".to_string())
            });

        let completions = Self::get_path_completions(&initial_path);

        self.floating_window = Some(FloatingWindow {
            textarea: TextArea::default(),
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Minibuffer {
                prompt: "Delete file: ".to_string(),
                input: initial_path.clone(),
                cursor_pos: initial_path.len(),
                completions,
                selected_completion: None,
                callback: MinibufferCallback::DeleteFile,
            },
        });
        self.focus_floating = true;
    }

    /// Start the confirmation dialog for deleting a file
    pub fn start_delete_confirmation(&mut self, path: PathBuf) {
        let dialog = DeleteFileConfirmation { path };
        let steps = dialog.steps();

        self.floating_window = Some(FloatingWindow {
            textarea: TextArea::default(),
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Confirm {
                dialog: Box::new(dialog),
                steps,
                current_index: 0,
                text_input: String::new(),
            },
        });
        self.focus_floating = true;
    }

    /// Apply completion to minibuffer input
    pub fn apply_minibuffer_completion(&mut self) {
        if let Some(ref mut fw) = self.floating_window {
            if let FloatingMode::Minibuffer {
                ref mut input,
                ref mut cursor_pos,
                ref mut completions,
                ref mut selected_completion,
                ..
            } = fw.mode {
                if completions.is_empty() {
                    // Refresh completions
                    *completions = Self::get_path_completions(input);
                    if !completions.is_empty() {
                        *selected_completion = Some(0);
                    }
                } else if let Some(idx) = *selected_completion {
                    // Apply selected completion
                    if let Some(completion) = completions.get(idx) {
                        *input = completion.clone();
                        *cursor_pos = input.len();
                        // Refresh completions for new input
                        *completions = Self::get_path_completions(input);
                        *selected_completion = if completions.is_empty() { None } else { Some(0) };
                    }
                } else if !completions.is_empty() {
                    // No selection, select first
                    *selected_completion = Some(0);
                }
            }
        }
    }

    /// Execute minibuffer callback with current input
    pub fn execute_minibuffer_callback(&mut self) {
        if let Some(ref fw) = self.floating_window {
            if let FloatingMode::Minibuffer { ref input, ref callback, .. } = fw.mode {
                let path = Self::expand_path(input);
                let callback_clone = callback.clone();
                let path_clone = path.clone();

                // Close minibuffer first
                self.floating_window = None;
                self.focus_floating = false;

                // Execute callback
                match callback_clone {
                    MinibufferCallback::OpenFile => {
                        if let Err(e) = self.open_file(&path_clone) {
                            // TODO: Show error message to user
                            eprintln!("Failed to open file: {}", e);
                        }
                    }
                    MinibufferCallback::SaveFileAs => {
                        if let Err(e) = self.save_file_to(&path_clone) {
                            eprintln!("Failed to save file: {}", e);
                        }
                    }
                    MinibufferCallback::DeleteFile => {
                        // Start delete confirmation
                        self.start_delete_confirmation(path_clone);
                    }
                }
            }
        }
    }

    /// Mark buffer as modified (called when text changes)
    pub fn mark_modified(&mut self) {
        self.modified = true;
    }

    /// Start the quit confirmation dialog (when buffer is modified)
    pub fn start_quit_confirmation(&mut self) {
        let dialog = QuitConfirmation;
        let steps = dialog.steps();

        self.floating_window = Some(FloatingWindow {
            textarea: TextArea::default(),
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Confirm {
                dialog: Box::new(dialog),
                steps,
                current_index: 0,
                text_input: String::new(),
            },
        });
        self.focus_floating = true;
    }

    /// Open the M-x command palette
    pub fn open_command_palette(&mut self) {
        // Get all commands and convert to CommandInfo
        let all_commands: Vec<CommandInfo> = self.status_bar.command_registry
            .all_commands()
            .map(|cmd| CommandInfo {
                name: cmd.name,
                description: cmd.description,
                keybinding: cmd.keybinding.as_ref().map(|kb| kb.display()),
            })
            .collect();

        // Sort commands by name for consistent ordering
        let mut sorted_commands = all_commands;
        sorted_commands.sort_by(|a, b| a.name.cmp(b.name));

        self.floating_window = Some(FloatingWindow {
            textarea: TextArea::default(),
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::CommandPalette {
                input: String::new(),
                cursor_pos: 0,
                filtered_commands: sorted_commands,
                selected: 0,
            },
        });
        self.focus_floating = true;
    }

    /// Filter commands for command palette based on search input
    pub fn filter_commands(&self, query: &str) -> Vec<CommandInfo> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<CommandInfo> = self.status_bar.command_registry
            .all_commands()
            .filter(|cmd| {
                cmd.name.to_lowercase().contains(&query_lower)
                    || cmd.description.to_lowercase().contains(&query_lower)
            })
            .map(|cmd| CommandInfo {
                name: cmd.name,
                description: cmd.description,
                keybinding: cmd.keybinding.as_ref().map(|kb| kb.display()),
            })
            .collect();

        // Sort by relevance - exact name match first, then starts with, then contains
        results.sort_by(|a, b| {
            let a_exact = a.name.to_lowercase() == query_lower;
            let b_exact = b.name.to_lowercase() == query_lower;
            let a_starts = a.name.to_lowercase().starts_with(&query_lower);
            let b_starts = b.name.to_lowercase().starts_with(&query_lower);

            match (a_exact, b_exact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => match (a_starts, b_starts) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(b.name),
                },
            }
        });

        results
    }
}