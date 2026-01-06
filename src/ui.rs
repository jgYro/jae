use crate::editor::text_widget::EditorWidget;
use crate::editor::{Editor, FloatingMode};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

pub fn draw(frame: &mut Frame, editor: &Editor) {
    // Status bar is always 3 lines: which-key line + status line + border
    let status_height = 3;

    let chunks = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(status_height),
    ])
    .split(frame.area());

    // Text area with syntax highlighting
    frame.render_widget(EditorWidget::new(editor), chunks[0]);

    // Status bar with optional which-key line above
    render_status_bar(frame, editor, chunks[1]);

    // Floating window
    if let Some(ref fw) = editor.floating_window {
        if fw.visible {
            let area = Rect::new(fw.x, fw.y, fw.width, fw.height);

            // Clear background first
            frame.render_widget(Clear, area);

            // Draw floating window with border
            let title: String = match &fw.mode {
                FloatingMode::Menu { state, .. } => {
                    if state.path.is_empty() {
                        "Menu - C-l:enter C-h:back ↑↓:nav Enter:select".to_string()
                    } else {
                        // Show breadcrumb path
                        format!("Menu [{}] - C-l:enter C-h:back", state.path.join(" > "))
                    }
                },
                FloatingMode::Settings { .. } => "Settings - ↑↓:nav Space:toggle C-h/C-l:adjust ESC:close".to_string(),
                FloatingMode::Minibuffer { .. } => "".to_string(),  // Minibuffer renders its own prompt
                FloatingMode::Confirm { .. } => "".to_string(),  // Confirm renders its own prompt
                FloatingMode::CommandPalette { .. } => "".to_string(),  // CommandPalette renders its own prompt
            };

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(if editor.focus_floating {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Gray)
                })
                .title(title);

            let inner_area = block.inner(area);
            frame.render_widget(block, area);

            // Render content based on mode
            match &fw.mode {
                FloatingMode::Menu { state, preview, metadata, .. } => {
                    use crate::editor::MenuItem;

                    // Split area for menu, preview, and metadata
                    let has_preview = preview.is_some();
                    let has_metadata = metadata.is_some();

                    let chunks = if has_preview || has_metadata {
                        let constraints = if has_preview && has_metadata {
                            vec![
                                Constraint::Percentage(50),  // Menu
                                Constraint::Percentage(30),  // Preview
                                Constraint::Percentage(20),  // Metadata
                            ]
                        } else if has_preview {
                            vec![
                                Constraint::Percentage(60),  // Menu
                                Constraint::Percentage(40),  // Preview
                            ]
                        } else {
                            vec![
                                Constraint::Percentage(70),  // Menu
                                Constraint::Percentage(30),  // Metadata
                            ]
                        };
                        Layout::vertical(constraints).split(inner_area)
                    } else {
                        Layout::vertical(vec![Constraint::Percentage(100)]).split(inner_area)
                    };

                    // Render menu items
                    let items: Vec<ListItem> = state.items
                        .iter()
                        .enumerate()
                        .map(|(i, item)| {
                            let (label, is_category) = match item {
                                MenuItem::Category(name, _) => (format!("📁 {}", name), true),
                                MenuItem::Action(_, label) => (label.clone(), false),
                            };

                            let content = if i == state.selected {
                                format!("→ {}", label)
                            } else {
                                format!("  {}", label)
                            };

                            let base_style = if is_category {
                                Style::default().fg(Color::Cyan)
                            } else {
                                Style::default()
                            };

                            ListItem::new(content).style(
                                if i == state.selected {
                                    base_style
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    base_style
                                }
                            )
                        })
                        .collect();

                    let list = List::new(items);
                    frame.render_widget(list, chunks[0]);

                    // Render preview if available
                    if let Some(preview_text) = preview {
                        if has_preview {
                            let preview_widget = Paragraph::new(preview_text.clone())
                                .block(Block::default()
                                    .borders(Borders::TOP)
                                    .title("Preview")
                                    .border_style(Style::default().fg(Color::DarkGray)));
                            frame.render_widget(preview_widget, chunks[1]);
                        }
                    }

                    // Render metadata if available
                    if let Some(metadata_text) = metadata {
                        let metadata_idx = if has_preview && has_metadata { 2 } else { 1 };
                        let metadata_widget = Paragraph::new(metadata_text.clone())
                            .block(Block::default()
                                .borders(Borders::TOP)
                                .title("Metadata")
                                .border_style(Style::default().fg(Color::DarkGray)));
                        frame.render_widget(metadata_widget, chunks[metadata_idx]);
                    }
                }
                FloatingMode::Settings { items, selected } => {
                    use crate::editor::SettingValue;

                    // Create setting items display
                    let display_items: Vec<ListItem> = items
                        .iter()
                        .enumerate()
                        .map(|(i, item)| {
                            let value_str = match &item.value {
                                SettingValue::Bool(b) => if *b { "[✓]".to_string() } else { "[ ]".to_string() },
                                SettingValue::Number(n) => format!("<{}>", n),
                                SettingValue::Choice { current, options } => {
                                    let opt = options.get(*current).map(|s| s.as_str()).unwrap_or("?");
                                    format!("[{}]", opt)
                                }
                            };

                            let content = if i == *selected {
                                format!("→ {} {}", item.name, value_str)
                            } else {
                                format!("  {} {}", item.name, value_str)
                            };

                            let mut spans = vec![Span::raw(content)];
                            if i == *selected {
                                spans.push(Span::styled(
                                    format!(" - {}", item.description),
                                    Style::default().fg(Color::DarkGray)
                                ));
                            }

                            ListItem::new(Line::from(spans)).style(
                                if i == *selected {
                                    Style::default()
                                        .fg(Color::Yellow)
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default()
                                }
                            )
                        })
                        .collect();

                    let list = List::new(display_items);
                    frame.render_widget(list, inner_area);
                }
                FloatingMode::Minibuffer {
                    prompt,
                    input,
                    cursor_pos,
                    completions,
                    selected_completion,
                    ..
                } => {
                    // For minibuffer, render at the bottom of the screen
                    let minibuffer_area = Rect::new(
                        0,
                        frame.area().height.saturating_sub(3),
                        frame.area().width,
                        3,
                    );

                    frame.render_widget(Clear, minibuffer_area);

                    // Build the input line with cursor and directory boundary indicator
                    // Find the last directory separator to show |/| marker
                    let chars: Vec<char> = input.chars().collect();

                    // Find last '/' position for directory boundary indicator
                    let last_slash_pos = input.rfind('/').map(|i| {
                        // Convert byte index to char index
                        input[..i].chars().count()
                    });

                    let mut spans = vec![
                        Span::styled(prompt.clone(), Style::default().fg(Color::Cyan)),
                    ];

                    // Build path with |/| indicator at directory boundary
                    if let Some(slash_pos) = last_slash_pos {
                        // Part before the slash
                        let before_slash: String = chars[..slash_pos].iter().collect();
                        spans.push(Span::raw(before_slash));

                        // The |/| indicator (highlighted)
                        spans.push(Span::styled(
                            "|/|",
                            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                        ));

                        // Part after the slash (the filename portion)
                        let after_slash_start = slash_pos + 1;
                        if after_slash_start < chars.len() {
                            // Handle cursor position within the filename portion
                            let relative_cursor = cursor_pos.saturating_sub(after_slash_start);
                            let filename_chars: Vec<char> = chars[after_slash_start..].to_vec();

                            if *cursor_pos >= after_slash_start && *cursor_pos <= chars.len() {
                                // Cursor is in the filename portion
                                let before_cursor: String = filename_chars[..relative_cursor].iter().collect();
                                let cursor_char = filename_chars.get(relative_cursor).unwrap_or(&' ');
                                let after_cursor: String = filename_chars[relative_cursor..].iter().skip(1).collect();

                                spans.push(Span::raw(before_cursor));
                                spans.push(Span::styled(
                                    cursor_char.to_string(),
                                    Style::default().bg(Color::White).fg(Color::Black),
                                ));
                                spans.push(Span::raw(after_cursor));
                            } else {
                                // Cursor is in the directory portion (shouldn't normally happen)
                                let filename: String = filename_chars.iter().collect();
                                spans.push(Span::raw(filename));
                            }
                        } else {
                            // Cursor is right after the slash
                            spans.push(Span::styled(
                                " ",
                                Style::default().bg(Color::White).fg(Color::Black),
                            ));
                        }
                    } else {
                        // No directory separator - simple cursor display
                        let before_cursor: String = chars[..*cursor_pos].iter().collect();
                        let cursor_char = chars.get(*cursor_pos).unwrap_or(&' ');
                        let after_cursor: String = chars[*cursor_pos..].iter().skip(1).collect();

                        spans.push(Span::raw(before_cursor));
                        spans.push(Span::styled(
                            cursor_char.to_string(),
                            Style::default().bg(Color::White).fg(Color::Black),
                        ));
                        spans.push(Span::raw(after_cursor));
                    }

                    let input_line = Line::from(spans);

                    // Show completions if any
                    let completion_hint = if !completions.is_empty() {
                        if let Some(idx) = selected_completion {
                            let comp = completions.get(*idx).cloned().unwrap_or_default();
                            format!(" [{}] ({}/{})", comp, idx + 1, completions.len())
                        } else {
                            format!(" ({} completions)", completions.len())
                        }
                    } else {
                        String::new()
                    };

                    let status_line = Line::from(vec![
                        Span::styled(
                            "Tab:enter dir ↑↓:navigate Enter:open/enter C-g:cancel",
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(completion_hint, Style::default().fg(Color::Yellow)),
                    ]);

                    let widget = Paragraph::new(vec![input_line, status_line])
                        .block(Block::default().borders(Borders::TOP));

                    frame.render_widget(widget, minibuffer_area);
                }

                FloatingMode::Confirm {
                    steps,
                    current_index,
                    ..
                } => {
                    // For confirm dialog, render at the bottom of the screen
                    let confirm_area = Rect::new(
                        0,
                        frame.area().height.saturating_sub(2),
                        frame.area().width,
                        2,
                    );

                    frame.render_widget(Clear, confirm_area);

                    if let Some(step) = steps.get(*current_index) {
                        use crate::editor::ResponseType;

                        let step_indicator = if steps.len() > 1 {
                            format!(" [{}/{}]", current_index + 1, steps.len())
                        } else {
                            String::new()
                        };

                        let prompt_line = match &step.response_type {
                            ResponseType::Binary => {
                                Line::from(vec![
                                    Span::styled(&step.prompt, Style::default().fg(Color::Yellow)),
                                    Span::styled(" (y/n)", Style::default().fg(Color::Cyan)),
                                    Span::styled(step_indicator, Style::default().fg(Color::DarkGray)),
                                ])
                            }
                            ResponseType::Choice(options) => {
                                let mut spans = vec![
                                    Span::styled(&step.prompt, Style::default().fg(Color::Yellow)),
                                    Span::raw(" ("),
                                ];
                                for (i, (key, label)) in options.iter().enumerate() {
                                    if i > 0 {
                                        spans.push(Span::raw("/"));
                                    }
                                    spans.push(Span::styled(
                                        key.to_string(),
                                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                                    ));
                                    spans.push(Span::raw(format!(":{}", label)));
                                }
                                spans.push(Span::raw(")"));
                                spans.push(Span::styled(step_indicator, Style::default().fg(Color::DarkGray)));
                                Line::from(spans)
                            }
                        };

                        let widget = Paragraph::new(vec![prompt_line])
                            .block(Block::default().borders(Borders::TOP));

                        frame.render_widget(widget, confirm_area);
                    }
                }

                FloatingMode::CommandPalette {
                    input,
                    cursor_pos,
                    filtered_commands,
                    selected,
                } => {
                    // Calculate height based on number of commands to show
                    let max_visible = 10;
                    let visible_count = filtered_commands.len().min(max_visible);
                    let palette_height = (visible_count + 3) as u16; // +3 for input line, borders

                    // Position at bottom of screen, full width
                    let palette_area = Rect::new(
                        0,
                        frame.area().height.saturating_sub(palette_height),
                        frame.area().width,
                        palette_height,
                    );

                    frame.render_widget(Clear, palette_area);

                    // Build input line with cursor
                    let chars: Vec<char> = input.chars().collect();
                    let before_cursor: String = chars[..*cursor_pos].iter().collect();
                    let cursor_char = chars.get(*cursor_pos).unwrap_or(&' ');
                    let after_cursor: String = chars[*cursor_pos..].iter().skip(1).collect();

                    let input_line = Line::from(vec![
                        Span::styled("M-x ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::raw(before_cursor),
                        Span::styled(
                            cursor_char.to_string(),
                            Style::default().bg(Color::White).fg(Color::Black),
                        ),
                        Span::raw(after_cursor),
                    ]);

                    // Build command list
                    let mut lines = vec![input_line];

                    // Calculate scroll offset to keep selection at top of visible window
                    let scroll_offset = if filtered_commands.len() <= max_visible {
                        0  // All items fit, no scrolling needed
                    } else {
                        // Keep selected at top, but don't scroll past end
                        (*selected).min(filtered_commands.len() - max_visible)
                    };

                    // Show filtered commands with selection highlight (scrolled view)
                    let end_idx = (scroll_offset + max_visible).min(filtered_commands.len());
                    for (i, cmd) in filtered_commands.iter().enumerate().skip(scroll_offset).take(max_visible) {
                        let is_selected = i == *selected;
                        let prefix = if is_selected { "→ " } else { "  " };

                        let mut cmd_spans = vec![
                            Span::styled(
                                prefix,
                                if is_selected {
                                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default()
                                },
                            ),
                            Span::styled(
                                cmd.name,
                                if is_selected {
                                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(Color::White)
                                },
                            ),
                        ];

                        // Show keybinding if available
                        if let Some(ref kb) = cmd.keybinding {
                            cmd_spans.push(Span::styled(
                                format!(" [{}]", kb),
                                Style::default().fg(Color::Cyan),
                            ));
                        }

                        // Show description
                        cmd_spans.push(Span::styled(
                            format!(" - {}", cmd.description),
                            Style::default().fg(Color::DarkGray),
                        ));

                        lines.push(Line::from(cmd_spans));
                    }

                    // Help line with position indicator
                    let position_info = if filtered_commands.len() > max_visible {
                        format!(" [{}-{}/{}]", scroll_offset + 1, end_idx, filtered_commands.len())
                    } else {
                        String::new()
                    };

                    let help_line = Line::from(vec![
                        Span::styled(
                            "↑↓:navigate Tab:complete Enter:execute C-g:cancel",
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            position_info,
                            Style::default().fg(Color::Yellow),
                        ),
                    ]);
                    lines.push(help_line);

                    let widget = Paragraph::new(lines)
                        .block(Block::default().borders(Borders::TOP).title("Command Palette"));

                    frame.render_widget(widget, palette_area);
                }
            }
        }
    }
}

