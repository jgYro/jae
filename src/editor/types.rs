//! Type definitions for the JAE editor.
//!
//! This module contains all the core types used throughout the editor:
//! - Menu system types (MenuAction, MenuItem, MenuState)
//! - Floating window types (FloatingWindow, FloatingMode)
//! - Confirmation dialog types (ResponseType, ResponseResult, ConfirmationStep)
//! - Settings types (SettingItem, SettingValue)
//! - Mark/selection state (MarkState)

use crate::commands::KeyPrefix;

/// Actions that can be performed from the menu
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

/// A menu item - either an action or a category containing more items
#[derive(Clone)]
pub enum MenuItem {
    Action(MenuAction, String), // (action, label)
    Category(String, Vec<MenuItem>), // (category name, items)
}

/// State for menu navigation with breadcrumb trail
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
    /// Stay on current step (e.g., invalid input)
    Stay,
    /// Cancel the entire operation
    Cancel,
    /// Finish immediately (skip any remaining steps)
    Finish,
}

/// Command info for display in command palette
pub struct CommandInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub keybinding: Option<String>,
}

/// A setting item with name, value, and description
#[derive(Clone)]
pub struct SettingItem {
    pub name: String,
    pub value: SettingValue,
    pub description: String,
}

/// Possible values for a setting
#[derive(Clone)]
pub enum SettingValue {
    Bool(bool),
    Number(u16),
    Choice { current: usize, options: Vec<String> },
}

/// The different modes a floating window can be in
pub enum FloatingMode {
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
        dialog: Box<dyn super::dialogs::ConfirmationDialog>,
        steps: Vec<ConfirmationStep>,
        current_index: usize,
    },
    /// Command palette (M-x) for executing commands by name
    CommandPalette {
        input: String,
        cursor_pos: usize,
        filtered_commands: Vec<CommandInfo>,
        selected: usize,
    },
}

/// A floating window that can display various modes
pub struct FloatingWindow {
    pub visible: bool,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub mode: FloatingMode,
}

/// Mark/selection state - replaces mark_active, mark_set, mark_position
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum MarkState {
    /// No mark set
    #[default]
    None,
    /// Mark set but selection not active (after C-SPC C-SPC)
    Set { row: usize, col: usize },
    /// Active selection from mark to cursor
    Active { row: usize, col: usize },
}

impl MarkState {
    /// Check if the mark is active (selection visible)
    pub fn is_active(&self) -> bool {
        matches!(self, MarkState::Active { .. })
    }

    /// Check if any mark is set (active or inactive)
    #[allow(dead_code)]
    pub fn is_set(&self) -> bool {
        !matches!(self, MarkState::None)
    }

    /// Get the mark position if set
    pub fn position(&self) -> Option<(usize, usize)> {
        match self {
            MarkState::Active { row, col } | MarkState::Set { row, col } => Some((*row, *col)),
            MarkState::None => None,
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
    pub command_registry: crate::commands::CommandRegistry,
    pub which_key_page: usize,
}

impl StatusBarState {
    pub fn new() -> Self {
        Self {
            expanded: false,
            active_prefix: None,
            which_key_items: Vec::new(),
            command_registry: crate::commands::CommandRegistry::new(),
            which_key_page: 0,
        }
    }

    /// Activate a prefix and expand which-key immediately
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
        self.which_key_page = 0;
        self.expanded = true;
    }

    /// Check if there's an active prefix
    pub fn has_active_prefix(&self) -> bool {
        self.active_prefix.is_some()
    }

    /// Clear the active prefix and collapse the status bar
    pub fn clear_prefix(&mut self) {
        self.active_prefix = None;
        self.which_key_items.clear();
        self.expanded = false;
        self.which_key_page = 0;
    }

    /// Get the display name of the active prefix
    pub fn prefix_display_name(&self) -> Option<&'static str> {
        self.active_prefix.as_ref().map(|p| p.display_name())
    }

    /// Navigate to next page of which-key (M->)
    pub fn which_key_next_page(&mut self, items_per_page: usize) {
        if self.which_key_items.is_empty() || items_per_page == 0 {
            return;
        }
        let total_pages = self.which_key_items.len().div_ceil(items_per_page);
        if self.which_key_page + 1 < total_pages {
            self.which_key_page += 1;
        }
    }

    /// Navigate to previous page of which-key (M-<)
    pub fn which_key_prev_page(&mut self) {
        if self.which_key_page > 0 {
            self.which_key_page -= 1;
        }
    }

    /// Get total number of pages
    pub fn which_key_total_pages(&self, items_per_page: usize) -> usize {
        if self.which_key_items.is_empty() || items_per_page == 0 {
            return 1;
        }
        self.which_key_items.len().div_ceil(items_per_page)
    }
}

impl Default for StatusBarState {
    fn default() -> Self {
        Self::new()
    }
}
