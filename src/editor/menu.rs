//! Menu operations for the editor.

use super::{
    Editor, FloatingMode, FloatingWindow, MenuAction, MenuItem, MenuState, SettingItem,
    SettingValue,
};

impl Editor {
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
        let (action_opt, selected_text_opt) = match &self.floating_window {
            Some(fw) => match &fw.mode {
                FloatingMode::Menu { state, .. } => match state.items.get(state.selected) {
                    Some(MenuItem::Action(action, _)) => (Some(action.clone()), self.get_selected_text()),
                    _ => (None, None),
                },
                _ => (None, None),
            },
            None => (None, None),
        };

        match (action_opt, selected_text_opt) {
            (Some(action), Some(selected_text)) => {
                let preview_text = self.generate_preview(&action, &selected_text);
                let metadata_text = match self.settings.show_metadata {
                    true => self.generate_metadata(&action, &selected_text),
                    false => None,
                };

                match &mut self.floating_window {
                    Some(fw) => match &mut fw.mode {
                        FloatingMode::Menu { preview, metadata, .. } => {
                            *preview = preview_text;
                            *metadata = metadata_text;
                        }
                        _ => {}
                    },
                    None => {}
                }
            }
            _ => match &mut self.floating_window {
                Some(fw) => match &mut fw.mode {
                    FloatingMode::Menu { preview, metadata, .. } => {
                        *preview = None;
                        *metadata = None;
                    }
                    _ => {}
                },
                None => {}
            },
        }
    }

    fn generate_preview(&self, action: &MenuAction, text: &str) -> Option<String> {
        match self.settings.show_preview {
            false => return None,
            true => {}
        }

        let preview_text = match text.chars().count() > 50 {
            true => format!("{}...", text.chars().take(50).collect::<String>()),
            false => text.to_string(),
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
        match self.floating_window.is_some() {
            true => {
                self.floating_window = None;
                self.focus_floating = false;
            }
            false => {
                let selection_range = self.textarea.selection_range();
                let was_mark_active = self.mark.is_active();

                let (cursor_row, cursor_col) = self.textarea.cursor();
                let x = (cursor_col as u16).saturating_add(5).min(80);
                let y = (cursor_row as u16).saturating_add(2).min(20);

                let root_items = match was_mark_active && selection_range.is_some() {
                    true => vec![
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
                    ],
                    false => vec![
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
                    ],
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
    }

    pub fn apply_menu_option(&mut self, action: MenuAction) {
        // Save undo state before any menu operation that modifies text
        self.save_undo_state();
        match action {
            MenuAction::Uppercase => match self.get_selected_text() {
                Some(text) => {
                    let transformed = text.to_uppercase();
                    self.replace_selection(transformed);
                }
                None => {}
            },
            MenuAction::Lowercase => match self.get_selected_text() {
                Some(text) => {
                    let transformed = text.to_lowercase();
                    self.replace_selection(transformed);
                }
                None => {}
            },
            MenuAction::Capitalize => match self.get_selected_text() {
                Some(text) => {
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
                None => {}
            },
            MenuAction::Reverse => match self.get_selected_text() {
                Some(text) => {
                    let transformed = text.chars().rev().collect();
                    self.replace_selection(transformed);
                }
                None => {}
            },
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
            MenuAction::Base64Encode => match self.get_selected_text() {
                Some(text) => {
                    use base64::{engine::general_purpose, Engine as _};
                    let encoded = general_purpose::STANDARD.encode(text.as_bytes());
                    self.replace_selection(encoded);
                }
                None => {}
            },
            MenuAction::Base64Decode => match self.get_selected_text() {
                Some(text) => {
                    use base64::{engine::general_purpose, Engine as _};
                    match general_purpose::STANDARD.decode(text.as_bytes()) {
                        Ok(decoded_bytes) => match String::from_utf8(decoded_bytes) {
                            Ok(decoded_str) => self.replace_selection(decoded_str),
                            Err(_) => {}
                        },
                        Err(_) => {}
                    }
                }
                None => {}
            },
            MenuAction::UrlEncode => match self.get_selected_text() {
                Some(text) => {
                    let encoded = urlencoding::encode(&text).to_string();
                    self.replace_selection(encoded);
                }
                None => {}
            },
            MenuAction::UrlDecode => match self.get_selected_text() {
                Some(text) => match urlencoding::decode(&text) {
                    Ok(decoded) => self.replace_selection(decoded.to_string()),
                    Err(_) => {}
                },
                None => {}
            },
            MenuAction::CountWords => match self.get_selected_text() {
                Some(text) => {
                    let count = text.split_whitespace().count();
                    let msg = format!("Word count: {}", count);
                    self.textarea.insert_str(&msg);
                }
                None => {}
            },
            MenuAction::CountChars => match self.get_selected_text() {
                Some(text) => {
                    let count = text.chars().count();
                    let msg = format!("Character count: {}", count);
                    self.textarea.insert_str(&msg);
                }
                None => {}
            },
            MenuAction::CountLines => match self.get_selected_text() {
                Some(text) => {
                    let count = text.lines().count();
                    let msg = format!("Line count: {}", count);
                    self.textarea.insert_str(&msg);
                }
                None => {}
            },
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
        match self.mark.is_active() {
            true => self.cancel_mark(),
            false => {}
        }

        self.floating_window = None;
        self.focus_floating = false;
    }

    pub(super) fn replace_selection(&mut self, new_text: String) {
        self.textarea.cut();
        self.textarea.insert_str(&new_text);
        self.cancel_mark();
    }
}