/// Render the status bar (2 lines: which-key above, status below)
fn render_status_bar(frame: &mut Frame, editor: &Editor, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    // Line 1: Which-key hints (if expanded) or empty
    if editor.status_bar.expanded && !editor.status_bar.which_key_items.is_empty() {
        let prefix_name = editor.status_bar.prefix_display_name().unwrap_or("?");
        let items = &editor.status_bar.which_key_items;
        let page = editor.status_bar.which_key_page;

        // Calculate items per page based on terminal width
        // Estimate ~20 chars per item on average
        let available_width = area.width.saturating_sub(20) as usize; // Reserve space for prefix and page info
        let items_per_page = (available_width / 18).max(3); // At least 3 items per page

        let total_pages = editor.status_bar.which_key_total_pages(items_per_page);
        // Clamp page to valid range in case of mismatch
        let page = page.min(total_pages.saturating_sub(1));
        let start = (page * items_per_page).min(items.len());
        let end = (start + items_per_page).min(items.len());

        // Build which-key line
        let mut spans: Vec<Span> = Vec::new();

        // Prefix
        spans.push(Span::styled(
            format!("{}- ", prefix_name),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));

        // Items for current page
        for (i, item) in items[start..end].iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(
                item.key_display.clone(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(":", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                item.command_name.to_string(),
                Style::default().fg(Color::White),
            ));
        }

        // Page indicator if multiple pages
        if total_pages > 1 {
            spans.push(Span::styled(
                format!("  [{}/{}] M-</>", page + 1, total_pages),
                Style::default().fg(Color::DarkGray),
            ));
        }

        lines.push(Line::from(spans));
    } else {
        // No which-key - empty line
        lines.push(Line::from(""));
    }

    // Line 2: Status info
    let mut status_spans: Vec<Span> = Vec::new();

    // File indicator
    let modified_marker = if editor.modified { "*" } else { "" };
    let filename = editor.current_file
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| format!("{}{}", modified_marker, s))
        .unwrap_or_else(|| format!("{}[No File]", modified_marker));

    status_spans.push(Span::styled(
        filename,
        Style::default()
            .fg(if editor.modified { Color::Red } else { Color::Green })
            .add_modifier(Modifier::BOLD),
    ));

    // Mark indicator
    if editor.mark.is_active() {
        status_spans.push(Span::styled(
            " MARK",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    // Floating indicator
    if editor.floating_window.is_some() {
        status_spans.push(Span::styled(
            if editor.focus_floating { " FLOAT*" } else { " FLOAT" },
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));
    }

    // Help hint (different when which-key is active)
    if editor.status_bar.has_active_prefix() {
        status_spans.push(Span::styled(
            " | ESC cancel",
            Style::default().fg(Color::DarkGray),
        ));
    } else {
        status_spans.push(Span::styled(
            " | C-x C-t C-c prefix  M-x cmd  M-q menu  ESC quit",
            Style::default().fg(Color::DarkGray),
        ));
    }

    lines.push(Line::from(status_spans));

    let widget = Paragraph::new(lines)
        .block(Block::default().borders(Borders::TOP));

    frame.render_widget(widget, area);
}