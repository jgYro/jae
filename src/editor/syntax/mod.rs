//! Syntax module for tree-sitter integration.
//!
//! Provides:
//! - Language detection from file extensions
//! - Syntax highlighting

mod highlight;
mod language;

pub use highlight::{HighlightSpan, SyntaxHighlighter};
pub use language::Language;

use tree_sitter::{Parser, Tree};

/// Syntax state for a buffer - manages parser and parse tree.
pub struct Syntax {
    #[allow(dead_code)] // Will be used for future AST operations
    language: Language,
    parser: Parser,
    tree: Option<Tree>,
}

impl Syntax {
    /// Create a new Syntax for the given language.
    pub fn new(language: Language) -> Option<Self> {
        if language == Language::PlainText {
            return None;
        }

        let mut parser = Parser::new();

        let set_result = match language {
            Language::Rust => parser.set_language(&tree_sitter_rust::LANGUAGE.into()),
            Language::Python => parser.set_language(&tree_sitter_python::LANGUAGE.into()),
            Language::JavaScript => parser.set_language(&tree_sitter_javascript::LANGUAGE.into()),
            Language::TypeScript => parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            Language::Tsx => parser.set_language(&tree_sitter_typescript::LANGUAGE_TSX.into()),
            Language::Go => parser.set_language(&tree_sitter_go::LANGUAGE.into()),
            Language::C => parser.set_language(&tree_sitter_c::LANGUAGE.into()),
            Language::Cpp => parser.set_language(&tree_sitter_cpp::LANGUAGE.into()),
            Language::Json => parser.set_language(&tree_sitter_json::LANGUAGE.into()),
            Language::Markdown => parser.set_language(&tree_sitter_md::LANGUAGE.into()),
            Language::Html => parser.set_language(&tree_sitter_html::LANGUAGE.into()),
            Language::Css => parser.set_language(&tree_sitter_css::LANGUAGE.into()),
            Language::Java => parser.set_language(&tree_sitter_java::LANGUAGE.into()),
            Language::PlainText => return None,
        };

        set_result.ok()?;

        Some(Self {
            language,
            parser,
            tree: None,
        })
    }

    /// Parse the source text.
    pub fn parse(&mut self, source: &str) {
        self.tree = self.parser.parse(source, self.tree.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_new_rust() {
        let syntax = Syntax::new(Language::Rust);
        assert!(syntax.is_some());
    }

    #[test]
    fn test_syntax_new_plain_text() {
        let syntax = Syntax::new(Language::PlainText);
        assert!(syntax.is_none());
    }

    #[test]
    fn test_markdown_highlighter() {
        let highlighter = SyntaxHighlighter::new(Language::Markdown);
        assert!(highlighter.is_some(), "Markdown highlighter should be created");

        let mut hl = highlighter.unwrap();
        let source = "# Hello World\n\nSome *emphasis* and **bold** text.";
        let spans = hl.highlight(source);

        println!("Markdown spans: {:?}", spans);
        // Should have at least some spans for the heading and emphasis
        assert!(!spans.is_empty(), "Should have highlight spans for markdown");
    }

    #[test]
    fn test_typescript_highlighter() {
        let highlighter = SyntaxHighlighter::new(Language::TypeScript);
        assert!(highlighter.is_some(), "TypeScript highlighter should be created");

        let mut hl = highlighter.unwrap();
        let source = r#"
import { useState } from "react";

interface Props {
    name: string;
}

function greet(props: Props): string {
    const message = `Hello, ${props.name}!`;
    return message;
}
"#;
        let spans = hl.highlight(source);

        println!("TypeScript spans ({} total):", spans.len());
        for span in &spans {
            let text = &source[span.start..span.end];
            println!("  {:?} => {:?}", text, span.style);
        }
        assert!(!spans.is_empty(), "Should have highlight spans for TypeScript");
    }

    #[test]
    fn test_tsx_jsx_highlighter() {
        let highlighter = SyntaxHighlighter::new(Language::Tsx);
        assert!(highlighter.is_some(), "TSX highlighter should be created");

        let mut hl = highlighter.unwrap();
        let source = r#"
import React from "react";

function App() {
    return (
        <div className="container">
            <h1>Hello World</h1>
            <button onClick={() => alert("clicked")}>Click me</button>
        </div>
    );
}
"#;
        let spans = hl.highlight(source);

        println!("TSX/JSX spans ({} total):", spans.len());
        for span in &spans {
            let text = &source[span.start..span.end];
            println!("  {:?} => {:?}", text, span.style);
        }
        assert!(!spans.is_empty(), "Should have highlight spans for TSX");

        // Check that we have tag highlighting (JSX elements like div, h1, button)
        let has_tag = spans.iter().any(|s| {
            let text = &source[s.start..s.end];
            text == "div" || text == "h1" || text == "button"
        });
        assert!(has_tag, "Should highlight JSX tags");
    }
}
