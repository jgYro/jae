use crate::kill_ring::KillRing;
use tui_textarea::{CursorMove, TextArea};

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
    pub last_key: Option<(ratatui::crossterm::event::KeyCode, ratatui::crossterm::event::KeyModifiers)>,
}

impl Editor {
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
            last_key: None,
        }
    }

    pub fn set_mark(&mut self) {
        let cursor_pos = self.textarea.cursor();

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
            self.textarea.move_cursor(CursorMove::Jump(current_cursor.0 as u16, current_cursor.1 as u16));

            // Start selection from current cursor position
            self.textarea.start_selection();

            // Move cursor to the saved mark (where cursor will end up)
            self.textarea.move_cursor(CursorMove::Jump(saved_mark.0 as u16, saved_mark.1 as u16));

            self.mark_active = true;
        } else {
            // No active selection, create one between cursor and mark
            // Move to current cursor position first (will be the new anchor)
            self.textarea.move_cursor(CursorMove::Jump(current_cursor.0 as u16, current_cursor.1 as u16));

            // Start selection from here
            self.textarea.start_selection();
            self.mark_active = true;

            // Move to saved mark (where cursor ends up)
            self.textarea.move_cursor(CursorMove::Jump(saved_mark.0 as u16, saved_mark.1 as u16));
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
        let mut new_col = col;

        // Skip current word
        while new_col < line.len() && !line.chars().nth(new_col).unwrap_or(' ').is_whitespace() {
            new_col += 1;
        }

        // Skip whitespace
        while new_col < line.len() && line.chars().nth(new_col).unwrap_or(' ').is_whitespace() {
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

        if col == 0 {
            if row > 0 {
                self.textarea.move_cursor(CursorMove::Up);
                self.textarea.move_cursor(CursorMove::End);
            }
            // Don't reset last_was_kill on movement
            return;
        }

        let mut new_col = col - 1;

        // Skip whitespace
        while new_col > 0 && line.chars().nth(new_col).unwrap_or(' ').is_whitespace() {
            new_col -= 1;
        }

        // Skip word
        while new_col > 0 && !line.chars().nth(new_col - 1).unwrap_or(' ').is_whitespace() {
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

            if col < line.len() {
                // Kill from cursor to end of line
                (line[col..].to_string(), true)
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
            line[0..col].to_string()
        };

        if !killed_text.is_empty() {
            let text_len = killed_text.len();
            // Add to kill ring (C-u doesn't append to previous kills)
            self.kill_ring.push(killed_text);

            // Move to beginning of line
            self.textarea.move_cursor(CursorMove::Head);

            // Start selection and move forward to select the text
            self.textarea.start_selection();
            for _ in 0..text_len {
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

        // Limit preview length
        let preview_text = if text.len() > 50 {
            format!("{}...", &text[..50])
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
}