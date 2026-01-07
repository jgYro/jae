//! Command registry and execution system for JAE editor.
//!
//! This module provides:
//! - Named commands that map to editor actions
//! - A flexible prefix system for multi-key sequences (C-x, C-c, etc.)
//! - Command categories for organization in which-key display

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

/// Category for organizing commands in which-key display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    File,
    Edit,
    Movement,
    Selection,
    System,
    Input,
}


/// A single key with modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyCombo {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyCombo {
    pub fn new(key: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { key, modifiers }
    }

    pub fn ctrl(c: char) -> Self {
        Self {
            key: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
        }
    }

    pub fn alt(c: char) -> Self {
        Self {
            key: KeyCode::Char(c),
            modifiers: KeyModifiers::ALT,
        }
    }

    #[allow(dead_code)]
    pub fn ctrl_shift(c: char) -> Self {
        Self {
            key: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL.union(KeyModifiers::SHIFT),
        }
    }

    pub fn plain(c: char) -> Self {
        Self {
            key: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
        }
    }

    pub fn special(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// Format for display (e.g., "C-x", "M-f", "k")
    pub fn display(&self) -> String {
        let mut parts = Vec::new();

        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("C-");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("M-");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("S-");
        }

        let key_str = match self.key {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Delete => "Delete".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::Esc => "ESC".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PgUp".to_string(),
            KeyCode::PageDown => "PgDn".to_string(),
            _ => "?".to_string(),
        };

        format!("{}{}", parts.join(""), key_str)
    }
}

/// Represents a keybinding (single key or sequence)
#[derive(Debug, Clone)]
pub enum Keybinding {
    Single(KeyCombo),
    Sequence(Vec<KeyCombo>),
}

impl Keybinding {
    pub fn display(&self) -> String {
        match self {
            Keybinding::Single(k) => k.display(),
            Keybinding::Sequence(keys) => {
                keys.iter().map(|k| k.display()).collect::<Vec<_>>().join(" ")
            }
        }
    }
}

/// A named command that can be executed
#[derive(Debug, Clone)]
pub struct Command {
    pub name: &'static str,
    pub description: &'static str,
    #[allow(dead_code)]
    pub category: Category,
    pub keybinding: Option<Keybinding>,
}

/// Mapping from a follow-up key to a command name
#[derive(Debug, Clone)]
pub struct PrefixBinding {
    pub key: KeyCombo,
    pub command: &'static str,
}

/// Trait for defining a key prefix that triggers which-key
pub trait KeyPrefix: Send + Sync {
    /// The key combination that activates this prefix
    #[allow(dead_code)]
    fn trigger(&self) -> KeyCombo;

    /// Display name shown in which-key (e.g., "C-x")
    fn display_name(&self) -> &'static str;

    /// Look up which command to execute for a given follow-up key
    /// Returns None if the key is not recognized (cancels prefix)
    fn get_command(&self, key: &KeyEvent) -> Option<&'static str>;

    /// Returns all bindings for which-key display
    fn bindings(&self) -> Vec<PrefixBinding>;
}

/// C-x prefix implementation
pub struct CtrlXPrefix;

impl KeyPrefix for CtrlXPrefix {
    fn trigger(&self) -> KeyCombo {
        KeyCombo::ctrl('x')
    }

    fn display_name(&self) -> &'static str {
        "C-x"
    }

    fn get_command(&self, key: &KeyEvent) -> Option<&'static str> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('f'), KeyModifiers::CONTROL) => Some("open-file"),
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => Some("save-file"),
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => Some("save-file-as"),
            (KeyCode::Char('x'), KeyModifiers::CONTROL) => Some("swap-cursor-mark"),
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => Some("force-quit"),
            (KeyCode::Char('k'), KeyModifiers::NONE) => Some("delete-file"),
            (KeyCode::Char('w'), KeyModifiers::NONE) => Some("toggle-soft-wrap"),
            _ => None,
        }
    }

    fn bindings(&self) -> Vec<PrefixBinding> {
        vec![
            PrefixBinding { key: KeyCombo::ctrl('f'), command: "open-file" },
            PrefixBinding { key: KeyCombo::ctrl('s'), command: "save-file" },
            PrefixBinding { key: KeyCombo::ctrl('w'), command: "save-file-as" },
            PrefixBinding { key: KeyCombo::ctrl('x'), command: "swap-cursor-mark" },
            PrefixBinding { key: KeyCombo::ctrl('q'), command: "force-quit" },
            PrefixBinding { key: KeyCombo::plain('k'), command: "delete-file" },
            PrefixBinding { key: KeyCombo::plain('w'), command: "toggle-soft-wrap" },
        ]
    }
}

