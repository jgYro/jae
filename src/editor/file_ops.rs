//! File operations for the editor.

use super::syntax::{Language, SyntaxState};
use super::{
    CommandInfo, ConfirmationDialog, DeleteFileConfirmation, Editor, FloatingMode, FloatingWindow,
    MarkState, MinibufferCallback, QuitConfirmation,
};
use ratatui::style::Style;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use tui_textarea::TextArea;

impl Editor {
    // ==================== File Operations ====================

    /// Expand ~ to home directory and resolve path
    pub fn expand_path(path_str: &str) -> PathBuf {
        match path_str.starts_with('~') {
            true => match dirs::home_dir() {
                Some(home) => match path_str {
                    "~" => home,
                    _ => match path_str.strip_prefix("~/") {
                        Some(rest) => home.join(rest),
                        None => PathBuf::from(path_str),
                    },
                },
                None => PathBuf::from(path_str),
            },
            false => PathBuf::from(path_str),
        }
    }

    /// Get filesystem completions for a partial path
    pub fn get_path_completions(partial: &str) -> Vec<String> {
        let expanded = Self::expand_path(partial);

        let (dir, prefix) = match partial.ends_with('/') || partial.ends_with(std::path::MAIN_SEPARATOR) {
            true => (expanded.clone(), String::new()),
            false => {
                let parent = match expanded.parent() {
                    Some(p) => p.to_path_buf(),
                    None => expanded.clone(),
                };
                let file_name = match expanded.file_name().and_then(|s| s.to_str()) {
                    Some(name) => name.to_string(),
                    None => String::new(),
                };
                (parent, file_name)
            }
        };

        let mut completions = Vec::new();

        match fs::read_dir(&dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    match entry.file_name().to_str() {
                        Some(name) => {
                            match name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                                true => {
                                    let mut completion = match (partial.starts_with('~') && !partial.starts_with("~/"), partial.ends_with('/') || partial.ends_with(std::path::MAIN_SEPARATOR)) {
                                        (true, _) => format!("~/{}", name),
                                        (_, true) => format!("{}{}", partial, name),
                                        (false, false) => {
                                            let parent_str = match partial.rfind(['/', std::path::MAIN_SEPARATOR]) {
                                                Some(sep_pos) => &partial[..=sep_pos],
                                                None => "",
                                            };
                                            format!("{}{}", parent_str, name)
                                        }
                                    };

                                    match entry.file_type() {
                                        Ok(ft) => match ft.is_dir() {
                                            true => completion.push('/'),
                                            false => {}
                                        },
                                        Err(_) => {}
                                    }

                                    completions.push(completion);
                                }
                                false => {}
                            }
                        }
                        None => {}
                    }
                }
            }
            Err(_) => {}
        }

        completions.sort();
        completions
    }

    /// Open minibuffer for file selection (C-x C-f)
    pub fn open_file_prompt(&mut self) {
        let initial_path = match &self.current_file {
            Some(current) => match current.parent() {
                Some(p) => format!("{}/", p.display()),
                None => "./".to_string(),
            },
            None => match std::env::current_dir() {
                Ok(p) => format!("{}/", p.display()),
                Err(_) => "~/".to_string(),
            },
        };

        let completions = Self::get_path_completions(&initial_path);

        self.floating_window = Some(FloatingWindow {
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Minibuffer {
                prompt: "Find file: ".to_string(),
                input: initial_path.clone(),
                cursor_pos: initial_path.len(),
                completions,
                selected_completion: None,
                callback: MinibufferCallback::OpenFile,
            },
        });
        self.focus_floating = true;
    }

    /// Open minibuffer for directory browsing starting at specified path
    pub fn open_directory_prompt(&mut self, dir: &std::path::Path) {
        let dir_path = if dir.to_string_lossy().ends_with('/') {
            dir.to_string_lossy().to_string()
        } else {
            format!("{}/", dir.display())
        };

        let completions = Self::get_path_completions(&dir_path);

        self.floating_window = Some(FloatingWindow {
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Minibuffer {
                prompt: "Find file: ".to_string(),
                input: dir_path.clone(),
                cursor_pos: dir_path.len(),
                completions,
                selected_completion: None,
                callback: MinibufferCallback::OpenFile,
            },
        });
        self.focus_floating = true;
    }

    /// Open file from path, load into textarea
    pub fn open_file(&mut self, path: &std::path::Path) -> io::Result<()> {
        let mut file = fs::File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let lines: Vec<&str> = contents.lines().collect();
        self.textarea = if lines.is_empty() {
            TextArea::default()
        } else {
            TextArea::new(lines.iter().map(|s| s.to_string()).collect())
        };

        self.update_textarea_colors();
        self.textarea.set_cursor_line_style(Style::default());

        self.current_file = Some(path.to_path_buf());
        self.modified = false;
        self.mark = MarkState::None;
        self.undo_manager.clear();

        // Detect language and initialize unified syntax state
        self.language = Language::from_path(path);
        self.syntax_state = SyntaxState::new(self.language);
        match &mut self.syntax_state {
            Some(state) => {
                state.parse(&contents);
                // Invalidate cache to trigger highlight computation on next render
                state.invalidate_cache();
            }
            None => {}
        }
        self.cached_highlights.clear();

        Ok(())
    }

    /// Save current buffer to current_file (or prompt if none)
    pub fn save_file(&mut self) -> io::Result<()> {
        match self.current_file.clone() {
            Some(path) => self.save_file_to(&path),
            None => {
                self.save_file_as_prompt();
                Ok(())
            }
        }
    }

    /// Open minibuffer for save-as path (C-x C-w)
    pub fn save_file_as_prompt(&mut self) {
        let initial_path = match &self.current_file {
            Some(current) => current.display().to_string(),
            None => match std::env::current_dir() {
                Ok(p) => format!("{}/", p.display()),
                Err(_) => "~/".to_string(),
            },
        };

        let completions = Self::get_path_completions(&initial_path);

        self.floating_window = Some(FloatingWindow {
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Minibuffer {
                prompt: "Save as: ".to_string(),
                input: initial_path.clone(),
                cursor_pos: initial_path.len(),
                completions,
                selected_completion: None,
                callback: MinibufferCallback::SaveFileAs,
            },
        });
        self.focus_floating = true;
    }

    /// Save to specific path
    pub fn save_file_to(&mut self, path: &std::path::Path) -> io::Result<()> {
        let contents = self.textarea.lines().join("\n");
        let mut file = fs::File::create(path)?;
        file.write_all(contents.as_bytes())?;

        self.current_file = Some(path.to_path_buf());
        self.modified = false;

        Ok(())
    }

    /// Start delete file confirmation chain (C-x k)
    pub fn delete_file_prompt(&mut self) {
        let initial_path = match &self.current_file {
            Some(p) => p.display().to_string(),
            None => match std::env::current_dir() {
                Ok(p) => format!("{}/", p.display()),
                Err(_) => "~/".to_string(),
            },
        };

        let completions = Self::get_path_completions(&initial_path);

        self.floating_window = Some(FloatingWindow {
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Minibuffer {
                prompt: "Delete file: ".to_string(),
                input: initial_path.clone(),
                cursor_pos: initial_path.len(),
                completions,
                selected_completion: None,
                callback: MinibufferCallback::DeleteFile,
            },
        });
        self.focus_floating = true;
    }

    /// Start the confirmation dialog for deleting a file
    pub fn start_delete_confirmation(&mut self, path: PathBuf) {
        let dialog = DeleteFileConfirmation { path };
        let steps = dialog.steps();

        self.floating_window = Some(FloatingWindow {
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Confirm {
                dialog: Box::new(dialog),
                steps,
                current_index: 0,
            },
        });
        self.focus_floating = true;
    }

    /// Execute minibuffer callback with current input
    pub fn execute_minibuffer_callback(&mut self) {
        match &self.floating_window {
            Some(fw) => match &fw.mode {
                FloatingMode::Minibuffer {
                    input,
                    callback,
                    ..
                } => {
                    let path = Self::expand_path(input);
                    let callback_clone = callback.clone();
                    let path_clone = path.clone();

                    self.floating_window = None;
                    self.focus_floating = false;

                    match callback_clone {
                        MinibufferCallback::OpenFile => {
                            match self.open_file(&path_clone) {
                                Ok(_) => {}
                                Err(e) => eprintln!("Failed to open file: {}", e),
                            }
                        }
                        MinibufferCallback::SaveFileAs => {
                            match self.save_file_to(&path_clone) {
                                Ok(_) => {}
                                Err(e) => eprintln!("Failed to save file: {}", e),
                            }
                        }
                        MinibufferCallback::DeleteFile => {
                            self.start_delete_confirmation(path_clone);
                        }
                    }
                }
                _ => {}
            },
            None => {}
        }
    }

    /// Mark buffer as modified (called when text changes)
    pub fn mark_modified(&mut self) {
        self.modified = true;
        // Update syntax highlights when buffer changes
        self.update_highlights();
    }

    /// Start the quit confirmation dialog (when buffer is modified)
    pub fn start_quit_confirmation(&mut self) {
        let dialog = QuitConfirmation;
        let steps = dialog.steps();

        self.floating_window = Some(FloatingWindow {
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::Confirm {
                dialog: Box::new(dialog),
                steps,
                current_index: 0,
            },
        });
        self.focus_floating = true;
    }

    /// Open the M-x command palette
    pub fn open_command_palette(&mut self) {
        let all_commands: Vec<CommandInfo> = self
            .status_bar
            .command_registry
            .all_commands()
            .map(|cmd| CommandInfo {
                name: cmd.name,
                description: cmd.description,
                keybinding: cmd.keybinding.as_ref().map(|kb| kb.display()),
            })
            .collect();

        let mut sorted_commands = all_commands;
        sorted_commands.sort_by(|a, b| a.name.cmp(b.name));

        self.floating_window = Some(FloatingWindow {
            visible: true,
            x: 0,
            y: 0,
            width: 80,
            height: 1,
            mode: FloatingMode::CommandPalette {
                input: String::new(),
                cursor_pos: 0,
                filtered_commands: sorted_commands,
                selected: 0,
            },
        });
        self.focus_floating = true;
    }

    /// Filter commands for command palette based on search input
    pub fn filter_commands(&self, query: &str) -> Vec<CommandInfo> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<CommandInfo> = self
            .status_bar
            .command_registry
            .all_commands()
            .filter(|cmd| {
                cmd.name.to_lowercase().contains(&query_lower)
                    || cmd.description.to_lowercase().contains(&query_lower)
            })
            .map(|cmd| CommandInfo {
                name: cmd.name,
                description: cmd.description,
                keybinding: cmd.keybinding.as_ref().map(|kb| kb.display()),
            })
            .collect();

        results.sort_by(|a, b| {
            let a_exact = a.name.to_lowercase() == query_lower;
            let b_exact = b.name.to_lowercase() == query_lower;
            let a_starts = a.name.to_lowercase().starts_with(&query_lower);
            let b_starts = b.name.to_lowercase().starts_with(&query_lower);

            match (a_exact, b_exact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => match (a_starts, b_starts) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(b.name),
                },
            }
        });

        results
    }
}
