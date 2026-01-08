use clap::Parser;
use jae::editor::Editor;
use jae::keybindings::{check_jump_mode_timeout, handle_input};
use jae::logging;
use jae::ui;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::Terminal;
use std::path::Path;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "jae")]
#[command(about = "Just Another Editor - An Emacs-like terminal editor")]
struct Args {
    /// File to open or directory to browse
    path: Option<String>,

    /// Enable debug logging to debug.log
    #[arg(long)]
    log: bool,

    /// Log selection state changes (requires --log)
    #[arg(long)]
    selection: bool,

    /// Log cursor movement (requires --log)
    #[arg(long)]
    movement: bool,

    /// Log all key inputs (requires --log)
    #[arg(long)]
    keys: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse arguments BEFORE terminal init
    let args = Args::parse();

    // Initialize logging if enabled
    if args.log {
        logging::init("debug.log")?;
        logging::configure(args.selection, args.movement, args.keys);
        log::info!("JAE starting with logging enabled");
        if args.selection {
            log::info!("Selection logging enabled");
        }
        if args.movement {
            log::info!("Movement logging enabled");
        }
        if args.keys {
            log::info!("Key input logging enabled");
        }
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
        // Lazy parse/highlight before render (only when cache is invalid)
        editor.ensure_highlights_current();
        terminal.draw(|frame| ui::draw(frame, editor))?;

        // Use poll with timeout to support jump mode timeout detection
        // When jump mode is active, use short timeout; otherwise use longer timeout
        let poll_timeout = match editor.jump_mode.is_some() {
            true => Duration::from_millis(50),
            false => Duration::from_millis(500),
        };

        match event::poll(poll_timeout)? {
            true => {
                // Event available, read it
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
            false => {
                // Timeout - check jump mode timeout
                check_jump_mode_timeout(editor);
            }
        }
    }

    Ok(())
}
