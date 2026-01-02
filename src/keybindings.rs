use crate::commands::{CtrlXPrefix, TestPrefix1, TestPrefix2};
use crate::editor::Editor;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::{CursorMove, Input};

/// Result of handling a key in a minibuffer-like context
enum MinibufferKeyResult {
    Handled,
    NotHandled,
    Cancel,
    Execute,
}

/// Handle common text editing keys for a string input (used in minibuffer, etc.)
/// Returns (handled, cursor_pos_change)
fn handle_string_edit_key(
    key: &KeyEvent,
    input: &mut String,
    cursor_pos: &mut usize,
) -> MinibufferKeyResult {
    match (key.code, key.modifiers) {
        // Cancel with ESC or C-g
        (KeyCode::Esc, _) | (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
            return MinibufferKeyResult::Cancel;
        }

        // Execute with Enter
        (KeyCode::Enter, _) => {
            return MinibufferKeyResult::Execute;
        }

        // Cursor movement
        (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
            *cursor_pos = 0;
        }
        (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
            *cursor_pos = input.chars().count();
        }
        (KeyCode::Char('f'), KeyModifiers::CONTROL) | (KeyCode::Right, _) => {
            let char_count = input.chars().count();
            if *cursor_pos < char_count {
                *cursor_pos += 1;
            }
        }
        (KeyCode::Char('b'), KeyModifiers::CONTROL) | (KeyCode::Left, _) => {
            if *cursor_pos > 0 {
                *cursor_pos -= 1;
            }
        }

        // Word movement (M-f, M-b)
        (KeyCode::Char('f'), KeyModifiers::ALT) => {
            let chars: Vec<char> = input.chars().collect();
            let mut pos = *cursor_pos;
            // Skip current word
            while pos < chars.len() && chars[pos].is_alphanumeric() {
                pos += 1;
            }
            // Skip whitespace
            while pos < chars.len() && !chars[pos].is_alphanumeric() {
                pos += 1;
            }
            *cursor_pos = pos;
        }
        (KeyCode::Char('b'), KeyModifiers::ALT) => {
            let chars: Vec<char> = input.chars().collect();
            let mut pos = *cursor_pos;
            if pos > 0 {
                pos -= 1;
                // Skip whitespace
                while pos > 0 && !chars[pos].is_alphanumeric() {
                    pos -= 1;
                }
                // Skip word
                while pos > 0 && chars[pos - 1].is_alphanumeric() {
                    pos -= 1;
                }
            }
            *cursor_pos = pos;
        }

        // Kill word backward (M-Backspace) - must come before regular Backspace
        (KeyCode::Backspace, KeyModifiers::ALT) => {
            let chars: Vec<char> = input.chars().collect();
            let mut new_pos = *cursor_pos;
            if new_pos > 0 {
                new_pos -= 1;
                while new_pos > 0 && !chars[new_pos].is_alphanumeric() {
                    new_pos -= 1;
                }
                while new_pos > 0 && chars[new_pos - 1].is_alphanumeric() {
                    new_pos -= 1;
                }
            }
            let new_chars: String = chars[..new_pos].iter()
                .chain(chars[*cursor_pos..].iter())
                .collect();
            *input = new_chars;
            *cursor_pos = new_pos;
        }

        // Kill word forward (M-d)
        (KeyCode::Char('d'), KeyModifiers::ALT) => {
            let chars: Vec<char> = input.chars().collect();
            let mut end_pos = *cursor_pos;
            // Skip current word
            while end_pos < chars.len() && chars[end_pos].is_alphanumeric() {
                end_pos += 1;
            }
            // Skip whitespace
            while end_pos < chars.len() && !chars[end_pos].is_alphanumeric() {
                end_pos += 1;
            }
            let new_chars: String = chars[..*cursor_pos].iter()
                .chain(chars[end_pos..].iter())
                .collect();
            *input = new_chars;
        }

        // Regular Backspace (after M-Backspace to avoid being shadowed)
        (KeyCode::Backspace, KeyModifiers::NONE) => {
            if *cursor_pos > 0 {
                let chars: Vec<char> = input.chars().collect();
                let new_chars: String = chars[..*cursor_pos - 1].iter()
                    .chain(chars[*cursor_pos..].iter())
                    .collect();
                *input = new_chars;
                *cursor_pos -= 1;
            }
        }

        // Delete (C-d or Delete key)
        (KeyCode::Char('d'), KeyModifiers::CONTROL) | (KeyCode::Delete, _) => {
            let char_count = input.chars().count();
            if *cursor_pos < char_count {
                let chars: Vec<char> = input.chars().collect();
                let new_chars: String = chars[..*cursor_pos].iter()
                    .chain(chars[*cursor_pos + 1..].iter())
                    .collect();
                *input = new_chars;
            }
        }

        // Kill to end of line (C-k)
        (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
            let chars: Vec<char> = input.chars().collect();
            *input = chars[..*cursor_pos].iter().collect();
        }

        // Kill to beginning of line (C-u)
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            let chars: Vec<char> = input.chars().collect();
            *input = chars[*cursor_pos..].iter().collect();
            *cursor_pos = 0;
        }

        // Regular character input
        (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
            let chars: Vec<char> = input.chars().collect();
            let new_chars: String = chars[..*cursor_pos].iter()
                .chain(std::iter::once(&c))
                .chain(chars[*cursor_pos..].iter())
                .collect();
            *input = new_chars;
            *cursor_pos += 1;
        }

        _ => return MinibufferKeyResult::NotHandled,
    }

    MinibufferKeyResult::Handled
}