/// Command registry holding all available commands
pub struct CommandRegistry {
    commands: HashMap<&'static str, Command>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            commands: HashMap::new(),
        };
        registry.register_all_commands();
        registry
    }

    fn register_all_commands(&mut self) {
        // Movement commands
        self.register(Command {
            name: "forward-char",
            description: "Move cursor forward one character",
            category: Category::Movement,
            keybinding: Some(Keybinding::Single(KeyCombo::ctrl('f'))),
        });
        self.register(Command {
            name: "backward-char",
            description: "Move cursor backward one character",
            category: Category::Movement,
            keybinding: Some(Keybinding::Single(KeyCombo::ctrl('b'))),
        });
        self.register(Command {
            name: "next-line",
            description: "Move cursor to next line",
            category: Category::Movement,
            keybinding: Some(Keybinding::Single(KeyCombo::ctrl('n'))),
        });
        self.register(Command {
            name: "previous-line",
            description: "Move cursor to previous line",
            category: Category::Movement,
            keybinding: Some(Keybinding::Single(KeyCombo::ctrl('p'))),
        });
        self.register(Command {
            name: "beginning-of-line",
            description: "Move cursor to beginning of line",
            category: Category::Movement,
            keybinding: Some(Keybinding::Single(KeyCombo::ctrl('a'))),
        });
        self.register(Command {
            name: "end-of-line",
            description: "Move cursor to end of line",
            category: Category::Movement,
            keybinding: Some(Keybinding::Single(KeyCombo::ctrl('e'))),
        });
        self.register(Command {
            name: "forward-word",
            description: "Move cursor forward one word",
            category: Category::Movement,
            keybinding: Some(Keybinding::Single(KeyCombo::alt('f'))),
        });
        self.register(Command {
            name: "backward-word",
            description: "Move cursor backward one word",
            category: Category::Movement,
            keybinding: Some(Keybinding::Single(KeyCombo::alt('b'))),
        });

        // File commands
        self.register(Command {
            name: "open-file",
            description: "Open a file",
            category: Category::File,
            keybinding: Some(Keybinding::Sequence(vec![KeyCombo::ctrl('x'), KeyCombo::ctrl('f')])),
        });
        self.register(Command {
            name: "save-file",
            description: "Save the current buffer",
            category: Category::File,
            keybinding: Some(Keybinding::Sequence(vec![KeyCombo::ctrl('x'), KeyCombo::ctrl('s')])),
        });
        self.register(Command {
            name: "save-file-as",
            description: "Save buffer to a new file",
            category: Category::File,
            keybinding: Some(Keybinding::Sequence(vec![KeyCombo::ctrl('x'), KeyCombo::ctrl('w')])),
        });
        self.register(Command {
            name: "delete-file",
            description: "Delete current file",
            category: Category::File,
            keybinding: Some(Keybinding::Sequence(vec![KeyCombo::ctrl('x'), KeyCombo::plain('k')])),
        });

        // Edit commands
        self.register(Command {
            name: "kill-line",
            description: "Kill to end of line",
            category: Category::Edit,
            keybinding: Some(Keybinding::Single(KeyCombo::ctrl('k'))),
        });
        self.register(Command {
            name: "kill-line-backward",
            description: "Kill to beginning of line",
            category: Category::Edit,
            keybinding: None, // C-u now used for undo
        });
        self.register(Command {
            name: "kill-word",
            description: "Kill word forward",
            category: Category::Edit,
            keybinding: Some(Keybinding::Single(KeyCombo::alt('d'))),
        });
        self.register(Command {
            name: "kill-word-backward",
            description: "Kill word backward",
            category: Category::Edit,
            keybinding: Some(Keybinding::Single(KeyCombo::new(KeyCode::Backspace, KeyModifiers::ALT))),
        });
        self.register(Command {
            name: "yank",
            description: "Yank (paste) killed text",
            category: Category::Edit,
            keybinding: Some(Keybinding::Single(KeyCombo::ctrl('y'))),
        });
        self.register(Command {
            name: "undo",
            description: "Undo last edit",
            category: Category::Edit,
            keybinding: Some(Keybinding::Single(KeyCombo::ctrl('z'))),
        });
        self.register(Command {
            name: "redo",
            description: "Redo last undo",
            category: Category::Edit,
            keybinding: Some(Keybinding::Single(KeyCombo::alt('z'))),
        });
        self.register(Command {
            name: "insert-newline",
            description: "Insert a newline",
            category: Category::Input,
            keybinding: Some(Keybinding::Single(KeyCombo::special(KeyCode::Enter))),
        });
        self.register(Command {
            name: "delete-char",
            description: "Delete character at cursor",
            category: Category::Edit,
            keybinding: Some(Keybinding::Single(KeyCombo::special(KeyCode::Delete))),
        });
        self.register(Command {
            name: "delete-char-backward",
            description: "Delete character before cursor",
            category: Category::Edit,
            keybinding: Some(Keybinding::Single(KeyCombo::special(KeyCode::Backspace))),
        });
        self.register(Command {
            name: "insert-char",
            description: "Insert typed character",
            category: Category::Input,
            keybinding: None, // Dynamic - any character
        });
        self.register(Command {
            name: "insert-tab",
            description: "Insert tab character",
            category: Category::Input,
            keybinding: Some(Keybinding::Single(KeyCombo::special(KeyCode::Tab))),
        });

        // Selection commands
        self.register(Command {
            name: "set-mark",
            description: "Set or toggle the mark",
            category: Category::Selection,
            keybinding: Some(Keybinding::Single(KeyCombo::new(KeyCode::Char(' '), KeyModifiers::CONTROL))),
        });
        self.register(Command {
            name: "swap-cursor-mark",
            description: "Swap cursor and mark positions",
            category: Category::Selection,
            keybinding: Some(Keybinding::Sequence(vec![KeyCombo::ctrl('x'), KeyCombo::ctrl('x')])),
        });
        self.register(Command {
            name: "kill-region",
            description: "Kill (cut) selected region",
            category: Category::Selection,
            keybinding: Some(Keybinding::Single(KeyCombo::ctrl('w'))),
        });
        self.register(Command {
            name: "copy-region",
            description: "Copy selected region",
            category: Category::Selection,
            keybinding: Some(Keybinding::Single(KeyCombo::alt('w'))),
        });
        self.register(Command {
            name: "cancel-mark",
            description: "Cancel active selection",
            category: Category::Selection,
            keybinding: None, // Via C-g
        });

        // Display commands
        self.register(Command {
            name: "toggle-soft-wrap",
            description: "Toggle soft word wrap",
            category: Category::Edit,
            keybinding: Some(Keybinding::Sequence(vec![KeyCombo::ctrl('x'), KeyCombo::plain('w')])),
        });

        // System commands
        self.register(Command {
            name: "operate",
            description: "Open operation menu",
            category: Category::System,
            keybinding: Some(Keybinding::Single(KeyCombo::alt('q'))),
        });
        self.register(Command {
            name: "settings",
            description: "Open settings menu",
            category: Category::System,
            keybinding: Some(Keybinding::Single(KeyCombo::alt('?'))),
        });
        self.register(Command {
            name: "quit",
            description: "Quit/cancel current operation",
            category: Category::System,
            keybinding: Some(Keybinding::Single(KeyCombo::ctrl('g'))),
        });
        self.register(Command {
            name: "force-quit",
            description: "Force quit without saving",
            category: Category::System,
            keybinding: Some(Keybinding::Sequence(vec![KeyCombo::ctrl('x'), KeyCombo::ctrl('q')])),
        });
        self.register(Command {
            name: "switch-focus",
            description: "Switch focus to/from floating window",
            category: Category::System,
            keybinding: Some(Keybinding::Single(KeyCombo::special(KeyCode::Tab))),
        });
        self.register(Command {
            name: "execute-command",
            description: "Open command palette",
            category: Category::System,
            keybinding: Some(Keybinding::Single(KeyCombo::alt('x'))),
        });
    }

    fn register(&mut self, command: Command) {
        self.commands.insert(command.name, command);
    }

    /// Get all commands
    pub fn all_commands(&self) -> impl Iterator<Item = &Command> {
        self.commands.values()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keycombo_display() {
        assert_eq!(KeyCombo::ctrl('x').display(), "C-x");
        assert_eq!(KeyCombo::alt('f').display(), "M-f");
        assert_eq!(KeyCombo::plain('k').display(), "k");
    }

    #[test]
    fn test_registry_all_commands() {
        let registry = CommandRegistry::new();
        let commands: Vec<_> = registry.all_commands().collect();
        assert!(!commands.is_empty());
        // Verify some expected commands exist
        assert!(commands.iter().any(|c| c.name == "save-file"));
        assert!(commands.iter().any(|c| c.name == "open-file"));
    }

    #[test]
    fn test_ctrl_x_prefix() {
        let prefix = CtrlXPrefix;
        assert_eq!(prefix.display_name(), "C-x");

        let bindings = prefix.bindings();
        assert!(!bindings.is_empty());
    }
}
