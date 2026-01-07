//! System clipboard integration for JAE editor.
//!
//! Provides simple cut/copy/paste using the OS clipboard via arboard.

use arboard::Clipboard;

pub struct ClipboardManager {
    clipboard: Option<Clipboard>,
}

impl ClipboardManager {
    pub fn new() -> Self {
        Self {
            clipboard: match Clipboard::new() {
                Ok(cb) => Some(cb),
                Err(_) => None,
            },
        }
    }

    /// Copy text to system clipboard
    pub fn copy(&mut self, text: &str) {
        match text.is_empty() {
            true => {}
            false => match &mut self.clipboard {
                Some(cb) => {
                    let _ = cb.set_text(text);
                }
                None => {}
            },
        }
    }

    /// Paste text from system clipboard
    pub fn paste(&mut self) -> Option<String> {
        match &mut self.clipboard {
            Some(cb) => match cb.get_text() {
                Ok(text) => Some(text),
                Err(_) => None,
            },
            None => None,
        }
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
