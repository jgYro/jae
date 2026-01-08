//! Custom text widget with syntax highlighting support.
//!
//! This widget renders text with syntax highlighting while preserving
//! selection and cursor rendering from the underlying textarea.
//! Supports both horizontal scrolling and soft word wrapping (Helix-style).

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

/// Represents a visual line segment from a document line
struct VisualLine<'a> {
    /// The document line index
    doc_line: usize,
    /// Start column in the document line
    start_col: usize,
    /// The text content for this visual line
    text: &'a str,
    /// Whether this is a wrapped continuation (needs wrap indicator)
    is_wrapped: bool,
}

impl Widget for EditorWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.editor.textarea.lines();
        if lines.is_empty() {
            return;
        }

        let (cursor_row, cursor_col) = self.editor.textarea.cursor();
        let viewport_width = area.width as usize;
        let viewport_height = area.height as usize;

        // Choose rendering mode based on soft_wrap setting
        match self.editor.settings.soft_wrap {
            true => self.render_with_soft_wrap(area, buf, &lines, cursor_row, cursor_col, viewport_width, viewport_height),
            false => self.render_with_h_scroll(area, buf, &lines, cursor_row, cursor_col, viewport_width, viewport_height),
        }
    }
}

impl EditorWidget<'_> {
    /// Get the scroll offset from the editor (managed by movement code)
    fn get_scroll_offset(&self) -> usize {
        self.editor.scroll_offset
    }

    /// Render with horizontal scrolling (original behavior)
    fn render_with_h_scroll(
        &self,
        area: Rect,
        buf: &mut Buffer,
        lines: &[String],
        cursor_row: usize,
        cursor_col: usize,
        viewport_width: usize,
        viewport_height: usize,
    ) {
        // Calculate vertical scroll offset based on cursor position and recenter state
        let scroll_offset = self.get_scroll_offset();

        // Calculate horizontal scroll offset to keep cursor in view
        let h_scroll_offset = if cursor_col >= viewport_width {
            let margin = viewport_width / 4;
            cursor_col.saturating_sub(viewport_width.saturating_sub(margin))
        } else {
            0
        };

        let source = lines.join("\n");
        let highlight_spans = &self.editor.cached_highlights;
        let selection_range = self.editor.textarea.selection_range();

        // Build a map of byte offset -> style for quick lookup
        let mut byte_styles: Vec<Style> = vec![Style::default(); source.len() + 1];
        for span in highlight_spans {
            for i in span.start..span.end.min(byte_styles.len()) {
                byte_styles[i] = span.style;
            }
        }

        let mut current_byte = 0;

        // Skip to scroll offset
        for i in 0..scroll_offset {
            if i < lines.len() {
                current_byte += lines[i].len() + 1;
            }
        }

        // Render visible lines
        for (screen_row, line_idx) in (scroll_offset..).take(viewport_height).enumerate() {
            if line_idx >= lines.len() {
                break;
            }

            let line = &lines[line_idx];
            let y = area.y + screen_row as u16;
            let line_start_byte = current_byte;

            let mut x = area.x;
            // Track byte offset incrementally instead of O(n²) recalculation
            let mut byte_offset = line_start_byte;
            for (col, ch) in line.chars().enumerate() {
                if col < h_scroll_offset {
                    byte_offset += ch.len_utf8();
                    continue;
                }

                if x >= area.x + area.width {
                    break;
                }

                let mut style = byte_styles.get(byte_offset).copied().unwrap_or_default();

                // Selection overlay
                match selection_range {
                    Some((sel_start, sel_end)) => {
                        let pos = (line_idx, col);
                        let in_selection = match sel_start <= sel_end {
                            true => pos >= sel_start && pos < sel_end,
                            false => pos >= sel_end && pos < sel_start,
                        };
                        match in_selection {
                            true => {
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

                // Cursor
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
                byte_offset += ch.len_utf8();
            }

            // Handle cursor at end of line
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

            // Selection highlight for end of line
            match selection_range {
                Some((sel_start, sel_end)) => {
                    let line_end_col = line.chars().count();
                    let line_end_pos = (line_idx, line_end_col);
                    let in_selection = match sel_start <= sel_end {
                        true => line_end_pos >= sel_start && line_end_pos < sel_end,
                        false => line_end_pos >= sel_end && line_end_pos < sel_start,
                    };

                    match in_selection && line_idx < lines.len() - 1 && line_end_col >= h_scroll_offset {
                        true => {
                            let end_x = area.x + (line_end_col - h_scroll_offset) as u16;
                            match end_x < area.x + area.width && !(line_idx == cursor_row && cursor_col >= line_end_col) {
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

    /// Render with soft word wrapping (Helix-style)
    fn render_with_soft_wrap(
        &self,
        area: Rect,
        buf: &mut Buffer,
        lines: &[String],
        cursor_row: usize,
        cursor_col: usize,
        viewport_width: usize,
        viewport_height: usize,
    ) {
        let wrap_indicator = &self.editor.settings.wrap_indicator;
        let wrap_indicator_len = wrap_indicator.chars().count();
        // Effective width for text (accounting for wrap indicator on continuation lines)
        let text_width = viewport_width.saturating_sub(wrap_indicator_len);

        if text_width == 0 {
            return;
        }

        let source = lines.join("\n");
        let highlight_spans = &self.editor.cached_highlights;
        let selection_range = self.editor.textarea.selection_range();

        // Build byte offset -> style map
        let mut byte_styles: Vec<Style> = vec![Style::default(); source.len() + 1];
        for span in highlight_spans {
            for i in span.start..span.end.min(byte_styles.len()) {
                byte_styles[i] = span.style;
            }
        }

        // Build visual lines and find cursor's visual position
        let mut visual_lines: Vec<VisualLine> = Vec::new();
        let mut cursor_visual_row = 0;
        let mut cursor_visual_col = 0;

        for (doc_line_idx, line) in lines.iter().enumerate() {
            let line_chars: Vec<char> = line.chars().collect();
            let line_len = line_chars.len();

            if line_len == 0 {
                // Empty line
                if doc_line_idx == cursor_row {
                    cursor_visual_row = visual_lines.len();
                    cursor_visual_col = 0;
                }
                visual_lines.push(VisualLine {
                    doc_line: doc_line_idx,
                    start_col: 0,
                    text: "",
                    is_wrapped: false,
                });
            } else {
                // Split line into visual segments
                let mut col = 0;
                let mut is_first_segment = true;

                while col < line_len {
                    let available_width = match is_first_segment {
                        true => viewport_width,
                        false => text_width,
                    };

                    let end_col = (col + available_width).min(line_len);

                    // Track cursor position
                    if doc_line_idx == cursor_row && cursor_col >= col && cursor_col < end_col {
                        cursor_visual_row = visual_lines.len();
                        cursor_visual_col = match is_first_segment {
                            true => cursor_col - col,
                            false => wrap_indicator_len + (cursor_col - col),
                        };
                    } else if doc_line_idx == cursor_row && cursor_col >= end_col && end_col == line_len {
                        // Cursor at end of line
                        cursor_visual_row = visual_lines.len();
                        cursor_visual_col = match is_first_segment {
                            true => end_col - col,
                            false => wrap_indicator_len + (end_col - col),
                        };
                    }

                    // Get the byte slice for this segment
                    let start_byte_idx: usize = line_chars[..col].iter().map(|c| c.len_utf8()).sum();
                    let end_byte_idx: usize = line_chars[..end_col].iter().map(|c| c.len_utf8()).sum();
                    let segment_text = &line[start_byte_idx..end_byte_idx];

                    visual_lines.push(VisualLine {
                        doc_line: doc_line_idx,
                        start_col: col,
                        text: segment_text,
                        is_wrapped: !is_first_segment,
                    });

                    col = end_col;
                    is_first_segment = false;
                }
            }
        }

        // For soft wrap, we use visual rows but the scroll_offset is managed elsewhere
        // This is a simplification - soft wrap may need refinement
        let scroll_offset = self.get_scroll_offset();

        // Track which document line we're processing for byte offset calculations
        let mut last_doc_line: Option<usize> = None;
        let mut line_byte_offset: usize = 0;

        for (visual_row, visual_line) in visual_lines.iter().enumerate().skip(scroll_offset).take(viewport_height) {
            let screen_row = (visual_row - scroll_offset) as u16;
            let y = area.y + screen_row;

            // Update byte offset when we move to a new document line
            match last_doc_line {
                Some(last) if last != visual_line.doc_line => {
                    line_byte_offset = lines.iter().take(visual_line.doc_line).map(|l| l.len() + 1).sum();
                }
                None => {
                    line_byte_offset = lines.iter().take(visual_line.doc_line).map(|l| l.len() + 1).sum();
                }
                Some(_) => {}
            }
            last_doc_line = Some(visual_line.doc_line);

            let mut x = area.x;

            // Draw wrap indicator for continuation lines
            match visual_line.is_wrapped {
                true => {
                    let wrap_style = Style::default().fg(self.editor.settings.wrap_indicator_color);
                    for ch in wrap_indicator.chars() {
                        match buf.cell_mut((x, y)) {
                            Some(cell) => {
                                cell.set_char(ch).set_style(wrap_style);
                            }
                            None => {}
                        }
                        x += 1;
                    }
                }
                false => {}
            }

            // Draw the text content
            // Precompute segment start byte offset instead of O(n²) recalculation per char
            let segment_start_byte: usize = line_byte_offset
                + lines[visual_line.doc_line]
                    .chars()
                    .take(visual_line.start_col)
                    .map(|c| c.len_utf8())
                    .sum::<usize>();
            let mut char_byte_offset = segment_start_byte;

            for (local_col, ch) in visual_line.text.chars().enumerate() {
                if x >= area.x + area.width {
                    break;
                }

                let doc_col = visual_line.start_col + local_col;
                let mut style = byte_styles.get(char_byte_offset).copied().unwrap_or_default();

                // Selection overlay
                match selection_range {
                    Some((sel_start, sel_end)) => {
                        let pos = (visual_line.doc_line, doc_col);
                        let in_selection = match sel_start <= sel_end {
                            true => pos >= sel_start && pos < sel_end,
                            false => pos >= sel_end && pos < sel_start,
                        };
                        match in_selection {
                            true => {
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

                // Cursor
                let visual_col = match visual_line.is_wrapped {
                    true => wrap_indicator_len + local_col,
                    false => local_col,
                };
                match visual_row == cursor_visual_row && visual_col == cursor_visual_col {
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
                char_byte_offset += ch.len_utf8();
            }

            // Handle cursor at end of visual line (only if it's the last segment of the doc line)
            let is_last_segment_of_line = visual_lines
                .get(visual_row + 1)
                .map(|next| next.doc_line != visual_line.doc_line)
                .unwrap_or(true);

            match is_last_segment_of_line
                && visual_row == cursor_visual_row
                && cursor_col >= visual_line.start_col + visual_line.text.chars().count()
                && x < area.x + area.width
            {
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
