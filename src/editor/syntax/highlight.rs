//! Syntax highlighting using tree-sitter-highlight.

use ratatui::style::{Color, Modifier, Style};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Highlight names that we recognize and can style.
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "comment",
    "constant",
    "constant.builtin",
    "constructor",
    "function",
    "function.builtin",
    "function.macro",
    "keyword",
    "label",
    "number",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "string",
    "string.special",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "escape",
];

/// A single highlighted span with start/end byte offsets and style.
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}

/// Map highlight name to terminal style.
pub fn highlight_style(highlight_name: &str) -> Style {
    match highlight_name {
        "keyword" => Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        "function" | "function.builtin" => Style::default().fg(Color::Blue),
        "function.macro" => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        "type" | "type.builtin" => Style::default().fg(Color::Yellow),
        "constructor" => Style::default().fg(Color::Yellow),
        "string" | "string.special" => Style::default().fg(Color::Green),
        "number" => Style::default().fg(Color::Cyan),
        "comment" => Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        "operator" => Style::default().fg(Color::White),
        "punctuation" | "punctuation.bracket" | "punctuation.delimiter" => {
            Style::default().fg(Color::White)
        }
        "variable" | "variable.parameter" => Style::default().fg(Color::White),
        "variable.builtin" => Style::default().fg(Color::Red),
        "constant" | "constant.builtin" => Style::default().fg(Color::Cyan),
        "attribute" => Style::default().fg(Color::Yellow),
        "property" => Style::default().fg(Color::Cyan),
        "label" => Style::default().fg(Color::Magenta),
        "escape" => Style::default().fg(Color::Cyan),
        _ => Style::default(),
    }
}

/// Highlighter state for a specific language.
pub struct SyntaxHighlighter {
    config: HighlightConfiguration,
    highlighter: Highlighter,
}

impl SyntaxHighlighter {
    /// Create a new highlighter for Rust.
    pub fn new_rust() -> Option<Self> {
        let mut config = HighlightConfiguration::new(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            tree_sitter_rust::INJECTIONS_QUERY,
            "",
        )
        .ok()?;

        config.configure(HIGHLIGHT_NAMES);

        Some(Self {
            config,
            highlighter: Highlighter::new(),
        })
    }

    /// Get highlight spans for the given source code.
    pub fn highlight(&mut self, source: &str) -> Vec<HighlightSpan> {
        let mut spans = Vec::new();

        let highlights = match self.highlighter.highlight(&self.config, source.as_bytes(), None, |_| None) {
            Ok(h) => h,
            Err(_) => return spans,
        };

        let mut style_stack: Vec<Style> = vec![Style::default()];

        for event in highlights {
            match event {
                Ok(HighlightEvent::Source { start, end }) => {
                    let style = style_stack.last().copied().unwrap_or_default();
                    if style != Style::default() {
                        spans.push(HighlightSpan { start, end, style });
                    }
                }
                Ok(HighlightEvent::HighlightStart(highlight)) => {
                    let name = HIGHLIGHT_NAMES.get(highlight.0).unwrap_or(&"");
                    let style = highlight_style(name);
                    style_stack.push(style);
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    style_stack.pop();
                }
                Err(_) => break,
            }
        }

        spans
    }
}

