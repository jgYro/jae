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
    Menu { state: MenuState, root_items: Vec<MenuItem> },
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

pub struct Editor {
    pub textarea: TextArea<'static>,
    pub mark_active: bool,
    pub kill_ring: KillRing,
    pub last_was_kill: bool,
    pub floating_window: Option<FloatingWindow>,
    pub focus_floating: bool,
}

impl Editor {
    pub fn new() -> Self {
        let mut textarea = TextArea::default();
        // Set cursor to be a visible block with inverted colors
        textarea.set_cursor_style(
            ratatui::style::Style::default()
                .add_modifier(ratatui::style::Modifier::REVERSED)
        );
        // Remove underline from the current line
        textarea.set_cursor_line_style(ratatui::style::Style::default());
        // Set selection style to be visible (highlighted with reversed colors)
        textarea.set_selection_style(
            ratatui::style::Style::default()
                .add_modifier(ratatui::style::Modifier::REVERSED)
        );

        Self {
            textarea,
            mark_active: false,
            kill_ring: KillRing::new(),
            last_was_kill: false,
            floating_window: None,
            focus_floating: false,
        }
    }

    pub fn set_mark(&mut self) {
        if !self.mark_active {
            self.textarea.start_selection();
            self.mark_active = true;
        } else {
            self.cancel_mark();
        }
        // Don't reset last_was_kill when setting mark
    }

    pub fn cancel_mark(&mut self) {
        self.textarea.cancel_selection();
        self.mark_active = false;
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
            };

            self.floating_window = Some(FloatingWindow {
                textarea: floating_textarea,
                visible: true,
                x,
                y,
                width: 40,
                height: 10,
                mode,
            });
            self.focus_floating = true;

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
        self.mark_active = false;
    }
}