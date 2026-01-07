//! Syntax highlighting using tree-sitter-highlight.

use super::Language;
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
    "punctuation.special",
    "string",
    "string.special",
    "string.escape",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "escape",
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
        "escape" | "string.escape" => Style::default().fg(Color::Cyan),
        "punctuation.special" => Style::default().fg(Color::Yellow),
        // Markdown-specific
        "text.title" => Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        "text.emphasis" => Style::default().add_modifier(Modifier::ITALIC),
        "text.strong" => Style::default().add_modifier(Modifier::BOLD),
        "text.literal" => Style::default().fg(Color::Green),
        "text.uri" => Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
        "text.reference" => Style::default().fg(Color::Blue),
        _ => Style::default(),
    }
}

/// Highlighter state for a specific language.
pub struct SyntaxHighlighter {
    config: HighlightConfiguration,
    highlighter: Highlighter,
}

impl SyntaxHighlighter {
    /// Create a new highlighter for the given language.
    pub fn new(language: Language) -> Option<Self> {
        let mut config = match language {
            Language::Rust => HighlightConfiguration::new(
                tree_sitter_rust::LANGUAGE.into(),
                "rust",
                tree_sitter_rust::HIGHLIGHTS_QUERY,
                tree_sitter_rust::INJECTIONS_QUERY,
                "",
            ),
            Language::Python => HighlightConfiguration::new(
                tree_sitter_python::LANGUAGE.into(),
                "python",
                tree_sitter_python::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Language::JavaScript => HighlightConfiguration::new(
                tree_sitter_javascript::LANGUAGE.into(),
                "javascript",
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_javascript::INJECTIONS_QUERY,
                tree_sitter_javascript::LOCALS_QUERY,
            ),
            Language::TypeScript => HighlightConfiguration::new(
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                "typescript",
                tree_sitter_typescript::HIGHLIGHTS_QUERY,
                "",
                tree_sitter_typescript::LOCALS_QUERY,
            ),
            Language::Tsx => HighlightConfiguration::new(
                tree_sitter_typescript::LANGUAGE_TSX.into(),
                "tsx",
                tree_sitter_typescript::HIGHLIGHTS_QUERY,
                "",
                tree_sitter_typescript::LOCALS_QUERY,
            ),
            Language::Go => HighlightConfiguration::new(
                tree_sitter_go::LANGUAGE.into(),
                "go",
                tree_sitter_go::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Language::C => HighlightConfiguration::new(
                tree_sitter_c::LANGUAGE.into(),
                "c",
                tree_sitter_c::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Cpp => HighlightConfiguration::new(
                tree_sitter_cpp::LANGUAGE.into(),
                "cpp",
                tree_sitter_cpp::HIGHLIGHT_QUERY,
                "",
                "",
            ),
            Language::Json => HighlightConfiguration::new(
                tree_sitter_json::LANGUAGE.into(),
                "json",
                tree_sitter_json::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Language::Markdown => {
                // tree-sitter-md has separate block and inline grammars
                // Use just the block grammar - inline highlighting would require injection support
                HighlightConfiguration::new(
                    tree_sitter_md::LANGUAGE.into(),
                    "markdown",
                    tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
                    tree_sitter_md::INJECTION_QUERY_BLOCK,
                    "",
                )
            }
            Language::Html => HighlightConfiguration::new(
                tree_sitter_html::LANGUAGE.into(),
                "html",
                tree_sitter_html::HIGHLIGHTS_QUERY,
                tree_sitter_html::INJECTIONS_QUERY,
                "",
            ),
            Language::Css => HighlightConfiguration::new(
                tree_sitter_css::LANGUAGE.into(),
                "css",
                tree_sitter_css::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Language::Java => HighlightConfiguration::new(
                tree_sitter_java::LANGUAGE.into(),
                "java",
                tree_sitter_java::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
            Language::PlainText => return None,
        }
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

