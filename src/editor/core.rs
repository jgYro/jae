//! Core Editor struct and initialization.

use super::syntax::{HighlightSpan, Language, Syntax, SyntaxHighlighter};
use super::{MarkState, Settings, StatusBarState, UndoManager};
use crate::clipboard::ClipboardManager;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::{Color, Modifier, Style};
use std::path::PathBuf;
use tui_textarea::TextArea;

use super::FloatingWindow;

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
    /// Detected language for syntax operations
    pub language: Language,
    /// Tree-sitter syntax state (None for plain text)
    pub syntax: Option<Syntax>,
    /// Syntax highlighter (None for plain text)
    pub highlighter: Option<SyntaxHighlighter>,
    /// Cached highlight spans (updated on buffer change)
    pub cached_highlights: Vec<HighlightSpan>,
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
            language: Language::PlainText,
            syntax: None,
            highlighter: None,
            cached_highlights: Vec::new(),
        }
    }

    /// Toggle soft word wrap mode
    pub fn toggle_soft_wrap(&mut self) {
        self.settings.soft_wrap = !self.settings.soft_wrap;
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

    /// Update cached syntax highlights for the current buffer.
    pub fn update_highlights(&mut self) {
        match &mut self.highlighter {
            Some(highlighter) => {
                let source = self.textarea.lines().join("\n");
                self.cached_highlights = highlighter.highlight(&source);
            }
            None => self.cached_highlights.clear(),
        }
    }

    // ==================== String Utilities ====================

    pub(crate) fn char_index_to_byte_index(s: &str, char_idx: usize) -> usize {
        s.char_indices()
            .nth(char_idx)
            .map(|(byte_idx, _)| byte_idx)
            .unwrap_or(s.len())
    }

    pub(crate) fn safe_string_slice(s: &str, start_char: usize, end_char: usize) -> String {
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
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
