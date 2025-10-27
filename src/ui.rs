use crate::editor::Editor;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
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
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(if editor.focus_floating {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Gray)
                })
                .title("Floating");

            let inner_area = block.inner(area);
            frame.render_widget(block, area);
            frame.render_widget(&fw.textarea, inner_area);
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

    let help_items = vec![
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