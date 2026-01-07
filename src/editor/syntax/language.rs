//! Language detection from file extensions.

use std::path::Path;

/// Supported languages for syntax operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Go,
    C,
    Cpp,
    Json,
    Markdown,
    Html,
    Css,
    Java,
    #[default]
    PlainText,
}

impl Language {
    /// Detect language from file path extension.
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => Language::Rust,
            Some("py" | "pyi" | "pyw") => Language::Python,
            Some("js" | "mjs" | "cjs" | "jsx") => Language::JavaScript,
            Some("ts" | "mts" | "cts") => Language::TypeScript,
            Some("tsx") => Language::Tsx,
            Some("go") => Language::Go,
            Some("c" | "h") => Language::C,
            Some("cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" | "c++" | "h++") => Language::Cpp,
            Some("json" | "jsonc") => Language::Json,
            Some("md" | "markdown") => Language::Markdown,
            Some("html" | "htm") => Language::Html,
            Some("css") => Language::Css,
            Some("java") => Language::Java,
            _ => Language::PlainText,
        }
    }
}
