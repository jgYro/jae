//! Core Editor struct and initialization.

use super::syntax::{HighlightResult, HighlightSpan, Language, SyntaxState};
use super::{MarkState, Settings, StatusBarState, UndoManager};
use crate::clipboard::ClipboardManager;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::{Color, Modifier, Style};
use std::path::PathBuf;
use tui_textarea::TextArea;

use super::FloatingWindow;

/// State for the C-l recenter cycling behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecenterState {
    /// Normal scrolling - keep cursor in view without forcing position
    #[default]
    Normal,
    /// Center the cursor line in the viewport
    Center,
    /// Put cursor line at top of viewport
    Top,
    /// Put cursor line at bottom of viewport
    Bottom,
}

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
    /// Unified syntax state for parsing and highlighting (None for plain text)
    pub syntax_state: Option<SyntaxState>,
    /// Cached highlight spans (updated on buffer change)
    pub cached_highlights: Vec<HighlightSpan>,
    /// Last syntax error message (if any)
    pub syntax_error: Option<String>,
    /// Current recenter state for C-l cycling
    pub recenter_state: RecenterState,
    /// Whether the last command was a recenter (for consecutive C-l detection)
    pub last_was_recenter: bool,
    /// Viewport height (updated each frame for page up/down calculations)
    pub viewport_height: u16,
    /// Current vertical scroll offset (persists across frames)
    pub scroll_offset: usize,
    /// Selection history for syntax-aware shrink (stores previous selections as byte ranges)
    pub selection_history: Vec<(usize, usize)>,
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
            syntax_state: None,
            cached_highlights: Vec::new(),
            syntax_error: None,
            recenter_state: RecenterState::default(),
            last_was_recenter: false,
            viewport_height: 24, // Default, updated each frame
            scroll_offset: 0,
            selection_history: Vec::new(),
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

    /// Ensure highlights are current before rendering.
    /// This performs lazy parsing only when needed.
    pub fn ensure_highlights_current(&mut self) {
        match &mut self.syntax_state {
            Some(state) => match state.cache_valid {
                false => {
                    let source = self.textarea.lines().join("\n");
                    let timeout = self.settings.parse_timeout_ms;

                    match state.parse_with_timeout(&source, timeout) {
                        true => {
                            // Parse succeeded, compute highlights
                            match state.compute_highlights(&source) {
                                HighlightResult::Success(spans) => {
                                    self.cached_highlights = spans;
                                    self.syntax_error = None;
                                }
                                HighlightResult::PartialSuccess { spans, error } => {
                                    self.cached_highlights = spans;
                                    self.syntax_error = Some(error);
                                }
                                HighlightResult::Failure(error) => {
                                    self.cached_highlights.clear();
                                    self.syntax_error = Some(error);
                                }
                            }
                            state.cache_valid = true;
                        }
                        false => {
                            // Parse timed out - keep old cached highlights
                            self.syntax_error = Some("Parse timeout".to_string());
                        }
                    }
                }
                true => {}
            },
            None => {}
        }
    }

    /// Update cached syntax highlights for the current buffer.
    /// This is the legacy method - prefer ensure_highlights_current() for lazy updates.
    pub fn update_highlights(&mut self) {
        match &mut self.syntax_state {
            Some(state) => {
                state.invalidate_cache();
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
        let end_byte = match end_char >= s.chars().count() {
            true => s.len(),
            false => Self::char_index_to_byte_index(s, end_char),
        };

        match start_byte <= end_byte && end_byte <= s.len() {
            true => s[start_byte..end_byte].to_string(),
            false => String::new(),
        }
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
