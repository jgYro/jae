//! Editor module for JAE - Just Another Editor.
//!
//! This module contains the core Editor struct and all supporting types
//! organized into submodules for maintainability.

pub mod dialogs;
pub mod settings;
pub mod types;
pub mod undo;

// Re-export commonly used types
pub use dialogs::{ConfirmationDialog, DeleteFileConfirmation, QuitConfirmation};
pub use settings::Settings;
pub use types::{
    CommandInfo, FloatingMode, FloatingWindow, MarkState, MenuAction, MenuItem, MenuState,
    MinibufferCallback, ResponseResult, ResponseType, SettingItem, SettingValue, StatusBarState,
};
pub use undo::{EditorSnapshot, UndoManager};

use crate::clipboard::ClipboardManager;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::{Color, Modifier, Style};
use std::cmp::min;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use tui_textarea::{CursorMove, TextArea};

/// The main editor state
pub struct Editor {
    pub textarea: TextArea<'static>,
    pub mark: MarkState,
    pub clipboard: ClipboardManager,
    pub floating_window: Option<FloatingWindow>,
    pub focus_floating: bool,
    pub settings: Settings,
    pub status_bar: StatusBarState,
    pub last_key: Option<(KeyCode, KeyModifiers)>,
    pub current_file: Option<PathBuf>,
    pub modified: bool,
    pub pending_quit: bool,
    pub undo_manager: UndoManager,
}

impl Editor {
    /// Create a new editor instance
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        let settings = Settings::default();

        // Set cursor to be a visible block with red background
        textarea.set_cursor_style(
            Style::default()
                .bg(settings.cursor_color)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        // Remove underline from the current line
        textarea.set_cursor_line_style(Style::default());
        // Set selection style to be magenta/purple background (distinct from cursor)
        textarea.set_selection_style(
            Style::default()
                .bg(settings.selection_color)
                .fg(Color::White),
        );

        Self {
            textarea,
            mark: MarkState::None,
            clipboard: ClipboardManager::new(),
            floating_window: None,
            focus_floating: false,
            settings,
            status_bar: StatusBarState::new(),
            last_key: None,
            current_file: None,
            modified: false,
            pending_quit: false,
            undo_manager: UndoManager::new(),
        }
    }

