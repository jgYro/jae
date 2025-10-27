use crate::editor::{Editor, FloatingMode};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

pub fn draw(frame: &mut Frame, editor: &Editor) {
    let chunks = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .split(frame.area());

    // Text area
    frame.render_widget(&editor.textarea, chunks[0]);

    // Status bar
    frame.render_widget(create_status_bar(editor), chunks[1]);

    // Floating window
    if let Some(ref fw) = editor.floating_window {
        if fw.visible {
            let area = Rect::new(fw.x, fw.y, fw.width, fw.height);

            // Clear background first
            frame.render_widget(Clear, area);

            // Draw floating window with border
            let title = match &fw.mode {
                FloatingMode::Menu { state, .. } => {
                    if state.path.is_empty() {
                        "Menu - C-l:enter C-h:back ↑↓:nav Enter:select"
                    } else {
                        // Show breadcrumb path
                        &format!("Menu [{}] - C-l:enter C-h:back", state.path.join(" > "))
                    }
                },
                FloatingMode::Settings { .. } => "Settings - ↑↓:nav Space:toggle C-h/C-l:adjust ESC:close",
                FloatingMode::TextEdit => "Floating",
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
                        if has_preview && has_metadata {
                            let preview_widget = Paragraph::new(preview_text.clone())
                                .block(Block::default()
                                    .borders(Borders::TOP)
                                    .title("Preview")
                                    .border_style(Style::default().fg(Color::DarkGray)));
                            frame.render_widget(preview_widget, chunks[1]);
                        } else if has_preview {
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
                                SettingValue::Bool(b) => if *b { "[✓]" } else { "[ ]" },
                                SettingValue::Number(n) => &format!("<{}>", n),
                                SettingValue::Choice { current, options } => {
                                    &format!("[{}]", options.get(*current).unwrap_or(&String::new()))
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
                FloatingMode::TextEdit => {
                    frame.render_widget(&fw.textarea, inner_area);
                }
            }
        }
    }
}

fn create_status_bar(editor: &Editor) -> Paragraph<'static> {
    let mark_indicator = if editor.mark_active {
        Span::styled(
            "MARK ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };

    let floating_indicator = if editor.floating_window.is_some() {
        Span::styled(
            if editor.focus_floating {
                "FLOAT* "
            } else {
                "FLOAT "
            },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw("")
    };

    let mut help_items = vec![
        ("C-a/e", "home/end"),
        ("C-f/b", "←→"),
        ("C-n/p", "↑↓"),
        ("M-f/b", "word"),
        ("C-SPC", "mark"),
        ("C-w", "kill"),
        ("M-w", "copy"),
        ("C-y", "yank"),
        ("M-q", "menu"),
        ("M-?", "settings"),
        ("C-g/ESC", "quit"),
    ];

    // Show focus switching help when floating window is open
    if editor.floating_window.is_some() {
        help_items.insert(9, ("Tab/S-Tab", "focus"));
    }

    let mut help_spans = Vec::new();
    for (i, (key, desc)) in help_items.iter().enumerate() {
        if i > 0 {
            help_spans.push(Span::raw(" "));
        }
        help_spans.push(Span::styled(
            *key,
            Style::default().fg(Color::Cyan),
        ));
        help_spans.push(Span::raw(":"));
        help_spans.push(Span::styled(
            *desc,
            Style::default().fg(Color::DarkGray),
        ));
    }

    let mut status_line = vec![mark_indicator, floating_indicator];
    status_line.extend(help_spans);

    Paragraph::new(vec![Line::from(status_line)])
        .block(Block::default().borders(Borders::TOP))
}