pub fn handle_input(editor: &mut Editor, key: KeyEvent) -> bool {
    log::debug!("Key input: {:?} modifiers: {:?}", key.code, key.modifiers);

    // C-x C-q: Ultimate force quit - bypasses everything, exits immediately
    // This is the "kill switch" that always works regardless of editor state
    if editor.last_key == Some((KeyCode::Char('x'), KeyModifiers::CONTROL)) {
        if key.code == KeyCode::Char('q') && key.modifiers == KeyModifiers::CONTROL {
            // Restore terminal state before force quitting
            let _ = ratatui::restore();
            std::process::exit(0);
        }
    }

    // Track if we had a floating window before should_quit
    let had_floating = editor.floating_window.is_some();

    // Check for quit commands first
    if should_quit(editor, &key) {
        return false;
    }

    // If should_quit just opened a dialog, don't process this key further
    // (prevents the same ESC from both opening and closing the dialog)
    if !had_floating && editor.floating_window.is_some() {
        return true;
    }

    // If floating window exists, handle input specially
    // (always route to floating handler, don't require focus_floating to be true)
    if editor.floating_window.is_some() {
        // Ensure focus is on the floating window
        editor.focus_floating = true;
        let result = handle_floating_input(editor, key);
        // Check if pending_quit was set during dialog processing
        if editor.pending_quit {
            return false;
        }
        return result;
    }

    // Handle active prefix (which-key mode)
    if editor.status_bar.active_prefix.is_some() {
        // Handle page navigation with M-< and M-> when which-key is expanded
        if editor.status_bar.expanded {
            match (key.code, key.modifiers) {
                // M-< (previous page)
                (KeyCode::Char('<'), KeyModifiers::ALT) => {
                    editor.status_bar.which_key_prev_page();
                    return true;
                }
                // M-> (next page) - use large items_per_page to ensure we always advance at least one page
                (KeyCode::Char('>'), KeyModifiers::ALT) => {
                    // Use 3 as minimum (matches ui.rs .max(3))
                    editor.status_bar.which_key_next_page(3);
                    return true;
                }
                _ => {}
            }
        }

        // Get the command for this follow-up key
        let command = editor.status_bar.active_prefix
            .as_ref()
            .and_then(|p| p.get_command(&key));

        // Clear the prefix state
        editor.status_bar.clear_prefix();
        editor.last_key = None;

        // Execute command if found
        if let Some(cmd_name) = command {
            return execute_command(editor, cmd_name);
        }

        // Invalid follow-up key - just return (prefix was cancelled)
        return true;
    }

    // Handle C-x prefix activation
    if key.code == KeyCode::Char('x') && key.modifiers == KeyModifiers::CONTROL {
        editor.status_bar.activate_prefix(Box::new(CtrlXPrefix));
        editor.last_key = Some((KeyCode::Char('x'), KeyModifiers::CONTROL));
        return true;
    }

    // TEST: Handle C-t prefix activation (delete after testing)
    if key.code == KeyCode::Char('t') && key.modifiers == KeyModifiers::CONTROL {
        editor.status_bar.activate_prefix(Box::new(TestPrefix1));
        editor.last_key = Some((KeyCode::Char('t'), KeyModifiers::CONTROL));
        return true;
    }

    // TEST: Handle C-c prefix activation (delete after testing)
    if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
        editor.status_bar.activate_prefix(Box::new(TestPrefix2));
        editor.last_key = Some((KeyCode::Char('c'), KeyModifiers::CONTROL));
        return true;
    }

    // Clear last_key for any key that isn't C-SPC
    if !matches!((key.code, key.modifiers), (KeyCode::Char(' '), KeyModifiers::CONTROL)) {
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
                editor.mark_modified();
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
        // M-x command palette
        (KeyCode::Char('x'), KeyModifiers::ALT) => {
            editor.open_command_palette();
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
            editor.mark_modified();
        }
        (KeyCode::Char('w'), KeyModifiers::ALT) => {
            editor.copy_region();
            // Copy doesn't modify buffer
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
            editor.yank();
            editor.mark_modified();
        }
        (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
            editor.kill_to_end_of_line();
            editor.mark_modified();
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            editor.kill_to_beginning_of_line();
            editor.mark_modified();
        }

        // Default: pass through to textarea
        _ => {
            let event = ratatui::crossterm::event::Event::Key(key);
            let input: Input = event.into();
            editor.textarea.input(input);
            editor.reset_kill_sequence();
            // Mark as modified for text-changing keys
            if matches!(key.code,
                KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete | KeyCode::Enter | KeyCode::Tab
            ) {
                editor.mark_modified();
            }
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
                        let _ = fw; // End borrow

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
                        let _ = fw; // End borrow

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
                // Text edit mode - use tui_textarea's built-in handling
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

                    // Basic movement commands
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

                    // Word movement
                    (KeyCode::Char('f'), KeyModifiers::ALT) => {
                        fw.textarea.move_cursor(CursorMove::WordForward);
                    }
                    (KeyCode::Char('b'), KeyModifiers::ALT) => {
                        fw.textarea.move_cursor(CursorMove::WordBack);
                    }

                    // Kill/yank operations
                    (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                        fw.textarea.delete_line_by_end();
                    }
                    (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                        fw.textarea.delete_line_by_head();
                    }
                    (KeyCode::Char('d'), KeyModifiers::ALT) => {
                        fw.textarea.delete_word();
                    }

                    // Default: pass through to floating window textarea
                    _ => {
                        let event = ratatui::crossterm::event::Event::Key(key);
                        let input: Input = event.into();
                        fw.textarea.input(input);
                    }
                }
            }

            crate::editor::FloatingMode::Minibuffer {
                input,
                cursor_pos,
                completions,
                selected_completion,
                ..
            } => {
                log::debug!("Minibuffer key: {:?}", key.code);

                // Helper: check if path ends with / (is a directory)
                let is_directory = |path: &str| path.ends_with('/');

                // Handle Tab - enter directory or cycle completions
                if matches!((key.code, key.modifiers), (KeyCode::Tab, _)) {
                    if !completions.is_empty() {
                        // If we have a selected completion that's a directory, enter it
                        if let Some(idx) = *selected_completion {
                            if let Some(comp) = completions.get(idx) {
                                if is_directory(comp) {
                                    // Enter the directory
                                    *input = comp.clone();
                                    *cursor_pos = input.chars().count();
                                    *completions = crate::editor::Editor::get_path_completions(input);
                                    *selected_completion = if completions.is_empty() { None } else { Some(0) };
                                    return true;
                                }
                            }
                        }

                        // Otherwise, cycle to next completion and update input
                        if let Some(idx) = *selected_completion {
                            *selected_completion = Some((idx + 1) % completions.len());
                        } else {
                            *selected_completion = Some(0);
                        }
                        // Update input to show selected completion
                        if let Some(completion) = selected_completion.and_then(|i| completions.get(i)) {
                            *input = completion.clone();
                            *cursor_pos = input.chars().count();
                        }
                    } else {
                        // No completions, refresh them
                        *completions = crate::editor::Editor::get_path_completions(input);
                        if !completions.is_empty() {
                            *selected_completion = Some(0);
                            // Update input to first completion
                            if let Some(comp) = completions.first() {
                                *input = comp.clone();
                                *cursor_pos = input.chars().count();
                            }
                        }
                    }
                    return true;
                }

                // Handle Up/Down - navigate completions and update input
                match (key.code, key.modifiers) {
                    (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) if !completions.is_empty() => {
                        if let Some(idx) = *selected_completion {
                            *selected_completion = Some(if idx == 0 { completions.len() - 1 } else { idx - 1 });
                        } else {
                            *selected_completion = Some(completions.len() - 1);
                        }
                        // Update input to show selected completion
                        if let Some(completion) = selected_completion.and_then(|i| completions.get(i)) {
                            *input = completion.clone();
                            *cursor_pos = input.chars().count();
                        }
                        return true;
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) if !completions.is_empty() => {
                        if let Some(idx) = *selected_completion {
                            *selected_completion = Some((idx + 1) % completions.len());
                        } else {
                            *selected_completion = Some(0);
                        }
                        // Update input to show selected completion
                        if let Some(completion) = selected_completion.and_then(|i| completions.get(i)) {
                            *input = completion.clone();
                            *cursor_pos = input.chars().count();
                        }
                        return true;
                    }
                    _ => {}
                }

                // Handle Enter - enter directory or open file
                if matches!((key.code, key.modifiers), (KeyCode::Enter, _)) {
                    // Check if current input is a directory
                    if is_directory(input) {
                        // Already in a directory, refresh completions to show contents
                        *completions = crate::editor::Editor::get_path_completions(input);
                        *selected_completion = if completions.is_empty() { None } else { Some(0) };
                        // Update input to first completion if available
                        if let Some(comp) = completions.first() {
                            *input = comp.clone();
                            *cursor_pos = input.chars().count();
                        }
                        return true;
                    }

                    // Check if input path is a directory (without trailing /)
                    let expanded = crate::editor::Editor::expand_path(input);
                    if expanded.is_dir() {
                        // Add trailing / and enter directory
                        input.push('/');
                        *cursor_pos = input.chars().count();
                        *completions = crate::editor::Editor::get_path_completions(input);
                        *selected_completion = if completions.is_empty() { None } else { Some(0) };
                        return true;
                    }

                    // It's a file, execute callback
                    let _ = fw; // End borrow
                    editor.execute_minibuffer_callback();
                    return true;
                }

                // Handle Backspace specially - go up directory at boundary
                if matches!((key.code, key.modifiers), (KeyCode::Backspace, KeyModifiers::NONE)) {
                    // Check if we're at a directory boundary (cursor after a /)
                    let chars: Vec<char> = input.chars().collect();
                    if *cursor_pos > 0 && chars.get(*cursor_pos - 1) == Some(&'/') {
                        // Go up one directory level
                        // Remove trailing / and find previous /
                        let path_without_trailing = &input[..input.len() - 1];
                        if let Some(last_sep) = path_without_trailing.rfind('/') {
                            *input = format!("{}/", &path_without_trailing[..last_sep]);
                            *cursor_pos = input.chars().count();
                            *completions = crate::editor::Editor::get_path_completions(input);
                            *selected_completion = if completions.is_empty() { None } else { Some(0) };
                        } else if input.starts_with('~') {
                            // At home directory, can't go higher
                            *input = "~/".to_string();
                            *cursor_pos = 2;
                            *completions = crate::editor::Editor::get_path_completions(input);
                            *selected_completion = if completions.is_empty() { None } else { Some(0) };
                        }
                        return true;
                    }
                    // Otherwise, normal backspace - handled by shared handler below
                }

                // Handle ESC/C-g for cancel
                if matches!((key.code, key.modifiers), (KeyCode::Esc, _) | (KeyCode::Char('g'), KeyModifiers::CONTROL)) {
                    editor.floating_window = None;
                    editor.focus_floating = false;
                    return true;
                }

                // Use shared text editing handler for other keys
                match handle_string_edit_key(&key, input, cursor_pos) {
                    MinibufferKeyResult::Cancel => {
                        editor.floating_window = None;
                        editor.focus_floating = false;
                    }
                    MinibufferKeyResult::Execute => {
                        // Already handled Enter above, but just in case
                        let _ = fw; // End borrow
                        editor.execute_minibuffer_callback();
                        return true;
                    }
                    MinibufferKeyResult::Handled => {
                        // Refresh completions after any input change
                        *completions = crate::editor::Editor::get_path_completions(input);
                        *selected_completion = if completions.is_empty() { None } else { Some(0) };
                    }
                    MinibufferKeyResult::NotHandled => {}
                }
            }

            crate::editor::FloatingMode::CommandPalette {
                input,
                cursor_pos,
                filtered_commands,
                selected,
            } => {
                match (key.code, key.modifiers) {
                    // Cancel with ESC or C-g
                    (KeyCode::Esc, _) | (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                        editor.floating_window = None;
                        editor.focus_floating = false;
                        return true;
                    }

                    // Navigate up/down
                    (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                        if *selected > 0 {
                            *selected -= 1;
                        }
                        return true;
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                        if !filtered_commands.is_empty() && *selected < filtered_commands.len() - 1 {
                            *selected += 1;
                        }
                        return true;
                    }

                    // Jump by 10 with M-< and M->
                    (KeyCode::Char('<'), KeyModifiers::ALT) => {
                        *selected = selected.saturating_sub(10);
                        return true;
                    }
                    (KeyCode::Char('>'), KeyModifiers::ALT) => {
                        if !filtered_commands.is_empty() {
                            *selected = (*selected + 10).min(filtered_commands.len() - 1);
                        }
                        return true;
                    }

                    // Tab for completion - select the current item
                    (KeyCode::Tab, _) => {
                        if let Some(cmd) = filtered_commands.get(*selected) {
                            *input = cmd.name.to_string();
                            *cursor_pos = input.len();
                            // Reset selection to top since input changed
                            *selected = 0;
                        }
                        return true;
                    }

                    // Execute selected command
                    (KeyCode::Enter, _) => {
                        if let Some(cmd) = filtered_commands.get(*selected) {
                            let cmd_name = cmd.name;
                            // Close the palette first
                            editor.floating_window = None;
                            editor.focus_floating = false;
                            // Execute the command
                            return execute_command(editor, cmd_name);
                        }
                        return true;
                    }

                    // Text editing
                    _ => {
                        let needs_filter = match handle_string_edit_key(&key, input, cursor_pos) {
                            MinibufferKeyResult::Cancel => {
                                editor.floating_window = None;
                                editor.focus_floating = false;
                                return true;
                            }
                            MinibufferKeyResult::Execute => {
                                // Try to execute by exact name match
                                if let Some(cmd) = filtered_commands.get(*selected) {
                                    let cmd_name = cmd.name;
                                    editor.floating_window = None;
                                    editor.focus_floating = false;
                                    return execute_command(editor, cmd_name);
                                }
                                false
                            }
                            MinibufferKeyResult::Handled => true,
                            MinibufferKeyResult::NotHandled => false,
                        };

                        if needs_filter {
                            // Get the updated input
                            let updated_input = input.clone();
                            // Drop the borrow and filter
                            let _ = fw; // End borrow
                            let new_filtered = editor.filter_commands(&updated_input);

                            // Re-borrow and update
                            if let Some(ref mut fw) = editor.floating_window {
                                if let crate::editor::FloatingMode::CommandPalette {
                                    filtered_commands,
                                    selected,
                                    ..
                                } = &mut fw.mode {
                                    *filtered_commands = new_filtered;
                                    *selected = 0;
                                }
                            }
                        }
                    }
                }
            }

            crate::editor::FloatingMode::Confirm {
                steps,
                current_index,
                text_input,
                ..
            } => {
                // Check what type of response we're expecting
                let response_type = steps.get(*current_index)
                    .map(|s| s.response_type.clone());
                let total_steps = steps.len();
                let idx = *current_index;

                // Determine what action to take based on response type and key
                let action = match response_type {
                    Some(crate::editor::ResponseType::Binary) => {
                        match (key.code, key.modifiers) {
                            (KeyCode::Esc, _) | (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                                Some(ConfirmAction::Cancel)
                            }
                            (KeyCode::Char('y'), _) => Some(ConfirmAction::Respond("y".to_string())),
                            (KeyCode::Char('n'), _) => Some(ConfirmAction::Respond("n".to_string())),
                            _ => None,
                        }
                    }
                    Some(crate::editor::ResponseType::Choice(options)) => {
                        match (key.code, key.modifiers) {
                            (KeyCode::Esc, _) | (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                                Some(ConfirmAction::Cancel)
                            }
                            (KeyCode::Char(c), _) => {
                                if options.iter().any(|(k, _)| *k == c) {
                                    Some(ConfirmAction::Respond(c.to_string()))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        }
                    }
                    Some(crate::editor::ResponseType::TextInput { .. }) => {
                        let mut cursor = text_input.chars().count();
                        match handle_string_edit_key(&key, text_input, &mut cursor) {
                            MinibufferKeyResult::Cancel => Some(ConfirmAction::Cancel),
                            MinibufferKeyResult::Execute => {
                                let input = text_input.clone();
                                *text_input = String::new();
                                Some(ConfirmAction::Respond(input))
                            }
                            _ => None,
                        }
                    }
                    None => None,
                };

                // Drop the borrow before processing the action
                let _ = fw; // End borrow

                // Now process the action with full access to editor
                if let Some(action) = action {
                    process_confirm_action(editor, action, idx, total_steps);
                }
            }
        }
    }

    true
}