    /// Update textarea colors based on current settings
    pub fn update_textarea_colors(&mut self) {
        self.textarea.set_cursor_style(
            Style::default()
                .bg(self.settings.cursor_color)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        self.textarea.set_selection_style(
            Style::default()
                .bg(self.settings.selection_color)
                .fg(Color::White),
        );
    }

    // ==================== Undo/Redo ====================

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

    // ==================== String Utilities ====================

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

    /// Cut selected text to system clipboard (C-w)
    pub fn cut_region(&mut self) {
        if let Some(text) = self.get_selected_text() {
            self.clipboard.copy(&text);
            self.textarea.cut();
            self.mark = MarkState::None;
        }
    }

    /// Copy selected text to system clipboard (M-w)
    pub fn copy_region(&mut self) {
        if let Some(text) = self.get_selected_text() {
            self.clipboard.copy(&text);
            self.cancel_mark();
        }
    }

    /// Paste from system clipboard (C-y)
    pub fn paste(&mut self) {
        if let Some(text) = self.clipboard.paste() {
            self.textarea.insert_str(&text);
        }
    }

    /// Cut from cursor to end of line to clipboard (C-k)
    pub fn cut_to_end_of_line(&mut self) {
        let (row, col) = self.textarea.cursor();

        let (cut_text, should_move_to_end) = {
            let lines = self.textarea.lines();

            if row >= lines.len() {
                return;
            }

            let line = &lines[row];

            if col < line.chars().count() {
                (Self::safe_string_slice(line, col, line.chars().count()), true)
            } else if row + 1 < lines.len() {
                ("\n".to_string(), false)
            } else {
                return;
            }
        };

        if !cut_text.is_empty() {
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
        }
    }

    /// Cut from beginning of line to cursor to clipboard (C-u)
    pub fn cut_to_beginning_of_line(&mut self) {
        let (row, col) = self.textarea.cursor();

        let cut_text = {
            let lines = self.textarea.lines();

            if row >= lines.len() || col == 0 {
                return;
            }

            let line = &lines[row];
            Self::safe_string_slice(line, 0, col)
        };

        if !cut_text.is_empty() {
            self.clipboard.copy(&cut_text);

            self.textarea.move_cursor(CursorMove::Head);
            self.textarea.start_selection();
            for _ in 0..col {
                self.textarea.move_cursor(CursorMove::Forward);
            }

            self.textarea.cut();
            self.mark = MarkState::None;
        }
    }

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

    // ==================== Menu Operations ====================

    pub fn open_settings_menu(&mut self) {
        self.floating_window = None;
        self.focus_floating = false;

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
                    current: self.settings.get_color_index(self.settings.cursor_color),
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
                    current: self.settings.get_color_index(self.settings.selection_color),
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

        self.floating_window = Some(FloatingWindow {
            visible: true,
            x: 10,
            y: 5,
            width: 50,
            height: 15,
            mode,
        });
        self.focus_floating = true;
    }

    pub fn update_menu_preview(&mut self) {
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

        if let (Some(action), Some(selected_text)) = (action_opt, selected_text_opt) {
            let preview_text = self.generate_preview(&action, &selected_text);
            let metadata_text = if self.settings.show_metadata {
                self.generate_metadata(&action, &selected_text)
            } else {
                None
            };

            if let Some(ref mut fw) = self.floating_window {
                if let FloatingMode::Menu { preview, metadata, .. } = &mut fw.mode {
                    *preview = preview_text;
                    *metadata = metadata_text;
                }
            }
        } else {
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

        let preview_text = if text.chars().count() > 50 {
            format!("{}...", text.chars().take(50).collect::<String>())
        } else {
            text.to_string()
        };

        match action {
            MenuAction::Uppercase => Some(preview_text.to_uppercase()),
            MenuAction::Lowercase => Some(preview_text.to_lowercase()),
            MenuAction::Capitalize => Some(
                preview_text
                    .split_whitespace()
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(first) => first
                                .to_uppercase()
                                .chain(chars.as_str().to_lowercase().chars())
                                .collect(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            MenuAction::Reverse => Some(preview_text.chars().rev().collect()),
            MenuAction::Base64Encode => {
                use base64::{engine::general_purpose, Engine as _};
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
            self.floating_window = None;
            self.focus_floating = false;
        } else {
            let selection_range = self.textarea.selection_range();
            let was_mark_active = self.mark.is_active();

            let (cursor_row, cursor_col) = self.textarea.cursor();
            let x = (cursor_col as u16).saturating_add(5).min(80);
            let y = (cursor_row as u16).saturating_add(2).min(20);

            let root_items = if was_mark_active && selection_range.is_some() {
                vec![
                    MenuItem::Category(
                        "Transform Case".to_string(),
                        vec![
                            MenuItem::Action(MenuAction::Uppercase, "UPPERCASE".to_string()),
                            MenuItem::Action(MenuAction::Lowercase, "lowercase".to_string()),
                            MenuItem::Action(MenuAction::Capitalize, "Capitalize Words".to_string()),
                        ],
                    ),
                    MenuItem::Category(
                        "Encoding/Decoding".to_string(),
                        vec![
                            MenuItem::Action(MenuAction::Base64Encode, "Base64 Encode".to_string()),
                            MenuItem::Action(MenuAction::Base64Decode, "Base64 Decode".to_string()),
                            MenuItem::Action(MenuAction::UrlEncode, "URL Encode".to_string()),
                            MenuItem::Action(MenuAction::UrlDecode, "URL Decode".to_string()),
                        ],
                    ),
                    MenuItem::Action(MenuAction::Reverse, "Reverse Text".to_string()),
                    MenuItem::Category(
                        "Text Analysis".to_string(),
                        vec![
                            MenuItem::Action(MenuAction::CountWords, "Count Words".to_string()),
                            MenuItem::Action(MenuAction::CountChars, "Count Characters".to_string()),
                            MenuItem::Action(MenuAction::CountLines, "Count Lines".to_string()),
                        ],
                    ),
                ]
            } else {
                vec![
                    MenuItem::Category(
                        "Insert Date/Time".to_string(),
                        vec![
                            MenuItem::Action(MenuAction::InsertDate, "Insert Date".to_string()),
                            MenuItem::Action(MenuAction::InsertTime, "Insert Time".to_string()),
                            MenuItem::Action(
                                MenuAction::InsertDateTime,
                                "Insert Date & Time".to_string(),
                            ),
                        ],
                    ),
                    MenuItem::Category(
                        "Insert Templates".to_string(),
                        vec![
                            MenuItem::Action(MenuAction::InsertLorem, "Lorem Ipsum".to_string()),
                            MenuItem::Action(MenuAction::InsertBullets, "Bullet List".to_string()),
                            MenuItem::Action(MenuAction::InsertNumbers, "Numbered List".to_string()),
                            MenuItem::Action(MenuAction::InsertTodo, "TODO List".to_string()),
                        ],
                    ),
                ]
            };

            let mode = FloatingMode::Menu {
                state: MenuState::new(root_items.clone()),
                root_items,
                preview: None,
                metadata: None,
            };

            self.floating_window = Some(FloatingWindow {
                visible: true,
                x,
                y,
                width: self.settings.floating_window_width,
                height: self.settings.floating_window_height,
                mode,
            });
            self.focus_floating = true;

            self.update_menu_preview();
        }
    }

    pub fn apply_menu_option(&mut self, action: MenuAction) {
        // Save undo state before any menu operation that modifies text
        self.save_undo_state();
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
                                Some(first) => first
                                    .to_uppercase()
                                    .chain(chars.as_str().to_lowercase().chars())
                                    .collect(),
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
                use chrono::Local;
                let date = Local::now().format("%Y-%m-%d").to_string();
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
                    use base64::{engine::general_purpose, Engine as _};
                    let encoded = general_purpose::STANDARD.encode(text.as_bytes());
                    self.replace_selection(encoded);
                }
            }
            MenuAction::Base64Decode => {
                if let Some(text) = self.get_selected_text() {
                    use base64::{engine::general_purpose, Engine as _};
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
                use chrono::Local;
                let time = Local::now().format("%H:%M:%S").to_string();
                self.textarea.insert_str(&time);
            }
            MenuAction::InsertDateTime => {
                use chrono::Local;
                let datetime = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
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
        if self.mark.is_active() {
            self.cancel_mark();
        }

        self.floating_window = None;
        self.focus_floating = false;
    }

    fn replace_selection(&mut self, new_text: String) {
        self.textarea.cut();
        self.textarea.insert_str(&new_text);
        self.cancel_mark();
    }

    // ==================== File Operations ====================

    /// Expand ~ to home directory and resolve path
    pub fn expand_path(path_str: &str) -> PathBuf {
        if path_str.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                if path_str == "~" {
                    return home;
                } else if let Some(rest) = path_str.strip_prefix("~/") {
                    return home.join(rest);
                }
            }
        }
        PathBuf::from(path_str)
    }

    /// Get filesystem completions for a partial path
    pub fn get_path_completions(partial: &str) -> Vec<String> {
        let expanded = Self::expand_path(partial);

        let (dir, prefix) = if partial.ends_with('/') || partial.ends_with(std::path::MAIN_SEPARATOR)
        {
            (expanded.clone(), String::new())
        } else {
            let parent = expanded.parent().unwrap_or(&expanded);
            let file_name = expanded
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            (parent.to_path_buf(), file_name.to_string())
        };

        let mut completions = Vec::new();

        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                        let mut completion = if partial.starts_with('~') && !partial.starts_with("~/")
                        {
                            format!("~/{}", name)
                        } else if partial.ends_with('/')
                            || partial.ends_with(std::path::MAIN_SEPARATOR)
                        {
                            format!("{}{}", partial, name)
                        } else {
                            let parent_str = if partial.contains('/')
                                || partial.contains(std::path::MAIN_SEPARATOR)
                            {
                                let sep_pos = partial
                                    .rfind(['/', std::path::MAIN_SEPARATOR])
                                    .unwrap();
                                &partial[..=sep_pos]
                            } else {
                                ""
                            };
                            format!("{}{}", parent_str, name)
                        };

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
        let initial_path = if let Some(ref current) = self.current_file {
            current
                .parent()
                .map(|p| format!("{}/", p.display()))
                .unwrap_or_else(|| "./".to_string())
        } else {
            std::env::current_dir()
                .map(|p| format!("{}/", p.display()))
                .unwrap_or_else(|_| "~/".to_string())
        };

        let completions = Self::get_path_completions(&initial_path);

        self.floating_window = Some(FloatingWindow {
            visible: true,
            x: 0,
            y: 0,
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

    /// Open minibuffer for directory browsing starting at specified path
    pub fn open_directory_prompt(&mut self, dir: &std::path::Path) {
        let dir_path = if dir.to_string_lossy().ends_with('/') {
            dir.to_string_lossy().to_string()
        } else {
            format!("{}/", dir.display())
        };

        let completions = Self::get_path_completions(&dir_path);

        self.floating_window = Some(FloatingWindow {
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Minibuffer {
                prompt: "Find file: ".to_string(),
                input: dir_path.clone(),
                cursor_pos: dir_path.len(),
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

        let lines: Vec<&str> = contents.lines().collect();
        self.textarea = if lines.is_empty() {
            TextArea::default()
        } else {
            TextArea::new(lines.iter().map(|s| s.to_string()).collect())
        };

        self.update_textarea_colors();
        self.textarea.set_cursor_line_style(Style::default());

        self.current_file = Some(path.to_path_buf());
        self.modified = false;
        self.mark = MarkState::None;
        self.undo_manager.clear();

        Ok(())
    }

    /// Save current buffer to current_file (or prompt if none)
    pub fn save_file(&mut self) -> io::Result<()> {
        if let Some(ref path) = self.current_file.clone() {
            self.save_file_to(path)
        } else {
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
        let initial_path = self
            .current_file
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|p| format!("{}/", p.display()))
                    .unwrap_or_else(|_| "~/".to_string())
            });

        let completions = Self::get_path_completions(&initial_path);

        self.floating_window = Some(FloatingWindow {
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
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Confirm {
                dialog: Box::new(dialog),
                steps,
                current_index: 0,
            },
        });
        self.focus_floating = true;
    }

    /// Execute minibuffer callback with current input
    pub fn execute_minibuffer_callback(&mut self) {
        if let Some(ref fw) = self.floating_window {
            if let FloatingMode::Minibuffer {
                ref input,
                ref callback,
                ..
            } = fw.mode
            {
                let path = Self::expand_path(input);
                let callback_clone = callback.clone();
                let path_clone = path.clone();

                self.floating_window = None;
                self.focus_floating = false;

                match callback_clone {
                    MinibufferCallback::OpenFile => {
                        if let Err(e) = self.open_file(&path_clone) {
                            eprintln!("Failed to open file: {}", e);
                        }
                    }
                    MinibufferCallback::SaveFileAs => {
                        if let Err(e) = self.save_file_to(&path_clone) {
                            eprintln!("Failed to save file: {}", e);
                        }
                    }
                    MinibufferCallback::DeleteFile => {
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
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Confirm {
                dialog: Box::new(dialog),
                steps,
                current_index: 0,
            },
        });
        self.focus_floating = true;
    }

    /// Open the M-x command palette
    pub fn open_command_palette(&mut self) {
        let all_commands: Vec<CommandInfo> = self
            .status_bar
            .command_registry
            .all_commands()
            .map(|cmd| CommandInfo {
                name: cmd.name,
                description: cmd.description,
                keybinding: cmd.keybinding.as_ref().map(|kb| kb.display()),
            })
            .collect();

        let mut sorted_commands = all_commands;
        sorted_commands.sort_by(|a, b| a.name.cmp(b.name));

        self.floating_window = Some(FloatingWindow {
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
        let mut results: Vec<CommandInfo> = self
            .status_bar
            .command_registry
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

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
