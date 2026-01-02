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
            clipboard: Clipboard::new().ok(),
        }
    }

    /// Copy text to system clipboard
    pub fn copy(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(ref mut cb) = self.clipboard {
            let _ = cb.set_text(text);
        }
    }

    /// Paste text from system clipboard
    pub fn paste(&mut self) -> Option<String> {
        self.clipboard.as_mut()?.get_text().ok()
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