/// Action to take in a confirmation dialog
enum ConfirmAction {
    Cancel,
    Respond(String),
}

/// Process a confirmation action
fn process_confirm_action(
    editor: &mut crate::editor::Editor,
    action: ConfirmAction,
    current_index: usize,
    total_steps: usize,
) {
    match action {
        ConfirmAction::Cancel => {
            cancel_confirm_dialog(editor);
        }
        ConfirmAction::Respond(response) => {
            // Take ownership of the floating window to call handle_response
            if let Some(mut fw) = editor.floating_window.take() {
                if let crate::editor::FloatingMode::Confirm { ref mut dialog, .. } = fw.mode {
                    let result = dialog.handle_response(current_index, &response, editor);

                    // Put the window back (might be modified by handle_response)
                    editor.floating_window = Some(fw);

                    // Apply the result
                    apply_confirm_result(editor, result, total_steps, current_index);
                }
            }
        }
    }
}

/// Cancel a confirmation dialog
fn cancel_confirm_dialog(editor: &mut crate::editor::Editor) {
    // Take ownership to avoid borrow issues
    if let Some(fw) = editor.floating_window.take() {
        if let crate::editor::FloatingMode::Confirm { dialog, .. } = fw.mode {
            dialog.on_cancel(editor);
        }
    }
    editor.focus_floating = false;
}

