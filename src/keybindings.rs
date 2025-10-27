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

    // Handle C-x C-x sequence
    if editor.last_key == Some((KeyCode::Char('x'), KeyModifiers::CONTROL)) {
        if matches!((key.code, key.modifiers), (KeyCode::Char('x'), KeyModifiers::CONTROL)) {
            editor.swap_cursor_mark();
            editor.last_key = None;
            return true;
        }
    }

    // Clear last_key for any key that isn't C-SPC or C-x
    if !matches!((key.code, key.modifiers), (KeyCode::Char(' '), KeyModifiers::CONTROL))
        && !matches!((key.code, key.modifiers), (KeyCode::Char('x'), KeyModifiers::CONTROL)) {
        editor.last_key = None;
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
        // Settings menu
        (KeyCode::Char('?'), KeyModifiers::ALT) => {
            editor.open_settings_menu();
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
            // Store this key for detecting C-SPC C-SPC
            editor.last_key = Some((KeyCode::Char(' '), KeyModifiers::CONTROL));
        }
        // C-x prefix for commands
        (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
            // Store for potential C-x sequences
            editor.last_key = Some((KeyCode::Char('x'), KeyModifiers::CONTROL));
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
            crate::editor::FloatingMode::Menu { state, root_items, .. } => {
                // Menu mode navigation
                match (key.code, key.modifiers) {
                    (KeyCode::Esc, _) | (KeyCode::Char('q'), KeyModifiers::ALT) => {
                        editor.floating_window = None;
                        editor.focus_floating = false;
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                        if state.selected > 0 {
                            state.selected -= 1;
                            editor.update_menu_preview();
                        }
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                        if state.selected < state.items.len() - 1 {
                            state.selected += 1;
                            editor.update_menu_preview();
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
            crate::editor::FloatingMode::Settings { items, selected } => {
                // Settings mode navigation
                // Store values we need before handling
                let selected_idx = *selected;
                match (key.code, key.modifiers) {
                    (KeyCode::Esc, _) | (KeyCode::Char('?'), KeyModifiers::ALT) => {
                        editor.floating_window = None;
                        editor.focus_floating = false;
                    }
                    (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                        if *selected > 0 {
                            *selected -= 1;
                        }
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                        if *selected < items.len() - 1 {
                            *selected += 1;
                        }
                    }
                    // Toggle boolean or adjust number values
                    (KeyCode::Enter, _) | (KeyCode::Char(' '), _) => {
                        if let Some(item) = items.get_mut(*selected) {
                            match &mut item.value {
                                crate::editor::SettingValue::Bool(b) => {
                                    *b = !*b;
                                    // Apply the setting
                                    match item.name.as_str() {
                                        "Show Metadata" => editor.settings.show_metadata = *b,
                                        "Show Preview" => editor.settings.show_preview = *b,
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    // Adjust number values with C-h/C-l (consistent with directory navigation)
                    (KeyCode::Left, _) | (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                        if let Some(item) = items.get_mut(*selected) {
                            let name = item.name.clone();
                            match &mut item.value {
                                crate::editor::SettingValue::Number(n) => {
                                    if *n > 10 {
                                        *n -= 5;
                                        match name.as_str() {
                                            "Window Width" => editor.settings.floating_window_width = *n,
                                            "Window Height" => editor.settings.floating_window_height = *n,
                                            _ => {}
                                        }
                                    }
                                }
                                crate::editor::SettingValue::Choice { current, .. } => {
                                    if *current > 0 {
                                        *current -= 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                        // Extract the values we need before calling editor methods
                        let update_info = items.get(selected_idx).and_then(|item| {
                            match (&item.value, item.name.as_str()) {
                                (crate::editor::SettingValue::Choice { current, .. }, "Cursor Color") => {
                                    Some((true, *current))
                                }
                                (crate::editor::SettingValue::Choice { current, .. }, "Selection Color") => {
                                    Some((false, *current))
                                }
                                _ => None
                            }
                        });

                        // Drop the mutable reference to fw before calling editor methods
                        drop(fw);

                        // Now update colors
                        if let Some((is_cursor, index)) = update_info {
                            if is_cursor {
                                editor.settings.cursor_color = editor.index_to_color(index, false);
                            } else {
                                editor.settings.selection_color = editor.index_to_color(index, true);
                            }
                            editor.update_textarea_colors();
                        }

                        // Return early since we dropped fw
                        return true;
                    }
                    (KeyCode::Right, _) | (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                        if let Some(item) = items.get_mut(*selected) {
                            let name = item.name.clone();
                            match &mut item.value {
                                crate::editor::SettingValue::Number(n) => {
                                    if *n < 100 {
                                        *n += 5;
                                        match name.as_str() {
                                            "Window Width" => editor.settings.floating_window_width = *n,
                                            "Window Height" => editor.settings.floating_window_height = *n,
                                            _ => {}
                                        }
                                    }
                                }
                                crate::editor::SettingValue::Choice { current, options } => {
                                    if *current < options.len() - 1 {
                                        *current += 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                        // Extract the values we need before calling editor methods
                        let update_info = items.get(selected_idx).and_then(|item| {
                            match (&item.value, item.name.as_str()) {
                                (crate::editor::SettingValue::Choice { current, .. }, "Cursor Color") => {
                                    Some((true, *current))
                                }
                                (crate::editor::SettingValue::Choice { current, .. }, "Selection Color") => {
                                    Some((false, *current))
                                }
                                _ => None
                            }
                        });

                        // Drop the mutable reference to fw before calling editor methods
                        drop(fw);

                        // Now update colors
                        if let Some((is_cursor, index)) = update_info {
                            if is_cursor {
                                editor.settings.cursor_color = editor.index_to_color(index, false);
                            } else {
                                editor.settings.selection_color = editor.index_to_color(index, true);
                            }
                            editor.update_textarea_colors();
                        }

                        // Return early since we dropped fw
                        return true;
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