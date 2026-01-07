//! Custom text widget with syntax highlighting support.
//!
//! This widget renders text with syntax highlighting while preserving
//! selection and cursor rendering from the underlying textarea.

use super::Editor;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

/// Widget for rendering the editor with syntax highlighting.
pub struct EditorWidget<'a> {
    editor: &'a Editor,
}

impl<'a> EditorWidget<'a> {
    pub fn new(editor: &'a Editor) -> Self {
        Self { editor }
    }
}

impl Widget for EditorWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.editor.textarea.lines();
        if lines.is_empty() {
            return;
        }

        // Get viewport info from textarea
        let (cursor_row, cursor_col) = self.editor.textarea.cursor();

        // Calculate scroll offset based on cursor position
        let viewport_height = area.height as usize;
        let scroll_offset = if cursor_row >= viewport_height {
            cursor_row - viewport_height + 1
        } else {
            0
        };

        // Get the full source for byte offset calculations
        let source = lines.join("\n");

        // Use cached highlight spans
        let highlight_spans = &self.editor.cached_highlights;

        // Get selection range if any
        let selection_range = self.editor.textarea.selection_range();

        // Build a map of byte offset -> style for quick lookup
        let mut byte_styles: Vec<Style> = vec![Style::default(); source.len() + 1];
        for span in highlight_spans {
            for i in span.start..span.end.min(byte_styles.len()) {
                byte_styles[i] = span.style;
            }
        }

        // Track current byte offset as we render
        let mut current_byte = 0;

        // Skip to scroll offset
        for i in 0..scroll_offset {
            if i < lines.len() {
                current_byte += lines[i].len() + 1; // +1 for newline
            }
        }

        // Render visible lines
        for (screen_row, line_idx) in (scroll_offset..).take(viewport_height).enumerate() {
            if line_idx >= lines.len() {
                break;
            }

            let line = &lines[line_idx];
            let y = area.y + screen_row as u16;

            // Calculate line start byte
            let line_start_byte = current_byte;

            // Render each character in the line
            let mut x = area.x;
            for (col, ch) in line.chars().enumerate() {
                if x >= area.x + area.width {
                    break;
                }

                // Calculate byte offset for this character
                let byte_offset = line_start_byte + line.chars().take(col).map(|c| c.len_utf8()).sum::<usize>();

                // Get base style from syntax highlighting
                let mut style = byte_styles.get(byte_offset).copied().unwrap_or_default();

                // Check if this position is in the selection (overlay on top)
                match selection_range {
                    Some((sel_start, sel_end)) => {
                        let pos = (line_idx, col);
                        let in_selection = match sel_start <= sel_end {
                            true => pos >= sel_start && pos < sel_end,
                            false => pos >= sel_end && pos < sel_start,
                        };

                        match in_selection {
                            true => {
                                // Apply selection style as overlay (Helix approach)
                                style = style.patch(
                                    Style::default()
                                        .bg(self.editor.settings.selection_color)
                                        .fg(Color::White),
                                );
                            }
                            false => {}
                        }
                    }
                    None => {}
                }

                // Check if this is the cursor position
                match line_idx == cursor_row && col == cursor_col {
                    true => {
                        style = Style::default()
                            .bg(self.editor.settings.cursor_color)
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD);
                    }
                    false => {}
                }

                match buf.cell_mut((x, y)) {
                    Some(cell) => {
                        cell.set_char(ch).set_style(style);
                    }
                    None => {}
                }
                x += 1;
            }

            // Handle cursor at end of line (clippy: collapsed if statements)
            match line_idx == cursor_row && cursor_col >= line.chars().count() && x < area.x + area.width {
                true => match buf.cell_mut((x, y)) {
                    Some(cell) => {
                        cell.set_char(' ').set_style(
                            Style::default()
                                .bg(self.editor.settings.cursor_color)
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        );
                    }
                    None => {}
                },
                false => {}
            }

            // Selection highlight for end of line (the newline character)
            match selection_range {
                Some((sel_start, sel_end)) => {
                    let line_end_pos = (line_idx, line.chars().count());
                    let in_selection = match sel_start <= sel_end {
                        true => line_end_pos >= sel_start && line_end_pos < sel_end,
                        false => line_end_pos >= sel_end && line_end_pos < sel_start,
                    };

                    match in_selection && line_idx < lines.len() - 1 {
                        true => {
                            // Highlight the "newline" position
                            let end_x = area.x + line.chars().count() as u16;
                            match end_x < area.x + area.width && !(line_idx == cursor_row && cursor_col >= line.chars().count()) {
                                true => match buf.cell_mut((end_x, y)) {
                                    Some(cell) => {
                                        cell.set_char(' ').set_style(
                                            Style::default()
                                                .bg(self.editor.settings.selection_color)
                                                .fg(Color::White),
                                        );
                                    }
                                    None => {}
                                },
                                false => {}
                            }
                        }
                        false => {}
                    }
                }
                None => {}
            }

            // Update byte offset for next line (including newline)
            current_byte += line.len() + 1;
        }

        // Handle cursor on empty document
        match lines.len() == 1 && lines[0].is_empty() && cursor_row == 0 && cursor_col == 0 {
            true => match buf.cell_mut((area.x, area.y)) {
                Some(cell) => {
                    cell.set_char(' ').set_style(
                        Style::default()
                            .bg(self.editor.settings.cursor_color)
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    );
                }
                None => {}
            },
            false => {}
        }
    }
}
