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
                FloatingMode::Menu { state, .. } => {
                    use crate::editor::MenuItem;
                    // Create menu items
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
        ("M-q", "float"),
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