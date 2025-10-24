use crate::editor::Editor;
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
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

    let help_items = vec![
        ("C-a/e", "home/end"),
        ("C-f/b", "←→"),
        ("C-n", "newline"),
        ("M-f/b", "word"),
        ("C-SPC", "mark"),
        ("C-w", "kill"),
        ("M-w", "copy"),
        ("C-y", "yank"),
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

    let mut status_line = vec![mark_indicator];
    status_line.extend(help_spans);

    Paragraph::new(vec![Line::from(status_line)])
        .block(Block::default().borders(Borders::TOP))
}