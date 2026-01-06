//! Language detection from file extensions.

use std::path::Path;

/// Supported languages for syntax operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    Rust,
    #[default]
    PlainText,
}

impl Language {
    /// Detect language from file path extension.
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => Language::Rust,
            _ => Language::PlainText,
        }
    }
}