/// Apply the result of a confirmation response
fn apply_confirm_result(
    editor: &mut crate::editor::Editor,
    result: crate::editor::ResponseResult,
    total_steps: usize,
    current_index: usize,
) {
    use crate::editor::ResponseResult;

    match result {
        ResponseResult::Continue => {
            if current_index + 1 >= total_steps {
                // Last step, execute on_complete
                if let Some(fw) = editor.floating_window.take() {
                    if let crate::editor::FloatingMode::Confirm { dialog, .. } = fw.mode {
                        let _ = dialog.on_complete(editor);
                    }
                }
                editor.focus_floating = false;
            } else {
                // Advance to next step
                if let Some(ref mut fw) = editor.floating_window {
                    if let crate::editor::FloatingMode::Confirm { current_index: ref mut ci, .. } = fw.mode {
                        *ci = current_index + 1;
                    }
                }
            }
        }
        ResponseResult::Back => {
            if current_index > 0 {
                if let Some(ref mut fw) = editor.floating_window {
                    if let crate::editor::FloatingMode::Confirm { current_index: ref mut ci, .. } = fw.mode {
                        *ci = current_index - 1;
                    }
                }
            }
        }
        ResponseResult::GoTo(idx) => {
            if idx < total_steps {
                if let Some(ref mut fw) = editor.floating_window {
                    if let crate::editor::FloatingMode::Confirm { current_index: ref mut ci, .. } = fw.mode {
                        *ci = idx;
                    }
                }
            }
        }
        ResponseResult::Stay => {
            // Do nothing, stay on current step
        }
        ResponseResult::Cancel => {
            cancel_confirm_dialog(editor);
        }
        ResponseResult::Finish => {
            // Complete immediately
            if let Some(fw) = editor.floating_window.take() {
                if let crate::editor::FloatingMode::Confirm { dialog, .. } = fw.mode {
                    let _ = dialog.on_complete(editor);
                }
            }
            editor.focus_floating = false;
        }
    }
}

