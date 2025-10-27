use crate::editor::Editor;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::{CursorMove, Input};

pub fn handle_input(editor: &mut Editor, key: KeyEvent) -> bool {
    // Check for quit commands first
    if should_quit(editor, &key) {
        return false;
    }

    // If floating window is focused, handle input specially
    if editor.focus_floating && editor.floating_window.is_some() {
        return handle_floating_input(editor, key);
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
            if editor.is_at_last_line() {
                // At the end of document, insert a newline
                editor.textarea.move_cursor(CursorMove::End);
                editor.textarea.insert_newline();
                editor.reset_kill_sequence();
            } else {
                // Normal case: just move down
                editor.move_cursor(CursorMove::Down);
            }
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

        // Floating window
        (KeyCode::Char('q'), KeyModifiers::ALT) => {
            editor.toggle_floating_window();
        }
        // Switch focus to floating window with Shift+Tab
        (KeyCode::BackTab, _) => {
            if editor.floating_window.is_some() && !editor.focus_floating {
                editor.focus_floating = true;
            }
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

fn handle_floating_input(editor: &mut Editor, key: KeyEvent) -> bool {
    if let Some(ref mut fw) = editor.floating_window {
        match &mut fw.mode {
            crate::editor::FloatingMode::Menu { state, root_items } => {
                // Menu mode navigation
                match (key.code, key.modifiers) {
                    (KeyCode::Esc, _) | (KeyCode::Char('q'), KeyModifiers::ALT) => {
                        editor.floating_window = None;
                        editor.focus_floating = false;
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                        if state.selected > 0 {
                            state.selected -= 1;
                        }
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                        if state.selected < state.items.len() - 1 {
                            state.selected += 1;
                        }
                    }
                    // Enter category with C-l
                    (KeyCode::Char('l'), KeyModifiers::CONTROL) | (KeyCode::Enter, _) => {
                        match state.items.get(state.selected).cloned() {
                            Some(crate::editor::MenuItem::Category(_, _)) => {
                                state.enter_category();
                            }
                            Some(crate::editor::MenuItem::Action(action, _)) => {
                                // Need to temporarily drop the borrow of fw
                                let action_to_apply = action;
                                editor.floating_window = None;
                                editor.focus_floating = false;
                                editor.apply_menu_option(action_to_apply);
                                return true;
                            }
                            None => {}
                        }
                    }
                    // Go back with C-h
                    (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                        state.go_back(root_items);
                    }
                    (KeyCode::Tab, _) => {
                        editor.focus_floating = false;
                    }
                    _ => {}
                }
            }
            crate::editor::FloatingMode::TextEdit => {
                // Text edit mode
                match (key.code, key.modifiers) {
                    // Close floating window with ESC or M-q
                    (KeyCode::Esc, _) | (KeyCode::Char('q'), KeyModifiers::ALT) => {
                        editor.floating_window = None;
                        editor.focus_floating = false;
                    }

                    // Switch focus with Tab
                    (KeyCode::Tab, _) => {
                        editor.focus_floating = false;
                    }

                    // Basic movement commands for floating window
                    (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                        fw.textarea.move_cursor(CursorMove::Forward);
                    }
                    (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                        fw.textarea.move_cursor(CursorMove::Back);
                    }
                    (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                        fw.textarea.move_cursor(CursorMove::Down);
                    }
                    (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                        fw.textarea.move_cursor(CursorMove::Up);
                    }
                    (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                        fw.textarea.move_cursor(CursorMove::Head);
                    }
                    (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                        fw.textarea.move_cursor(CursorMove::End);
                    }

                    // Default: pass through to floating window textarea
                    _ => {
                        let event = ratatui::crossterm::event::Event::Key(key);
                        let input: Input = event.into();
                        fw.textarea.input(input);
                    }
                }
            }
        }
    }

    true
}

fn should_quit(editor: &mut Editor, key: &KeyEvent) -> bool {
    // Close floating window first if it's open
    if editor.floating_window.is_some() {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                editor.floating_window = None;
                editor.focus_floating = false;
                return false; // Don't quit main editor, just close floating
            }
            _ => {}
        }
    }

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