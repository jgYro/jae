mod editor;
mod keybindings;
mod kill_ring;
mod ui;

use editor::Editor;
use keybindings::handle_input;
use ratatui::{crossterm::event::{self, Event}, Terminal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();
    let mut editor = Editor::new();

    let result = run_app(&mut terminal, &mut editor);

    ratatui::restore();
    result
}

fn run_app(
    terminal: &mut Terminal<impl ratatui::backend::Backend>,
    editor: &mut Editor,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|frame| ui::draw(frame, editor))?;

        if let Event::Key(key) = event::read()? {
            if !handle_input(editor, key) {
                break;
            }
        }
    }

    Ok(())
}