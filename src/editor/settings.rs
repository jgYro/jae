//! Editor settings and configuration.
//!
//! This module contains the Settings struct and color management utilities.

use ratatui::style::Color;

/// Editor settings for appearance and behavior
pub struct Settings {
    pub show_metadata: bool,
    pub floating_window_width: u16,
    pub floating_window_height: u16,
    pub show_preview: bool,
    pub cursor_color: Color,
    pub selection_color: Color,
    /// Enable soft word wrapping (visual only, no actual line breaks)
    pub soft_wrap: bool,
    /// Character(s) shown at the start of wrapped lines (Helix-style)
    pub wrap_indicator: String,
    /// Color for the wrap indicator
    pub wrap_indicator_color: Color,
    /// Maximum time for syntax parsing in milliseconds (0 = no limit)
    pub parse_timeout_ms: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_metadata: true,
            floating_window_width: 60,
            floating_window_height: 20,
            show_preview: true,
            cursor_color: Color::Red,
            selection_color: Color::Magenta,
            soft_wrap: false,
            wrap_indicator: "↪ ".to_string(),
            wrap_indicator_color: Color::DarkGray,
            parse_timeout_ms: 100, // 100ms default timeout
        }
    }
}

/// Color utilities for settings
impl Settings {
    /// Map a color to its index for settings display
    pub fn get_color_index(&self, color: Color) -> usize {
        match color {
            Color::Red => 0,
            Color::Green => 1,
            Color::Yellow => 2,
            Color::Blue => 3,
            Color::Magenta => 4,
            Color::Cyan => 5,
            Color::White => 6,
            Color::LightBlue => 6, // Map LightBlue to last index for selection
            _ => 0,
        }
    }

    /// Map an index to a color
    pub fn index_to_color(&self, index: usize, for_selection: bool) -> Color {
        if for_selection {
            match index {
                0 => Color::Red,
                1 => Color::Green,
                2 => Color::Yellow,
                3 => Color::Blue,
                4 => Color::Magenta,
                5 => Color::Cyan,
                6 => Color::LightBlue,
                _ => Color::Magenta,
            }
        } else {
            match index {
                0 => Color::Red,
                1 => Color::Green,
                2 => Color::Yellow,
                3 => Color::Blue,
                4 => Color::Magenta,
                5 => Color::Cyan,
                6 => Color::White,
                _ => Color::Red,
            }
        }
    }
}
