use crate::editor::Editor;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::{CursorMove, Input};

pub fn handle_input(editor: &mut Editor, key: KeyEvent) -> bool {
    // Check for quit commands first
    if should_quit(editor, &key) {
        return false;
    }

    // Handle Emacs keybindings
    match (key.code, key.modifiers) {
        // Basic movement - these don't reset kill sequence
        (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
            editor.move_cursor(CursorMove::Forward);
        }
        (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
            editor.move_cursor(CursorMove::Back);
        }
        (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            editor.textarea.insert_newline();
            editor.reset_kill_sequence(); // This does reset because it modifies text
        }
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            editor.move_cursor(CursorMove::Up);
        }
        (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
            editor.move_cursor(CursorMove::Head);
        }
        (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
            editor.move_cursor(CursorMove::End);
        }

        // Word movement - these don't reset kill sequence
        (KeyCode::Char('f'), KeyModifiers::ALT) => {
            editor.move_word_forward();
        }
        (KeyCode::Char('b'), KeyModifiers::ALT) => {
            editor.move_word_backward();
        }

        // Selection and mark - doesn't reset kill sequence
        (KeyCode::Char(' '), KeyModifiers::CONTROL) => {
            editor.set_mark();
        }

        // Kill and yank operations
        (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
            editor.kill_region();
        }
        (KeyCode::Char('w'), KeyModifiers::ALT) => {
            editor.copy_region();
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
            editor.yank();
        }
        (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
            editor.kill_to_end_of_line();
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            editor.kill_to_beginning_of_line();
        }

        // Default: pass through to textarea
        _ => {
            let event = ratatui::crossterm::event::Event::Key(key);
            let input: Input = event.into();
            editor.textarea.input(input);
            editor.reset_kill_sequence();
        }
    }

    true
}

fn should_quit(editor: &mut Editor, key: &KeyEvent) -> bool {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => {
            if editor.mark_active {
                editor.cancel_mark();
                false
            } else {
                true
            }
        }
        (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
            if editor.mark_active {
                editor.cancel_mark();
                false
            } else {
                true
            }
        }
        _ => false,
    }
}