use crate::commands::CtrlXPrefix;
use crate::editor::buffer_ops::is_text_input_key;
use crate::editor::{Editor, JumpMode, JumpPhase};
use crate::logging;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{SystemTime, UNIX_EPOCH};
use tui_textarea::{CursorMove, Input};

/// Get current time in milliseconds since Unix epoch
fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Result of handling jump mode input
enum JumpModeResult {
    /// Jump mode handled the key, continue running
    Handled,
    /// Jump mode not active
    NotActive,
    /// Jump mode cancelled
    Cancelled,
    /// Jump completed, cursor moved
    Jumped,
}

/// Handle input when jump mode is active
fn handle_jump_mode_input(editor: &mut Editor, key: &KeyEvent) -> JumpModeResult {
    let jump_mode = match &editor.jump_mode {
        Some(jm) => jm.clone(),
        None => return JumpModeResult::NotActive,
    };

    match (key.code, key.modifiers) {
        // Cancel with ESC or C-g
        (KeyCode::Esc, _) | (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
            editor.jump_mode = None;
            return JumpModeResult::Cancelled;
        }
        // Backspace - remove last character from pattern (accept any modifiers)
        (KeyCode::Backspace, _) => {
            match &mut editor.jump_mode {
                Some(ref mut jm) => {
                    jm.pattern.pop();
                    jm.last_keystroke_ms = current_time_ms();
                    // Update targets based on new pattern
                    let lines: Vec<String> = editor.textarea.lines().iter().map(|s| s.to_string()).collect();
                    let pattern = jm.pattern.clone();
                    jm.find_matches(&lines, &pattern);
                    // If pattern is empty, cancel jump mode
                    match jm.pattern.is_empty() {
                        true => {
                            editor.jump_mode = None;
                            return JumpModeResult::Cancelled;
                        }
                        false => {}
                    }
                }
                None => {}
            }
            return JumpModeResult::Handled;
        }
        // Enter - immediately transition to selecting phase
        (KeyCode::Enter, _) => {
            match &mut editor.jump_mode {
                Some(ref mut jm) => {
                    match jm.targets.is_empty() {
                        true => {
                            // No matches, cancel
                            editor.jump_mode = None;
                            return JumpModeResult::Cancelled;
                        }
                        false => {
                            // Transition to selecting phase
                            jm.phase = JumpPhase::Selecting;
                        }
                    }
                }
                None => {}
            }
            return JumpModeResult::Handled;
        }
        // Regular character input
        (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
            match jump_mode.phase {
                JumpPhase::Typing => {
                    match &mut editor.jump_mode {
                        Some(ref mut jm) => {
                            jm.pattern.push(c);
                            jm.last_keystroke_ms = current_time_ms();
                            // Update targets based on new pattern
                            let lines: Vec<String> = editor.textarea.lines().iter().map(|s| s.to_string()).collect();
                            let pattern = jm.pattern.clone();
                            jm.find_matches(&lines, &pattern);
                        }
                        None => {}
                    }
                    return JumpModeResult::Handled;
                }
                JumpPhase::Selecting => {
                    // Try to match against labels
                    let target = jump_mode.find_target_by_label(&c.to_string());
                    match target {
                        Some(t) => {
                            // Jump to target
                            let row = t.row;
                            let col = t.col;
                            editor.jump_mode = None;
                            // Move cursor to target position
                            editor.textarea.move_cursor(CursorMove::Jump(row as u16, col as u16));
                            return JumpModeResult::Jumped;
                        }
                        None => {
                            // Check if this is a prefix for multi-char labels
                            match jump_mode.has_label_prefix(&c.to_string()) {
                                true => {
                                    // Wait for more input - update pending label
                                    match &mut editor.jump_mode {
                                        Some(ref mut jm) => {
                                            // Filter targets to only those starting with this char
                                            jm.targets.retain(|t| t.label.starts_with(c));
                                            // Shorten labels by removing the first char
                                            for t in &mut jm.targets {
                                                match t.label.len() > 1 {
                                                    true => {
                                                        t.label = t.label[1..].to_string();
                                                    }
                                                    false => {}
                                                }
                                            }
                                        }
                                        None => {}
                                    }
                                    return JumpModeResult::Handled;
                                }
                                false => {
                                    // Invalid label, cancel
                                    editor.jump_mode = None;
                                    return JumpModeResult::Cancelled;
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => {
            // Unknown key in jump mode, cancel
            editor.jump_mode = None;
            return JumpModeResult::Cancelled;
        }
    }
}

/// Check if jump mode should transition from typing to selecting based on timeout
pub fn check_jump_mode_timeout(editor: &mut Editor) {
    match &mut editor.jump_mode {
        Some(ref mut jm) => {
            match jm.phase {
                JumpPhase::Typing => {
                    let now = current_time_ms();
                    let elapsed = now.saturating_sub(jm.last_keystroke_ms);
                    match elapsed >= jm.timeout_ms && !jm.pattern.is_empty() && !jm.targets.is_empty() {
                        true => {
                            jm.phase = JumpPhase::Selecting;
                        }
                        false => {}
                    }
                }
                JumpPhase::Selecting => {}
            }
        }
        None => {}
    }
}

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
    if logging::log_keys() {
        log::debug!("Key input: {:?} modifiers: {:?}", key.code, key.modifiers);
    }

    // C-x C-q: Ultimate force quit - bypasses everything, exits immediately
    // This is the "kill switch" that always works regardless of editor state
    if editor.last_key == Some((KeyCode::Char('x'), KeyModifiers::CONTROL))
        && key.code == KeyCode::Char('q')
        && key.modifiers == KeyModifiers::CONTROL
    {
        // Restore terminal state before force quitting
        ratatui::restore();
        std::process::exit(0);
    }

    // Check jump mode timeout before processing input
    check_jump_mode_timeout(editor);

    // Handle jump mode input (takes priority over everything except force quit)
    match handle_jump_mode_input(editor, &key) {
        JumpModeResult::Handled => {
            log::debug!("Jump mode: Handled");
            return true;
        }
        JumpModeResult::Jumped => {
            log::debug!("Jump mode: Jumped");
            return true;
        }
        JumpModeResult::Cancelled => {
            log::debug!("Jump mode: Cancelled");
            return true;
        }
        JumpModeResult::NotActive => {
            // Continue with normal handling
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
        match command {
            Some(cmd_name) => return execute_command(editor, cmd_name),
            None => {}
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

    // Clear last_key for any key that isn't C-SPC
    if !matches!((key.code, key.modifiers), (KeyCode::Char(' '), KeyModifiers::CONTROL)) {
        editor.last_key = None;
    }

    // Handle Emacs keybindings
    match (key.code, key.modifiers) {
        // C-g and ESC are handled by should_quit above - don't pass to default
        // This prevents them from being sent to textarea.input_without_shortcuts
        (KeyCode::Char('g'), KeyModifiers::CONTROL) | (KeyCode::Esc, _) => {
            // Already handled by should_quit, just return
        }

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
                editor.save_undo_state();
                editor.textarea.move_cursor(CursorMove::End);
                editor.textarea.insert_newline();
                // Text input resets selection
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
            if logging::log_keys() {
                log::debug!("C-e pressed, calling move_cursor(End)");
            }
            editor.move_cursor(CursorMove::End);
        }

        // Page up/down
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            editor.page_up();
        }
        (KeyCode::Char('u'), KeyModifiers::ALT) => {
            editor.page_down();
        }

        // Recenter (C-l) - cycles through center/top/bottom
        (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
            editor.recenter();
        }

        // Word movement - these don't reset kill sequence
        (KeyCode::Char('f'), KeyModifiers::ALT) => {
            editor.move_word_forward();
        }
        (KeyCode::Char('b'), KeyModifiers::ALT) => {
            if logging::log_keys() {
                log::debug!("M-b pressed, calling move_word_backward");
            }
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
            if logging::log_keys() || logging::log_selection() {
                log::debug!("C-SPC pressed, calling set_mark");
            }
            editor.set_mark();
            if logging::log_selection() {
                log::debug!(
                    "After set_mark: mark={:?}, is_selecting={}, selection_range={:?}",
                    editor.mark,
                    editor.textarea.is_selecting(),
                    editor.textarea.selection_range()
                );
            }
            // Store this key for detecting C-SPC C-SPC, but only if we didn't
            // just toggle off a selection (set_mark clears last_key in that case)
            if editor.last_key.is_some() || editor.mark.is_active() {
                editor.last_key = Some((KeyCode::Char(' '), KeyModifiers::CONTROL));
            }
        }
        // C-x prefix for commands
        (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
            // Store for potential C-x sequences
            editor.last_key = Some((KeyCode::Char('x'), KeyModifiers::CONTROL));
        }

        // Kill and yank operations
        // All these operations are self-contained: they handle undo state and
        // modification tracking internally. See buffer_ops.rs for the pattern.
        (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
            editor.cut_region();
        }
        (KeyCode::Char('w'), KeyModifiers::ALT) => {
            editor.copy_region();
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
            editor.paste();
        }
        (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
            editor.cut_to_end_of_line();
        }

        // Undo (C-z)
        (KeyCode::Char('z'), mods) if mods.contains(KeyModifiers::CONTROL) && !mods.contains(KeyModifiers::SHIFT) => {
            editor.undo();
        }

        // Redo (M-z)
        (KeyCode::Char('z'), mods) if mods.contains(KeyModifiers::ALT) => {
            editor.redo();
        }

        // Syntax-aware selection expansion (Alt-o, like Helix)
        (KeyCode::Char('o'), KeyModifiers::ALT) => {
            editor.expand_selection();
        }

        // Syntax-aware selection shrink (Alt-i, like Helix)
        (KeyCode::Char('i'), KeyModifiers::ALT) => {
            editor.shrink_selection();
        }

        // Avy-like jump mode (M-j for "jump")
        (KeyCode::Char('j'), mods) if mods.contains(KeyModifiers::ALT) => {
            // Ensure clean state
            editor.jump_mode = None;
            let mut jm = JumpMode::new();
            jm.last_keystroke_ms = current_time_ms();
            editor.jump_mode = Some(jm);
            log::debug!("Jump mode activated");
        }

        // Word delete operations (self-contained)
        (KeyCode::Char('d'), mods) if mods.contains(KeyModifiers::ALT) => {
            editor.delete_word_forward();
        }
        (KeyCode::Backspace, mods) if mods.contains(KeyModifiers::ALT) => {
            editor.delete_word_backward();
        }

        // Default: pass through to textarea (without tui-textarea's default shortcuts)
        _ => {
            // Use the is_text_input_key helper to determine if this key actually
            // modifies the buffer. Control/Alt+letter that aren't handled above
            // are ignored (e.g., C-t does nothing, shouldn't mark modified).
            // See buffer_ops.rs for documentation.
            if is_text_input_key(key.code, key.modifiers) {
                editor.save_undo_state();
                let event = ratatui::crossterm::event::Event::Key(key);
                let input: Input = event.into();
                editor.textarea.input_without_shortcuts(input);
                editor.mark_modified();
            } else {
                // Non-text input key that we don't handle - just pass through
                // but don't save undo or mark modified
                let event = ratatui::crossterm::event::Event::Key(key);
                let input: Input = event.into();
                editor.textarea.input_without_shortcuts(input);
            }
        }
    }

    true
}

fn handle_floating_input(editor: &mut Editor, key: KeyEvent) -> bool {
    match &mut editor.floating_window {
        Some(fw) => match &mut fw.mode {
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
                        match items.get_mut(*selected) {
                            Some(item) => match &mut item.value {
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
                            },
                            None => {}
                        }
                    }
                    // Adjust number values with C-h/C-l (consistent with directory navigation)
                    (KeyCode::Left, _) | (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                        match items.get_mut(*selected) {
                            Some(item) => {
                                let name = item.name.clone();
                                match &mut item.value {
                                    crate::editor::SettingValue::Number(n) => {
                                        match *n > 10 {
                                            true => {
                                                *n -= 5;
                                                match name.as_str() {
                                                    "Window Width" => editor.settings.floating_window_width = *n,
                                                    "Window Height" => editor.settings.floating_window_height = *n,
                                                    _ => {}
                                                }
                                            }
                                            false => {}
                                        }
                                    }
                                    crate::editor::SettingValue::Choice { current, .. } => {
                                        match *current > 0 {
                                            true => *current -= 1,
                                            false => {}
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            None => {}
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
                        match update_info {
                            Some((is_cursor, index)) => {
                                match is_cursor {
                                    true => editor.settings.cursor_color = editor.settings.index_to_color(index, false),
                                    false => editor.settings.selection_color = editor.settings.index_to_color(index, true),
                                }
                                editor.update_textarea_colors();
                            }
                            None => {}
                        }

                        // Return early since we dropped fw
                        return true;
                    }
                    (KeyCode::Right, _) | (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                        match items.get_mut(*selected) {
                            Some(item) => {
                                let name = item.name.clone();
                                match &mut item.value {
                                    crate::editor::SettingValue::Number(n) => {
                                        match *n < 100 {
                                            true => {
                                                *n += 5;
                                                match name.as_str() {
                                                    "Window Width" => editor.settings.floating_window_width = *n,
                                                    "Window Height" => editor.settings.floating_window_height = *n,
                                                    _ => {}
                                                }
                                            }
                                            false => {}
                                        }
                                    }
                                    crate::editor::SettingValue::Choice { current, options } => {
                                        match *current < options.len() - 1 {
                                            true => *current += 1,
                                            false => {}
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            None => {}
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
                        match update_info {
                            Some((is_cursor, index)) => {
                                match is_cursor {
                                    true => editor.settings.cursor_color = editor.settings.index_to_color(index, false),
                                    false => editor.settings.selection_color = editor.settings.index_to_color(index, true),
                                }
                                editor.update_textarea_colors();
                            }
                            None => {}
                        }

                        // Return early since we dropped fw
                        return true;
                    }
                    _ => {}
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
                        match *selected_completion {
                            Some(idx) => match completions.get(idx) {
                                Some(comp) => {
                                    if is_directory(comp) {
                                        // Enter the directory
                                        *input = comp.clone();
                                        *cursor_pos = input.chars().count();
                                        *completions = crate::editor::Editor::get_path_completions(input);
                                        *selected_completion = match completions.is_empty() {
                                            true => None,
                                            false => Some(0),
                                        };
                                        return true;
                                    }
                                }
                                None => {}
                            },
                            None => {}
                        }

                        // Otherwise, cycle to next completion and update input
                        match *selected_completion {
                            Some(idx) => {
                                *selected_completion = Some((idx + 1) % completions.len());
                            }
                            None => {
                                *selected_completion = Some(0);
                            }
                        }
                        // Update input to show selected completion
                        match selected_completion.and_then(|i| completions.get(i)) {
                            Some(completion) => {
                                *input = completion.clone();
                                *cursor_pos = input.chars().count();
                            }
                            None => {}
                        }
                    } else {
                        // No completions, refresh them
                        *completions = crate::editor::Editor::get_path_completions(input);
                        if !completions.is_empty() {
                            *selected_completion = Some(0);
                            // Update input to first completion
                            match completions.first() {
                                Some(comp) => {
                                    *input = comp.clone();
                                    *cursor_pos = input.chars().count();
                                }
                                None => {}
                            }
                        }
                    }
                    return true;
                }

                // Handle Up/Down - navigate completions and update input
                match (key.code, key.modifiers) {
                    (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) if !completions.is_empty() => {
                        match *selected_completion {
                            Some(idx) => {
                                *selected_completion = Some(match idx == 0 {
                                    true => completions.len() - 1,
                                    false => idx - 1,
                                });
                            }
                            None => {
                                *selected_completion = Some(completions.len() - 1);
                            }
                        }
                        // Update input to show selected completion
                        match selected_completion.and_then(|i| completions.get(i)) {
                            Some(completion) => {
                                *input = completion.clone();
                                *cursor_pos = input.chars().count();
                            }
                            None => {}
                        }
                        return true;
                    }
                    (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) if !completions.is_empty() => {
                        match *selected_completion {
                            Some(idx) => {
                                *selected_completion = Some((idx + 1) % completions.len());
                            }
                            None => {
                                *selected_completion = Some(0);
                            }
                        }
                        // Update input to show selected completion
                        match selected_completion.and_then(|i| completions.get(i)) {
                            Some(completion) => {
                                *input = completion.clone();
                                *cursor_pos = input.chars().count();
                            }
                            None => {}
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
                        *selected_completion = match completions.is_empty() {
                            true => None,
                            false => Some(0),
                        };
                        // Update input to first completion if available
                        match completions.first() {
                            Some(comp) => {
                                *input = comp.clone();
                                *cursor_pos = input.chars().count();
                            }
                            None => {}
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

                // Handle Backspace - just delete the character before cursor (like Emacs)
                // Completions will be regenerated by the shared handler below
                // No special directory boundary handling needed - Emacs doesn't do it either

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
                        match filtered_commands.get(*selected) {
                            Some(cmd) => {
                                *input = cmd.name.to_string();
                                *cursor_pos = input.len();
                                // Reset selection to top since input changed
                                *selected = 0;
                            }
                            None => {}
                        }
                        return true;
                    }

                    // Execute selected command
                    (KeyCode::Enter, _) => {
                        match filtered_commands.get(*selected) {
                            Some(cmd) => {
                                let cmd_name = cmd.name;
                                // Close the palette first
                                editor.floating_window = None;
                                editor.focus_floating = false;
                                // Execute the command
                                return execute_command(editor, cmd_name);
                            }
                            None => {}
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
                                match filtered_commands.get(*selected) {
                                    Some(cmd) => {
                                        let cmd_name = cmd.name;
                                        editor.floating_window = None;
                                        editor.focus_floating = false;
                                        return execute_command(editor, cmd_name);
                                    }
                                    None => false,
                                }
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
                            match &mut editor.floating_window {
                                Some(ref mut fw) => match &mut fw.mode {
                                    crate::editor::FloatingMode::CommandPalette {
                                        filtered_commands,
                                        selected,
                                        ..
                                    } => {
                                        *filtered_commands = new_filtered;
                                        *selected = 0;
                                    }
                                    _ => {}
                                },
                                None => {}
                            }
                        }
                    }
                }
            }

            crate::editor::FloatingMode::Confirm {
                steps,
                current_index,
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
                    None => None,
                };

                // Drop the borrow before processing the action
                let _ = fw; // End borrow

                // Now process the action with full access to editor
                match action {
                    Some(action) => process_confirm_action(editor, action, idx, total_steps),
                    None => {}
                }
            }
        },
        None => {}
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
            match editor.floating_window.take() {
                Some(mut fw) => match &mut fw.mode {
                    crate::editor::FloatingMode::Confirm { ref mut dialog, .. } => {
                        let result = dialog.handle_response(current_index, &response, editor);

                        // Put the window back (might be modified by handle_response)
                        editor.floating_window = Some(fw);

                        // Apply the result
                        apply_confirm_result(editor, result, total_steps, current_index);
                    }
                    _ => {
                        // Put the window back if not a Confirm mode
                        editor.floating_window = Some(fw);
                    }
                },
                None => {}
            }
        }
    }
}

/// Cancel a confirmation dialog
fn cancel_confirm_dialog(editor: &mut crate::editor::Editor) {
    // Take ownership to avoid borrow issues
    match editor.floating_window.take() {
        Some(fw) => match fw.mode {
            crate::editor::FloatingMode::Confirm { dialog, .. } => {
                dialog.on_cancel(editor);
            }
            _ => {}
        },
        None => {}
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
                match editor.floating_window.take() {
                    Some(fw) => match fw.mode {
                        crate::editor::FloatingMode::Confirm { dialog, .. } => {
                            let _ = dialog.on_complete(editor);
                        }
                        _ => {}
                    },
                    None => {}
                }
                editor.focus_floating = false;
            } else {
                // Advance to next step
                match &mut editor.floating_window {
                    Some(ref mut fw) => match &mut fw.mode {
                        crate::editor::FloatingMode::Confirm { current_index: ref mut ci, .. } => {
                            *ci = current_index + 1;
                        }
                        _ => {}
                    },
                    None => {}
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
            match editor.floating_window.take() {
                Some(fw) => match fw.mode {
                    crate::editor::FloatingMode::Confirm { dialog, .. } => {
                        let _ = dialog.on_complete(editor);
                    }
                    _ => {}
                },
                None => {}
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
            if editor.mark.is_active() {
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
            if editor.mark.is_active() {
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
            ratatui::restore();
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
                editor.save_undo_state();
                editor.textarea.move_cursor(CursorMove::End);
                editor.textarea.insert_newline();
                // Text input resets selection
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
        "page-up" => {
            editor.page_up();
            true
        }
        "page-down" => {
            editor.page_down();
            true
        }
        "recenter" => {
            editor.recenter();
            true
        }

        // Edit commands
        // All these operations are self-contained: they handle undo state and
        // modification tracking internally. See buffer_ops.rs for the pattern.
        "kill-line" => {
            editor.cut_to_end_of_line();
            true
        }
        "kill-line-backward" => {
            editor.cut_to_beginning_of_line();
            true
        }
        "yank" => {
            editor.paste();
            true
        }

        // Selection commands
        "set-mark" => {
            editor.set_mark();
            true
        }
        "kill-region" => {
            editor.cut_region();
            true
        }
        "copy-region" => {
            editor.copy_region();
            true
        }
        "undo" => {
            editor.undo();
            true
        }
        "redo" => {
            editor.redo();
            true
        }

        // Display commands
        "toggle-soft-wrap" => {
            editor.toggle_soft_wrap();
            true
        }

        // Unknown command
        _ => {
            // Command not found - just return true to continue
            true
        }
    }
}