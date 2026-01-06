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

        match language {
            Language::Rust => {
                parser
                    .set_language(&tree_sitter_rust::LANGUAGE.into())
                    .ok()?;
            }
            Language::PlainText => return None,
        }

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
}