fn should_quit(editor: &mut Editor, key: &KeyEvent) -> bool {
    // Check if we should quit after a confirmation dialog
    if editor.pending_quit {
        return true;
    }

    // Note: C-ESC force quit is handled at the start of handle_input
    // before this function is even called

    // Handle floating windows - let handle_floating_input process all keys
    // (including ESC/C-g which will close the window there)
    if editor.floating_window.is_some() {
        return false;
    }

    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => {
            if editor.mark_active {
                editor.cancel_mark();
                false
            } else if editor.modified {
                // Buffer modified, show confirmation
                editor.start_quit_confirmation();
                false
            } else {
                true
            }
        }
        (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
            if editor.mark_active {
                editor.cancel_mark();
                false
            } else if editor.modified {
                // Buffer modified, show confirmation
                editor.start_quit_confirmation();
                false
            } else {
                true
            }
        }
        _ => false,
    }
}

/// Execute a command by name
fn execute_command(editor: &mut Editor, command_name: &str) -> bool {
    log::info!("Command: {}", command_name);
    match command_name {
        // File commands
        "open-file" => {
            editor.open_file_prompt();
            true
        }
        "save-file" => {
            let _ = editor.save_file();
            true
        }
        "save-file-as" => {
            editor.save_file_as_prompt();
            true
        }
        "delete-file" => {
            editor.delete_file_prompt();
            true
        }

        // Selection commands
        "swap-cursor-mark" => {
            editor.swap_cursor_mark();
            true
        }

        // System commands
        "force-quit" => {
            let _ = ratatui::restore();
            std::process::exit(0);
        }
        "operate" => {
            editor.toggle_floating_window();
            true
        }
        "settings" => {
            editor.open_settings_menu();
            true
        }
        "execute-command" => {
            editor.open_command_palette();
            true
        }

        // Movement commands
        "forward-char" => {
            editor.move_cursor(CursorMove::Forward);
            true
        }
        "backward-char" => {
            editor.move_cursor(CursorMove::Back);
            true
        }
        "next-line" => {
            if editor.is_at_last_line() {
                editor.textarea.move_cursor(CursorMove::End);
                editor.textarea.insert_newline();
                editor.reset_kill_sequence();
                editor.mark_modified();
            } else {
                editor.move_cursor(CursorMove::Down);
            }
            true
        }
        "previous-line" => {
            editor.move_cursor(CursorMove::Up);
            true
        }
        "beginning-of-line" => {
            editor.move_cursor(CursorMove::Head);
            true
        }
        "end-of-line" => {
            editor.move_cursor(CursorMove::End);
            true
        }
        "forward-word" => {
            editor.move_word_forward();
            true
        }
        "backward-word" => {
            editor.move_word_backward();
            true
        }

        // Edit commands
        "kill-line" => {
            editor.kill_to_end_of_line();
            editor.mark_modified();
            true
        }
        "kill-line-backward" => {
            editor.kill_to_beginning_of_line();
            editor.mark_modified();
            true
        }
        "yank" => {
            editor.yank();
            editor.mark_modified();
            true
        }

        // Selection commands
        "set-mark" => {
            editor.set_mark();
            true
        }
        "kill-region" => {
            editor.kill_region();
            editor.mark_modified();
            true
        }
        "copy-region" => {
            editor.copy_region();
            true
        }

        // Unknown command
        _ => {
            // Command not found - just return true to continue
            true
        }
    }
}