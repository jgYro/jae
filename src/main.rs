mod clipboard;
mod commands;
mod editor;
mod keybindings;
mod logging;
mod ui;

use clap::Parser;
use editor::Editor;
use keybindings::handle_input;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::Terminal;
use std::path::Path;

#[derive(Parser)]
#[command(name = "jae")]
#[command(about = "Just Another Editor - An Emacs-like terminal editor")]
struct Args {
    /// File to open or directory to browse
    path: Option<String>,

    /// Enable debug logging to debug.log
    #[arg(long)]
    log: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse arguments BEFORE terminal init
    let args = Args::parse();

    // Initialize logging if enabled
    if args.log {
        logging::init("debug.log")?;
        log::info!("JAE starting with logging enabled");
    }

    let mut terminal = ratatui::init();
    let mut editor = Editor::new();

    // Handle path argument
    match args.path {
        Some(path_str) => {
            let path = Path::new(&path_str);
            match (path.is_file(), path.is_dir()) {
                (true, _) => {
                    log::info!("Opening file: {}", path.display());
                    match editor.open_file(path) {
                        Ok(_) => {}
                        Err(e) => log::error!("Failed to open file: {}", e),
                    }
                }
                (_, true) => {
                    log::info!("Opening directory: {}", path.display());
                    editor.open_directory_prompt(path);
                }
                (false, false) => {
                    // Path doesn't exist - could be a new file
                    // Just set current_file so save will work
                    log::info!("New file: {}", path.display());
                    editor.current_file = Some(path.to_path_buf());
                }
            }
        }
        None => {}
    }

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

        match event::read()? {
            Event::Key(key) => {
                // Only handle key press events (Windows sends both Press and Release)
                match key.kind == KeyEventKind::Press && !handle_input(editor, key) {
                    true => break,
                    false => {}
                }
            }
            _ => {}
        }
    }

    Ok(())
}
