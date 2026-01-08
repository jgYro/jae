//! Syntax highlighting types and utilities.
//!
//! This module provides:
//! - HighlightSpan: A highlighted region with style information
//! - HighlightResult: Result type for highlight operations with error info
//! - HIGHLIGHT_NAMES: List of recognized highlight capture names
//! - highlight_style: Map highlight names to terminal styles

use ratatui::style::{Color, Modifier, Style};

/// Highlight names that we recognize and can style.
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "charset",
    "comment",
    "comment.documentation",
    "constant",
    "constant.builtin",
    "constructor",
    "delimiter",
    "embedded",
    "escape",
    "function",
    "function.builtin",
    "function.macro",
    "function.method",
    "function.special",
    "import",
    "keyframes",
    "keyword",
    "label",
    "media",
    "module",
    "namespace",
    "number",
    "operator",
    "property",
    "property.definition",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "string",
    "string.escape",
    "string.special",
    "string.special.key",
    "supports",
    "tag",
    "tag.error",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
    // Markdown-specific
    "text.title",
    "text.emphasis",
    "text.strong",
    "text.literal",
    "text.uri",
    "text.reference",
];

/// A single highlighted span with start/end byte offsets and style.
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}

/// Result of a highlight operation with detailed error information.
#[derive(Debug)]
pub enum HighlightResult {
    /// Highlighting completed successfully
    Success(Vec<HighlightSpan>),
    /// Highlighting partially completed with some spans and an error
    PartialSuccess {
        spans: Vec<HighlightSpan>,
        error: String,
    },
    /// Highlighting failed completely
    Failure(String),
}

/// Map highlight name to terminal style.
pub fn highlight_style(highlight_name: &str) -> Style {
    match highlight_name {
        // Keywords and control flow
        "keyword" | "import" => Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),

        // Functions
        "function" | "function.builtin" | "function.method" | "function.special" => {
            Style::default().fg(Color::Blue)
        }
        "function.macro" => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),

        // Types and constructors
        "type" | "type.builtin" | "constructor" => Style::default().fg(Color::Yellow),

        // Strings
        "string" | "string.special" | "string.special.key" => Style::default().fg(Color::Green),

        // Numbers and constants
        "number" => Style::default().fg(Color::Cyan),
        "constant" | "constant.builtin" => Style::default().fg(Color::Cyan),

        // Comments
        "comment" | "comment.documentation" => {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC)
        }

        // Operators
        "operator" => Style::default().fg(Color::LightMagenta),

        // Punctuation
        "punctuation" | "punctuation.bracket" | "punctuation.delimiter" | "delimiter" => {
            Style::default().fg(Color::White)
        }
        "punctuation.special" => Style::default().fg(Color::Yellow),

        // Variables
        "variable" | "variable.parameter" => Style::default().fg(Color::White),
        "variable.builtin" => Style::default().fg(Color::Red),

        // Properties and attributes
        "attribute" => Style::default().fg(Color::Yellow),
        "property" | "property.definition" => Style::default().fg(Color::Cyan),
        "namespace" | "module" => Style::default().fg(Color::Yellow),

        // Labels
        "label" => Style::default().fg(Color::Magenta),

        // Escapes
        "escape" | "string.escape" => Style::default().fg(Color::Cyan),

        // Embedded/injected code
        "embedded" => Style::default().fg(Color::Red),

        // HTML/JSX tags
        "tag" => Style::default().fg(Color::Red),
        "tag.error" => Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::UNDERLINED),

        // CSS-specific
        "charset" | "media" | "keyframes" | "supports" => {
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
        }

        // Markdown-specific
        "text.title" => Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
        "text.emphasis" => Style::default().add_modifier(Modifier::ITALIC),
        "text.strong" => Style::default().add_modifier(Modifier::BOLD),
        "text.literal" => Style::default().fg(Color::Green),
        "text.uri" => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED),
        "text.reference" => Style::default().fg(Color::Blue),

        _ => Style::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_names_not_empty() {
        assert!(!HIGHLIGHT_NAMES.is_empty());
    }

    #[test]
    fn test_highlight_style_keyword() {
        let style = highlight_style("keyword");
        assert_eq!(style.fg, Some(Color::Magenta));
    }

    #[test]
    fn test_highlight_style_unknown() {
        let style = highlight_style("unknown_highlight_name");
        assert_eq!(style, Style::default());
    }

    #[test]
    fn test_highlight_result_variants() {
        let success = HighlightResult::Success(vec![]);
        match success {
            HighlightResult::Success(spans) => assert!(spans.is_empty()),
            _ => panic!("Expected Success variant"),
        }

        let partial = HighlightResult::PartialSuccess {
            spans: vec![],
            error: "test error".to_string(),
        };
        match partial {
            HighlightResult::PartialSuccess { error, .. } => assert_eq!(error, "test error"),
            _ => panic!("Expected PartialSuccess variant"),
        }

        let failure = HighlightResult::Failure("fatal error".to_string());
        match failure {
            HighlightResult::Failure(msg) => assert_eq!(msg, "fatal error"),
            _ => panic!("Expected Failure variant"),
        }
    }
}
